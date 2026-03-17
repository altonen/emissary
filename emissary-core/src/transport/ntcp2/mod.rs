// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use crate::{
    config::Ntcp2Config,
    error::{ConnectionError, Error},
    primitives::{RouterAddress, RouterId, RouterInfo, TransportKind},
    router::context::RouterContext,
    runtime::{
        Counter, Gauge, Histogram, Instant, JoinSet, MetricType, MetricsHandle, Runtime,
        TcpListener,
    },
    subsystem::SubsystemEvent,
    transport::{
        ntcp2::{
            listener::Ntcp2Listener,
            metrics::*,
            session::{Ntcp2Session, SessionManager},
        },
        TerminationReason, Transport, TransportEvent,
    },
};

use futures::{Stream, StreamExt};
use hashbrown::{hash_map::Entry, HashMap};
use thingbuf::mpsc::Sender;

use alloc::{format, vec, vec::Vec};
use core::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll, Waker},
};

mod listener;
mod message;
mod metrics;
mod options;
mod session;

#[cfg(feature = "fuzz")]
pub use message::MessageBlock;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ntcp2";

/// NTCP2 context.
pub struct Ntcp2Context<R: Runtime> {
    /// NTCP2 configuration.
    config: Ntcp2Config,

    /// IPv4 listener.
    ipv4_listener: Option<R::TcpListener>,

    /// IPv4 socket address.
    ipv4_socket_address: Option<SocketAddr>,

    /// IPv6 listener.
    ipv6_listener: Option<R::TcpListener>,

    /// IPv6 socket address.
    ipv6_socket_address: Option<SocketAddr>,
}

impl<R: Runtime> Ntcp2Context<R> {
    /// Get the port of `Ntcp2LIstener`.
    ///
    /// IPv4 and IPv6 are always bound to the same port.
    pub fn port(&self) -> u16 {
        self.ipv4_socket_address
            .as_ref()
            .or(self.ipv6_socket_address.as_ref())
            .map(SocketAddr::port)
            .expect("socket address to exist")
    }

    /// Classify `Ntcp2Listener` into zero or more `TransportKind`s.
    pub fn classify(&self) -> impl Iterator<Item = TransportKind> {
        vec![
            self.ipv4_listener.is_some().then_some(TransportKind::Ntcp2V4),
            self.ipv6_listener.is_some().then_some(TransportKind::Ntcp2V6),
        ]
        .into_iter()
        .flatten()
    }

    /// Get copy of [`Ntcp2Config`].
    pub fn config(&self) -> Ntcp2Config {
        self.config.clone()
    }
}

/// NTCP2 transport.
pub struct Ntcp2Transport<R: Runtime> {
    /// IPv4 connection listener.
    ipv4_listener: Option<Ntcp2Listener<R>>,

    /// IPv6 connection listener.
    ipv6_listener: Option<Ntcp2Listener<R>>,

    /// Open connections.
    open_connections: R::JoinSet<(RouterId, TerminationReason)>,

    /// Pending connections.
    ///
    /// Connections which have been established successfully
    /// but are waiting approval/rejection from the `TransportManager`.
    pending_connections: HashMap<RouterId, Ntcp2Session<R>>,

    /// Pending connections.
    ///
    /// `RouterId` is `None` for inbound sessions.
    pending_handshakes: R::JoinSet<Result<Ntcp2Session<R>, (Option<RouterId>, Error)>>,

    /// Router context.
    router_ctx: RouterContext<R>,

    /// Session manager.
    session_manager: SessionManager<R>,

    /// Waker.
    waker: Option<Waker>,
}

impl<R: Runtime> Ntcp2Transport<R> {
    /// Create new [`Ntcp2Transport`].
    pub fn new(
        context: Ntcp2Context<R>,
        allow_local: bool,
        router_ctx: RouterContext<R>,
        transport_tx: Sender<SubsystemEvent>,
    ) -> Self {
        let Ntcp2Context {
            config,
            ipv4_listener,
            ipv4_socket_address,
            ipv6_listener,
            ipv6_socket_address,
        } = context;

        let session_manager = SessionManager::new(
            config.key,
            config.iv,
            router_ctx.clone(),
            allow_local,
            transport_tx,
        );

        tracing::info!(
            target: LOG_TARGET,
            ipv4_address = ?ipv4_socket_address,
            ipv6_address = ?ipv6_socket_address,
            ?allow_local,
            "starting ntcp2",
        );

        Ntcp2Transport {
            ipv4_listener: ipv4_listener.map(|listener| Ntcp2Listener::new(listener, allow_local)),
            ipv6_listener: ipv6_listener.map(|listener| Ntcp2Listener::new(listener, allow_local)),
            open_connections: R::join_set(),
            pending_connections: HashMap::new(),
            pending_handshakes: R::join_set(),
            router_ctx,
            session_manager,
            waker: None,
        }
    }

    /// Collect `Ntcp2Transport`-related metric counters, gauges and histograms.
    pub fn metrics(metrics: Vec<MetricType>) -> Vec<MetricType> {
        register_metrics(metrics)
    }

    /// Initialize `Ntcp2Transport`.
    ///
    /// If NTCP2 has been enabled, create router address(es) using the configuration that was
    /// provided and bind TCP listener(s) to the port specified in the config.
    ///
    /// `Ntcp2Transport` can be run as IPv4-only, IPv6-only or with IPv4 and IPv6 enabled.
    ///
    /// Returns `RouterAddress`(es) of the transport and an `Ntcp2Context` that needs to be passed
    /// to `Ntcp2Transport::new()` when constructing the transport.
    pub async fn initialize(
        config: Option<Ntcp2Config>,
    ) -> crate::Result<(
        Option<Ntcp2Context<R>>,
        Option<RouterAddress>,
        Option<RouterAddress>,
    )> {
        let Some(config) = config else {
            return Ok((None, None, None));
        };

        if !config.ipv4 && !config.ipv6 {
            tracing::info!(
                target: LOG_TARGET,
                "both ipv4 and ipv6 disabled, disabling ntcp2",
            );
            return Ok((None, None, None));
        }

        // create ipv4 listener if it was enabled
        let (ipv4_listener, ipv4_socket_address, ipv4_address) = if config.ipv4 {
            let listener = R::TcpListener::bind(
                format!("0.0.0.0:{}", config.port).parse().expect("to succeed"),
            )
            .await
            .ok_or_else(|| {
                tracing::warn!(
                    target: LOG_TARGET,
                    port = %config.port,
                    "ntcp2 port in use, select another port for the transport",
                );

                Error::Connection(ConnectionError::BindFailure)
            })?;

            let socket_address = listener.local_address().ok_or_else(|| {
                tracing::warn!(
                    target: LOG_TARGET,
                    "failed to get local address of the ipv4 ntcp2 listener",
                );

                Error::Connection(ConnectionError::BindFailure)
            })?;

            let address = match (config.publish, config.ipv4_host) {
                (true, Some(host)) => RouterAddress::new_published_ntcp2(
                    config.key,
                    config.iv,
                    IpAddr::V4(host),
                    socket_address,
                ),
                (true, None) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "ntcp2 requested to be published but no host provided",
                    );
                    RouterAddress::new_unpublished_ntcp2(config.key, socket_address)
                }
                (_, _) => RouterAddress::new_unpublished_ntcp2(config.key, socket_address),
            };

            (Some(listener), Some(socket_address), Some(address))
        } else {
            (None, None, None)
        };

        // create ipv6 listener if it was enabled
        let (ipv6_listener, ipv6_socket_address, ipv6_address) = if config.ipv6 {
            // bind ipv4 and ipv6 to same ports
            let port = ipv4_listener.as_ref().map_or(config.port, |address| {
                address.local_address().expect("address to exist").port()
            });

            let listener =
                R::TcpListener::bind(format!("[::]:{port}").parse().expect("to succeed"))
                    .await
                    .ok_or_else(|| {
                        tracing::warn!(
                            target: LOG_TARGET,
                            port = %config.port,
                            "ntcp2 port in use, select another port for the transport",
                        );

                        Error::Connection(ConnectionError::BindFailure)
                    })?;

            let socket_address = listener.local_address().ok_or_else(|| {
                tracing::warn!(
                    target: LOG_TARGET,
                    "failed to get local address of the ipv6 ntcp2 listener",
                );

                Error::Connection(ConnectionError::BindFailure)
            })?;

            let address = match (config.publish, config.ipv6_host) {
                (true, Some(host)) => RouterAddress::new_published_ntcp2(
                    config.key,
                    config.iv,
                    IpAddr::V6(host),
                    socket_address,
                ),
                (true, None) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "ntcp2 requested to be published but no host provided",
                    );
                    RouterAddress::new_unpublished_ntcp2(config.key, socket_address)
                }
                (_, _) => RouterAddress::new_unpublished_ntcp2(config.key, socket_address),
            };

            (Some(listener), Some(socket_address), Some(address))
        } else {
            (None, None, None)
        };

        Ok((
            Some(Ntcp2Context {
                config,
                ipv4_listener,
                ipv4_socket_address,
                ipv6_listener,
                ipv6_socket_address,
            }),
            ipv4_address,
            ipv6_address,
        ))
    }
}

impl<R: Runtime> Transport for Ntcp2Transport<R> {
    fn connect(&mut self, router: RouterInfo) {
        tracing::trace!(
            target: LOG_TARGET,
            router_id = %router.identity.id(),
            "negotiate ntcp2 session with router",
        );

        let future = self.session_manager.create_session(
            router,
            self.ipv4_listener.is_some(),
            self.ipv6_listener.is_some(),
        );
        self.pending_handshakes.push(future);
        self.router_ctx.metrics_handle().counter(NUM_OUTBOUND_NTCP2).increment(1);

        if let Some(waker) = self.waker.take() {
            waker.wake_by_ref();
        }
    }

    fn accept(&mut self, router_id: &RouterId) {
        match self.pending_connections.remove(router_id) {
            Some(session) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    %router_id,
                    "ntcp2 session accepted, starting event loop",
                );
                self.router_ctx.metrics_handle().gauge(NUM_CONNECTIONS).increment(1);

                self.open_connections.push(session.run());

                if let Some(waker) = self.waker.take() {
                    waker.wake_by_ref();
                }
            }
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    %router_id,
                    "cannot accept non-existent ntcp2 session",
                );
                debug_assert!(false);
            }
        }
    }

    fn reject(&mut self, router_id: &RouterId) {
        match self.pending_connections.remove(router_id) {
            Some(connection) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    %router_id,
                    "ntcp2 session rejected, closing connection",
                );
                drop(connection);
            }
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    %router_id,
                    "cannot reject non-existent ntcp2 session",
                );
                debug_assert!(false);
            }
        }
    }
}

impl<R: Runtime> Stream for Ntcp2Transport<R> {
    type Item = TransportEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = Pin::into_inner(self);

        match this.open_connections.poll_next_unpin(cx) {
            Poll::Pending => {}
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Ready(Some((router_id, reason))) => {
                this.router_ctx.metrics_handle().gauge(NUM_CONNECTIONS).decrement(1);
                return Poll::Ready(Some(TransportEvent::ConnectionClosed { router_id, reason }));
            }
        }

        if let Some(ref mut listener) = this.ipv4_listener {
            loop {
                match listener.poll_next_unpin(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => return Poll::Ready(None),
                    Poll::Ready(Some((stream, address))) => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            ?address,
                            "inbound ipv4 tcp connection, accept session",
                        );

                        let future = this.session_manager.accept_session(stream, address);
                        this.pending_handshakes.push(future);
                        this.router_ctx.metrics_handle().counter(NUM_INBOUND_NTCP2).increment(1);
                    }
                }
            }
        }

        if let Some(ref mut listener) = this.ipv6_listener {
            loop {
                match listener.poll_next_unpin(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => return Poll::Ready(None),
                    Poll::Ready(Some((stream, address))) => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            "inbound ipv6 tcp connection, accept session",
                        );

                        let future = this.session_manager.accept_session(stream, address);
                        this.pending_handshakes.push(future);
                        this.router_ctx.metrics_handle().counter(NUM_INBOUND_NTCP2).increment(1);
                    }
                }
            }
        }

        loop {
            match this.pending_handshakes.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(Some(Ok(session))) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        role = ?session.role(),
                        router = %session.router().identity.id(),
                        "ntcp2 connection opened",
                    );
                    this.router_ctx.metrics_handle().counter(NUM_HANDSHAKE_SUCCESSES).increment(1);
                    this.router_ctx
                        .metrics_handle()
                        .histogram(HANDSHAKE_DURATION)
                        .record(session.started().elapsed().as_millis() as f64);

                    // get router info from the session, store the session itthis into
                    // `pending_connections` and inform `TransportManager` that new ntcp2 connection
                    // with `router` has been opened
                    //
                    // `TransportManager` will either accept or reject the session
                    let router_info = session.router();
                    let router_id = router_info.identity.id();
                    let direction = session.direction();
                    let address = session.address();

                    // multiple connections raced and got negotiated at the same time
                    //
                    // reject any connection to/from the same router if a connection is already
                    // under validation in `TransportManager`
                    match this.pending_connections.entry(router_id.clone()) {
                        Entry::Vacant(entry) => {
                            entry.insert(session);
                        }
                        Entry::Occupied(_) => {
                            tracing::debug!(
                                target: LOG_TARGET,
                                %router_id,
                                "pending connection already exist, rejecting new connection",
                            );
                            continue;
                        }
                    }

                    return Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                        address,
                        direction,
                        router_id,
                    }));
                }
                Poll::Ready(Some(Err((router_id, error)))) => match router_id {
                    Some(router_id) => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            ?error,
                            "failed to connect to router",
                        );
                        this.router_ctx
                            .metrics_handle()
                            .counter(NUM_HANDSHAKE_FAILURES)
                            .increment(1);

                        return Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id }));
                    }
                    None => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to accept inbound connection",
                        );
                        this.router_ctx
                            .metrics_handle()
                            .counter(NUM_HANDSHAKE_FAILURES)
                            .increment(1);
                    }
                },
                Poll::Ready(None) => return Poll::Ready(None),
            }
        }

        this.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{primitives::Str, runtime::mock::MockRuntime};

    #[tokio::test]
    async fn publish_ntcp2_ipv4() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: Some("8.8.8.8".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();
        let port = context.as_ref().unwrap().ipv4_socket_address.unwrap().port().to_string();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert_eq!(
                    socket_address,
                    Some(format!("0.0.0.0:{port}").parse().unwrap())
                );
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("8.8.8.8")));
                assert_eq!(options.get(&Str::from("port")), Some(&Str::from(port)));
                assert!(options.get(&Str::from("i")).is_some());
                assert!(socket_address.is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn publish_ntcp2_ipv6() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: None,
            ipv6_host: Some("::1".parse().unwrap()),
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: false,
            ipv6: true,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();
        let port = context.as_ref().unwrap().ipv6_socket_address.unwrap().port().to_string();

        match ipv6_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert_eq!(
                    socket_address,
                    Some(format!("[::]:{port}").parse().unwrap())
                );
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("::1")));
                assert_eq!(
                    options.get(&Str::from("port")),
                    Some(&Str::from(port.clone()))
                );
                assert!(options.get(&Str::from("i")).is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv4_address.is_none());
    }

    #[tokio::test]
    async fn publish_ntcp2_ipv4_and_ipv6() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: Some("8.8.8.8".parse().unwrap()),
            ipv6_host: Some("::1".parse().unwrap()),
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: true,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();
        let port = context.as_ref().unwrap().ipv6_socket_address.unwrap().port().to_string();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert_eq!(
                    socket_address,
                    Some(format!("0.0.0.0:{port}").parse().unwrap())
                );
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("8.8.8.8")));
                assert_eq!(
                    options.get(&Str::from("port")),
                    Some(&Str::from(port.clone()))
                );
                assert!(options.get(&Str::from("i")).is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }

        match ipv6_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert_eq!(
                    socket_address,
                    Some(format!("[::]:{port}").parse().unwrap())
                );
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("::1")));
                assert_eq!(
                    options.get(&Str::from("port")),
                    Some(&Str::from(port.clone()))
                );
                assert!(options.get(&Str::from("i")).is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }

        assert!(context.is_some());
    }

    #[tokio::test]
    async fn dont_publish_ntcp2() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: None,
            ipv6_host: None,
            publish: false,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("i")).is_none());
                assert!(socket_address.is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn dont_publish_ntcp2_host_specified() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: Some("8.8.8.8".parse().unwrap()),
            ipv6_host: None,
            publish: false,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("i")).is_none());
                assert!(socket_address.is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn publish_ntcp2_but_no_host() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: None,
            ipv6_host: None,
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("i")).is_none());
                assert!(socket_address.is_some());
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn bind_to_random_port() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: None,
            ipv6_host: None,
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("i")).is_none());
                assert!(socket_address.is_some());
                assert_ne!(socket_address.as_ref().unwrap().port(), 0u16);
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn publish_random_port() {
        let config = Some(Ntcp2Config {
            port: 0u16,
            ipv4_host: Some("8.8.8.8".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0xaa; 32],
            iv: [0xbb; 16],
            ipv4: true,
            ipv6: false,
        });
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(config).await.unwrap();

        match ipv4_address.unwrap() {
            RouterAddress::Ntcp2 {
                options,
                socket_address,
                ..
            } => {
                let published_port =
                    options.get(&Str::from("port")).unwrap().parse::<u16>().unwrap();
                let socket_address_port = socket_address.as_ref().unwrap().port();

                assert!(options.get(&Str::from("host")).is_some());
                assert!(options.get(&Str::from("port")).is_some());
                assert!(options.get(&Str::from("i")).is_some());
                assert_eq!(published_port, socket_address_port);
                assert_ne!(published_port, 0u16);
            }
            _ => panic!("invalid ntcp2 address"),
        }
        assert!(context.is_some());
        assert!(ipv6_address.is_none());
    }

    #[tokio::test]
    async fn ntcp2_not_enabled() {
        let (context, ipv4_address, ipv6_address) =
            Ntcp2Transport::<MockRuntime>::initialize(None).await.unwrap();
        assert!(context.is_none());
        assert!(ipv4_address.is_none());
        assert!(ipv6_address.is_none());
    }
}
