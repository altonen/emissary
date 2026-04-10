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
    config::Ssu2Config,
    constants::ssu2,
    crypto::StaticPrivateKey,
    error::{ConnectionError, Error},
    primitives::{MlKemPreference, RouterAddress, RouterId, RouterInfo, TransportKind},
    router::context::RouterContext,
    runtime::{MetricType, Runtime, UdpSocket},
    subsystem::SubsystemEvent,
    transport::{ssu2::socket::Ssu2Socket, Transport, TransportEvent},
};

use futures::{Stream, StreamExt};
use thingbuf::mpsc::Sender;

use alloc::{format, vec, vec::Vec};
use core::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

mod detector;
mod duplicate;
mod message;
mod metrics;
mod peer_test;
mod relay;
mod session;
mod socket;

#[cfg(feature = "fuzz")]
pub use message::{Block, HeaderReader};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2";

#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet.
    pub pkt: Vec<u8>,

    /// Socket address of the remote router.
    #[allow(unused)]
    pub address: SocketAddr,
}

impl Default for Packet {
    fn default() -> Self {
        Self {
            pkt: Default::default(),
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        }
    }
}

/// SSU2 context.
pub struct Ssu2Context<R: Runtime> {
    /// SSU configuration.
    config: Ssu2Config,

    /// Force router to be firewalled.
    firewalled: bool,

    /// IPv4 ML-KEM preference.
    ipv4_ml_kem: Option<MlKemPreference>,

    /// IPv4 MTU.
    ipv4_mtu: Option<usize>,

    /// IPv4 UDP socket.
    ipv4_socket: Option<R::UdpSocket>,

    /// IPv4 socket address.
    ipv4_socket_address: Option<SocketAddr>,

    /// IPv6 ML-KEM preference.
    ipv6_ml_kem: Option<MlKemPreference>,

    /// IPv6 MTU.
    ipv6_mtu: Option<usize>,

    /// IPv6 UDP socket.
    ipv6_socket: Option<R::UdpSocket>,

    /// IPV6 socket address.
    ipv6_socket_address: Option<SocketAddr>,
}

impl<R: Runtime> Ssu2Context<R> {
    /// Get the port where [`Ssu2Socket`] is bound to.
    pub fn port(&self) -> u16 {
        self.ipv4_socket_address
            .as_ref()
            .or(self.ipv6_socket_address.as_ref())
            .map(SocketAddr::port)
            .expect("socket address to exist")
    }

    /// Classify `Ssu2Socket` into zero or more `TransportKind`s.
    pub fn classify(&self) -> impl Iterator<Item = TransportKind> {
        vec![
            self.ipv4_socket.is_some().then_some(TransportKind::Ssu2V4),
            self.ipv6_socket.is_some().then_some(TransportKind::Ssu2V6),
        ]
        .into_iter()
        .flatten()
    }

    /// Get copy of [`Ssu2Config`].
    pub fn config(&self) -> Ssu2Config {
        self.config.clone()
    }
}

/// SSU2 transport.
pub struct Ssu2Transport<R: Runtime> {
    /// SSU2 server socket.
    socket: Ssu2Socket<R>,
}

impl<R: Runtime> Ssu2Transport<R> {
    /// Create new [`Ssu2Transport`].
    pub fn new(
        context: Ssu2Context<R>,
        allow_local: bool,
        router_ctx: RouterContext<R>,
        transport_tx: Sender<SubsystemEvent>,
    ) -> Self {
        let Ssu2Context {
            config,
            firewalled,
            ipv4_ml_kem,
            ipv4_mtu,
            ipv4_socket,
            ipv4_socket_address,
            ipv6_ml_kem,
            ipv6_mtu,
            ipv6_socket,
            ipv6_socket_address,
        } = context;

        tracing::info!(
            target: LOG_TARGET,
            ipv4_address = ?ipv4_socket_address,
            ipv4_mtu = ?ipv4_mtu,
            ipv6_address = ?ipv6_socket_address,
            ipv6_mtu = ?ipv6_mtu,
            ?allow_local,
            "starting ssu2",
        );

        Self {
            socket: Ssu2Socket::<R>::new(
                ipv4_socket,
                ipv4_mtu,
                ipv4_ml_kem,
                ipv6_socket,
                ipv6_mtu,
                ipv6_ml_kem,
                StaticPrivateKey::from_bytes(config.static_key),
                config.intro_key,
                transport_tx,
                router_ctx.clone(),
                firewalled,
                config.disable_pq,
            ),
        }
    }

    /// Collect SSU2-related metric counters, gauges and histograms.
    pub fn metrics(metrics: Vec<MetricType>) -> Vec<MetricType> {
        metrics::register_metrics(metrics)
    }

    /// Configure socket.
    async fn configure_socket(
        config: &Ssu2Config,
        ipv4: bool,
        host: Option<IpAddr>,
        mtu: Option<usize>,
    ) -> crate::Result<(
        Option<R::UdpSocket>,
        Option<SocketAddr>,
        Option<RouterAddress>,
    )> {
        if (ipv4 && !config.ipv4) || (!ipv4 && !config.ipv6) {
            return Ok((None, None, None));
        }

        let address = if ipv4 {
            format!("0.0.0.0:{}", config.port).parse::<SocketAddr>().expect("to succeed")
        } else {
            format!("[::]:{}", config.port).parse::<SocketAddr>().expect("to succeed")
        };

        let socket = match mtu {
            None => R::UdpSocket::bind(address).await,
            Some(mut mtu) => {
                if mtu > ssu2::MAX_MTU {
                    tracing::info!(
                        target: LOG_TARGET,
                        ?mtu,
                        "clamping configured mtu to maximum mtu ({})",
                        ssu2::MAX_MTU,
                    );
                    mtu = ssu2::MAX_MTU;
                }

                R::UdpSocket::bind_with_mtu(address, mtu).await
            }
        }
        .ok_or_else(|| {
            tracing::warn!(
                target: LOG_TARGET,
                port = %config.port,
                "ssu2 port in use, select another port for the transport",
            );

            Error::Connection(ConnectionError::BindFailure)
        })?;

        let mtu = socket.mtu();
        if socket.mtu() < ssu2::MIN_MTU {
            tracing::error!(
                target: LOG_TARGET,
                ?mtu,
                "mtu size is smaller than minimum mtu ({}), unable to start ssu2 for {}",
                ssu2::MIN_MTU,
                if ipv4 { "ipv4" } else { "ipv6" },
            );
            return Ok((None, None, None));
        }

        let socket_address = socket.local_address().ok_or_else(|| {
            tracing::warn!(
                target: LOG_TARGET,
                "failed to get local address of the ipv4 ssu2 listener",
            );

            Error::Connection(ConnectionError::BindFailure)
        })?;

        let address = match (config.publish, host) {
            (true, Some(host)) => RouterAddress::new_published_ssu2(
                config.static_key,
                config.intro_key,
                config.ml_kem.clone(),
                config.disable_pq,
                host,
                socket_address,
                mtu,
            ),
            (true, None) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    "ssu2 requested to be published but no host provided",
                );
                RouterAddress::new_unpublished_ssu2(
                    config.static_key,
                    config.intro_key,
                    config.ml_kem.clone(),
                    config.disable_pq,
                    socket_address,
                    mtu,
                )
            }
            (_, _) => RouterAddress::new_unpublished_ssu2(
                config.static_key,
                config.intro_key,
                config.ml_kem.clone(),
                config.disable_pq,
                socket_address,
                mtu,
            ),
        };

        if config.disable_pq {
            tracing::info!(
                target: LOG_TARGET,
                "running ssu2 {} as x25519-only",
                if ipv4 { "ipv4" } else { "ipv6" }
            );
        } else {
            match address {
                RouterAddress::Ntcp2 { .. } => unreachable!(),
                RouterAddress::Ssu2 { ml_kem, .. } => {
                    tracing::info!(
                        target: LOG_TARGET,
                        inbound = ?ml_kem,
                        "running ssu2 {} with PQ connections",
                        if ipv4 { "ipv4" } else { "ipv6" }
                    );
                }
            }
        }

        Ok((Some(socket), Some(socket_address), Some(address)))
    }

    /// Initialize `Ssu2Transport`.
    ///
    /// If SSU2 has been enabled, create router address(es) using the configuration that was
    /// provided and bind UDP socket(s) to the port specified in the config.
    ///
    /// `Ssu2Transport` can be run as IPv4-only, IPv6-only or with IPv4 and IPv6 enabled.
    ///
    /// Returns `RouterAddress`(es) of the transport and an `Ssu22Context` that needs to be passed
    /// to `Ssu22Transport::new()` when constructing the transport.
    pub async fn initialize(
        config: Option<Ssu2Config>,
    ) -> crate::Result<(
        Option<Ssu2Context<R>>,
        Option<RouterAddress>,
        Option<RouterAddress>,
    )> {
        let Some(config) = config else {
            return Ok((None, None, None));
        };

        tracing::warn!(
            target: LOG_TARGET,
            "ssu2 support is experimental and not recommend for general use",
        );

        // create ipv4 address if it was enabled
        let (ipv4_socket, ipv4_socket_address, ipv4_address) = Self::configure_socket(
            &config,
            true,
            config.ipv4_host.map(IpAddr::V4),
            config.ipv4_mtu,
        )
        .await?;

        // create ipv6 address if it was enabled
        let (ipv6_socket, ipv6_socket_address, ipv6_address) = Self::configure_socket(
            &config,
            false,
            config.ipv6_host.map(IpAddr::V6),
            config.ipv6_mtu,
        )
        .await?;

        if ipv6_socket.is_none() && ipv4_socket.is_none() {
            return Ok((None, None, None));
        }

        Ok((
            Some(Ssu2Context {
                config,
                ipv4_socket_address,
                ipv4_mtu: ipv4_socket.as_ref().map(|socket| socket.mtu()),
                ipv4_ml_kem: ipv4_address.as_ref().and_then(|address| match address {
                    RouterAddress::Ssu2 { ml_kem, .. } => *ml_kem,
                    RouterAddress::Ntcp2 { .. } => unreachable!(),
                }),
                ipv4_socket,
                ipv6_socket_address,
                ipv6_mtu: ipv6_socket.as_ref().map(|socket| socket.mtu()),
                ipv6_ml_kem: ipv6_address.as_ref().and_then(|address| match address {
                    RouterAddress::Ssu2 { ml_kem, .. } => *ml_kem,
                    RouterAddress::Ntcp2 { .. } => unreachable!(),
                }),
                ipv6_socket,
                firewalled: false,
            }),
            ipv4_address,
            ipv6_address,
        ))
    }
}

impl<R: Runtime> Transport for Ssu2Transport<R> {
    fn connect(&mut self, router_info: RouterInfo) {
        self.socket.connect(router_info);
    }

    fn accept(&mut self, router_id: &RouterId) {
        self.socket.accept(router_id);
    }

    fn reject(&mut self, router_id: &RouterId) {
        self.socket.reject(router_id);
    }
}

impl<R: Runtime> Stream for Ssu2Transport<R> {
    type Item = TransportEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.socket.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{base64_encode, SigningPrivateKey},
        events::EventManager,
        i2np::{Message, MessageType, I2NP_MESSAGE_EXPIRATION},
        primitives::Str,
        profile::ProfileStorage,
        runtime::mock::MockRuntime,
        subsystem::{OutboundMessage, OutboundMessageRecycle},
        timeout,
        transport::{EncryptionKind, FirewallStatus},
    };
    use bytes::Bytes;
    use std::{collections::HashMap, time::Duration};
    use thingbuf::mpsc::channel;

    #[tokio::test]
    async fn connect_ssu2_ipv4_x25519() {
        connect_ssu2(true, true, None).await
    }

    #[tokio::test]
    async fn connect_ssu2_ipv4_ml_kem_512() {
        connect_ssu2(true, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn connect_ssu2_ipv4_ml_kem_768() {
        connect_ssu2(true, false, Some("4,3".to_string())).await
    }

    #[tokio::test]
    async fn connect_ssu2_ipv6_x25519() {
        connect_ssu2(false, true, None).await
    }

    #[tokio::test]
    async fn connect_ssu2_ipv6_ml_kem_512() {
        connect_ssu2(false, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn connect_ssu2_ipv6_ml_kem_768() {
        connect_ssu2(false, false, Some("4,3".to_string())).await
    }

    async fn connect_ssu2(ipv4: bool, disable_pq: bool, ml_kem: Option<String>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (ctx1, address1_ipv4, address1_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();
        let (ctx2, address2_ipv4, address2_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static1, signing1) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static2, signing2) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let router_info1 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address1_ipv4,
            address1_ipv6,
            &static1,
            &signing1,
            false,
        );
        let router_info2 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address2_ipv4,
            address2_ipv6,
            &static2,
            &signing2,
            false,
        );
        let (event1_tx, _event1_rx) = channel(64);
        let (event2_tx, _event2_rx) = channel(64);

        let mut transport1 = Ssu2Transport::<MockRuntime>::new(
            ctx1.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info1.identity.id(),
                Bytes::from(router_info1.serialize(&signing1)),
                static1,
                signing1.clone(),
                2u8,
                event_handle.clone(),
            ),
            event1_tx,
        );
        let mut transport2 = Ssu2Transport::<MockRuntime>::new(
            ctx2.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info2.identity.id(),
                Bytes::from(router_info2.serialize(&signing2)),
                static2,
                signing2.clone(),
                2u8,
                event_handle.clone(),
            ),
            event2_tx,
        );
        let router_info2 =
            RouterInfo::parse::<MockRuntime>(router_info2.serialize(&signing2)).unwrap();

        tokio::spawn(async move {
            loop {
                match transport2.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport2.accept(&router_id);
                    }
                    _ => {}
                }
            }
        });

        transport1.connect(router_info2);
        let future = async move {
            loop {
                match transport1.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id,
                        address,
                        encryption,
                        ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport1.accept(&router_id);
                        break encryption;
                    }
                    _ => {}
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(15), future).await {
            Err(_) => panic!("timeout"),
            Ok(encryption) => match encryption {
                EncryptionKind::X25519 => assert!(disable_pq && ml_kem.is_none()),
                EncryptionKind::MlKem512X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "3,4".to_string()),
                EncryptionKind::MlKem768X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "4,3".to_string()),
                EncryptionKind::MlKem1024X25519 => unreachable!(),
            },
        }
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv4_x25519() {
        connect_ssu2_wrong_network(true, true, None).await
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv4_ml_kem_512() {
        connect_ssu2_wrong_network(true, false, Some("3,4".to_string())).await
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv4_ml_kem_768() {
        connect_ssu2_wrong_network(true, false, Some("4,3".to_string())).await
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv6_x25519() {
        connect_ssu2_wrong_network(false, true, None).await
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv6_ml_kem_512() {
        connect_ssu2_wrong_network(false, false, Some("3,4".to_string())).await
    }

    #[tokio::test(start_paused = true)]
    async fn connect_ssu2_wrong_network_ipv6_ml_kem_768() {
        connect_ssu2_wrong_network(false, false, Some("4,3".to_string())).await
    }

    async fn connect_ssu2_wrong_network(ipv4: bool, disable_pq: bool, ml_kem: Option<String>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (ctx1, address1_ipv4, address1_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();
        let (ctx2, address2_ipv4, address2_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: None,
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static1, signing1) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static2, signing2) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let router_info1 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address1_ipv4,
            address1_ipv6,
            &static1,
            &signing1,
            false,
        );
        let router_info2 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address2_ipv4,
            address2_ipv6,
            &static2,
            &signing2,
            false,
        );
        let (event1_tx, _event1_rx) = channel(64);
        let (event2_tx, _event2_rx) = channel(64);

        let mut transport1 = Ssu2Transport::<MockRuntime>::new(
            ctx1.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info1.identity.id(),
                Bytes::from(router_info1.serialize(&signing1)),
                static1,
                signing1,
                2u8,
                event_handle.clone(),
            ),
            event1_tx,
        );
        let mut transport2 = Ssu2Transport::<MockRuntime>::new(
            ctx2.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info2.identity.id(),
                Bytes::from(router_info2.serialize(&signing2)),
                static2,
                signing2,
                5u8, // wrong network
                event_handle.clone(),
            ),
            event2_tx,
        );
        tokio::spawn(async move { while let Some(_) = transport2.next().await {} });

        transport1.connect(router_info2);
        let future = async move {
            loop {
                match transport1.next().await.unwrap() {
                    TransportEvent::ConnectionFailure { .. } => break,
                    _ => {}
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(20), future).await {
            Err(_) => panic!("timeout"),
            Ok(()) => {}
        }
    }

    #[tokio::test]
    async fn peer_test_works_ipv4_x25519() {
        peer_test_works(true, true, None).await
    }

    #[tokio::test]
    async fn peer_test_works_ipv4_ml_kem_512() {
        peer_test_works(true, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn peer_test_works_ipv4_ml_kem_768() {
        peer_test_works(true, false, Some("4,3".to_string())).await
    }

    #[tokio::test]
    async fn peer_test_works_ipv6_x25519() {
        peer_test_works(false, true, None).await
    }

    #[tokio::test]
    async fn peer_test_works_ipv6_ml_kem_512() {
        peer_test_works(false, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn peer_test_works_ipv6_ml_kem_768() {
        peer_test_works(false, false, Some("4,3".to_string())).await
    }

    async fn peer_test_works(ipv4: bool, disable_pq: bool, ml_kem: Option<String>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (ctx1, address1_ipv4, address1_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();
        let (ctx2, address2_ipv4, address2_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();
        let (ctx3, address3_ipv4, address3_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xee; 32],
                intro_key: [0xff; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static1, signing1) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static2, signing2) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static3, signing3) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let router_info1 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address1_ipv4,
            address1_ipv6,
            &static1,
            &signing1,
            false,
        );
        let router_info2 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address2_ipv4,
            address2_ipv6,
            &static2,
            &signing2,
            false,
        );
        let router_info3 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address3_ipv4,
            address3_ipv6,
            &static3,
            &signing3,
            false,
        );
        let (event1_tx, _event1_rx) = channel(64);
        let (event2_tx, _event2_rx) = channel(64);
        let (event3_tx, _event3_rx) = channel(64);

        let serialized1 = Bytes::from(router_info1.serialize(&signing1));
        let serialized2 = Bytes::from(router_info2.serialize(&signing2));
        let serialized3 = Bytes::from(router_info3.serialize(&signing3));

        let storage1 = {
            let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
            storage.discover_router(router_info2.clone(), serialized2.clone());
            storage.discover_router(router_info3.clone(), serialized3.clone());

            storage
        };
        let storage2 = {
            let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
            storage.discover_router(router_info3.clone(), serialized3.clone());
            storage.discover_router(router_info1.clone(), serialized1.clone());

            storage
        };
        let storage3 = {
            let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
            storage.discover_router(router_info2.clone(), serialized2.clone());
            storage.discover_router(router_info1.clone(), serialized1.clone());

            storage
        };

        let mut transport1 = Ssu2Transport::<MockRuntime>::new(
            ctx1.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                storage1,
                router_info1.identity.id(),
                Bytes::from(router_info1.serialize(&signing1)),
                static1,
                signing1,
                5u8,
                event_handle.clone(),
            ),
            event1_tx,
        );
        let mut transport2 = Ssu2Transport::<MockRuntime>::new(
            ctx2.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                storage2,
                router_info2.identity.id(),
                Bytes::from(router_info2.serialize(&signing2)),
                static2,
                signing2.clone(),
                5u8,
                event_handle.clone(),
            ),
            event2_tx,
        );
        let mut transport3 = Ssu2Transport::<MockRuntime>::new(
            ctx3.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                storage3,
                router_info3.identity.id(),
                Bytes::from(router_info3.serialize(&signing3)),
                static3,
                signing3,
                5u8,
                event_handle.clone(),
            ),
            event3_tx,
        );
        let router_info2 =
            RouterInfo::parse::<MockRuntime>(router_info2.serialize(&signing2)).unwrap();

        // spawn the first router in the background
        tokio::spawn(async move {
            while let Some(event) = transport2.next().await {
                match event {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport2.accept(&router_id);
                    }
                    _ => {}
                }
            }
        });

        // connect the routers together and ensure connection works
        transport1.connect(router_info2.clone());
        let future = async {
            loop {
                match transport1.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id,
                        address,
                        encryption,
                        ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport1.accept(&router_id);
                        break encryption;
                    }
                    _ => {}
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(20), future).await {
            Err(_) => panic!("timeout"),
            Ok(encryption) => match encryption {
                EncryptionKind::X25519 => assert!(disable_pq && ml_kem.is_none()),
                EncryptionKind::MlKem512X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "3,4".to_string()),
                EncryptionKind::MlKem768X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "4,3".to_string()),
                EncryptionKind::MlKem1024X25519 => unreachable!(),
            },
        }

        // spawn the second router in the background
        tokio::spawn(async move {
            while let Some(event) = transport1.next().await {
                match event {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport1.accept(&router_id);
                    }
                    _ => {}
                }
            }
        });

        // connect the third router to router2 which also starts a peer test process
        transport3.connect(router_info2);
        let future = async {
            loop {
                match transport3.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport3.accept(&router_id);
                        break;
                    }
                    _ => {}
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(20), future).await {
            Err(_) => panic!("timeout"),
            Ok(()) => {}
        }

        let future = async move {
            loop {
                match transport3.next().await.unwrap() {
                    TransportEvent::FirewallStatus {
                        status,
                        ipv4: transport,
                    } => {
                        assert_eq!(status, FirewallStatus::Ok);
                        assert_eq!(transport, ipv4);
                        break;
                    }
                    _ => {}
                }
            }
        };
        match tokio::time::timeout(Duration::from_secs(20), future).await {
            Err(_) => panic!("timeout"),
            Ok(()) => {}
        }
    }

    #[tokio::test]
    async fn relay_works_ipv4_x25519() {
        relay_works(true, true, None).await
    }

    #[tokio::test]
    async fn relay_works_ipv4_ml_kem_512() {
        relay_works(true, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn relay_works_ipv4_ml_kem_768() {
        relay_works(true, false, Some("4,3".to_string())).await
    }

    #[tokio::test]
    async fn relay_works_ipv6_x25519() {
        relay_works(false, true, None).await
    }

    #[tokio::test]
    async fn relay_works_ipv6_ml_kem_512() {
        relay_works(false, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn relay_works_ipv6_ml_kem_768() {
        relay_works(false, false, Some("4,3".to_string())).await
    }

    async fn relay_works(ipv4: bool, disable_pq: bool, ml_kem: Option<String>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));

        // router that is behind firewall
        let (ctx1, address1_ipv4, address1_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: false,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        // set ssu2 as firewalled, causing router1 to request relay tag from router2
        let mut ctx1 = ctx1.unwrap();
        ctx1.firewalled = true;

        // introducer for router1
        let (ctx2, address2_ipv4, address2_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static1, signing1) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static2, signing2) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let mut router_info1 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address1_ipv4,
            address1_ipv6,
            &static1,
            &signing1,
            false,
        );
        let router_info2 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address2_ipv4,
            address2_ipv6,
            &static2,
            &signing2,
            false,
        );
        let charlie = router_info1.identity.id();
        let bob = router_info2.identity.id();
        let (event1_tx, event1_rx) = channel(64);
        let (event2_tx, _event2_rx) = channel(64);

        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        storage.add_router(router_info2.clone());
        let mut transport1 = Ssu2Transport::<MockRuntime>::new(
            ctx1,
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                storage,
                router_info1.identity.id(),
                Bytes::from(router_info1.serialize(&signing1)),
                static1,
                signing1.clone(),
                2u8,
                event_handle.clone(),
            ),
            event1_tx,
        );
        let mut transport2 = Ssu2Transport::<MockRuntime>::new(
            ctx2.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info2.identity.id(),
                Bytes::from(router_info2.serialize(&signing2)),
                static2,
                signing2.clone(),
                2u8,
                event_handle.clone(),
            ),
            event2_tx,
        );
        let router_info2 =
            RouterInfo::parse::<MockRuntime>(router_info2.serialize(&signing2)).unwrap();

        tokio::spawn(async move {
            loop {
                match transport2.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport2.accept(&router_id);
                    }
                    _ => {}
                }
            }
        });

        transport1.connect(router_info2.clone());

        let (relay_tag, introducer, expires) = loop {
            tokio::select! {
                event = transport1.next() => match event {
                    Some(TransportEvent::ConnectionEstablished { router_id, address, .. }) => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport1.accept(&router_id);
                    }
                    Some(TransportEvent::IntroducerAdded { relay_tag, router_id, expires,ipv4: transport }) => {
                        assert_eq!(transport, ipv4);
                        break (relay_tag, router_id, expires)
                    }
                    _ => {}
                },
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
            }
        };

        // spawn router1 in the background
        let handle = tokio::spawn(async move {
            let mut routers =
                HashMap::<RouterId, Sender<OutboundMessage, OutboundMessageRecycle>>::new();

            loop {
                tokio::select! {
                    event = transport1.next() => match event.unwrap() {
                        TransportEvent::ConnectionEstablished { router_id, address, .. } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                            transport1.accept(&router_id);
                        }
                        _ => {}
                    },
                    event = event1_rx.recv() => match event.unwrap() {
                        SubsystemEvent::ConnectionEstablished { router_id: router, tx } => {
                            routers.insert(router, tx);
                        }
                        SubsystemEvent::Message { messages } => {
                            if messages.iter().any(|(_, message)| {
                                message.message_type == MessageType::DatabaseStore
                                && message.message_id == 1337
                                && message.payload == vec![1,3,3,7]
                            }) {
                                break
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        // modify router1's info to contain router2 as an introducer
        let router_info1 = {
            let (introducers, options) = if ipv4 {
                let Some(RouterAddress::Ssu2 {
                    introducers,
                    options,
                    ..
                }) = router_info1.ssu2_ipv4_mut()
                else {
                    panic!("should exist");
                };

                (introducers, options)
            } else {
                let Some(RouterAddress::Ssu2 {
                    introducers,
                    options,
                    ..
                }) = router_info1.ssu2_ipv6_mut()
                else {
                    panic!("should exist");
                };

                (introducers, options)
            };

            introducers.push((introducer.clone(), relay_tag));
            options.insert(Str::from("iexp0"), Str::from(expires.as_secs().to_string()));
            options.insert(Str::from("itag0"), Str::from(relay_tag.to_string()));
            options.insert(
                Str::from("ih0"),
                Str::from(base64_encode(introducer.to_vec())),
            );
            let router_info1 = router_info1.serialize(&signing1);

            RouterInfo::parse::<MockRuntime>(&router_info1).unwrap()
        };

        // create third router which connects to router1 with the help of router2
        let (ctx3, address3_ipv4, address3_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xee; 32],
                intro_key: [0xff; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static3, signing3) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let router_info3 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address3_ipv4,
            address3_ipv6,
            &static3,
            &signing3,
            false,
        );
        let (event3_tx, event3_rx) = channel(64);
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        storage.add_router(router_info2.clone());

        let mut transport3 = Ssu2Transport::<MockRuntime>::new(
            ctx3.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                storage,
                router_info3.identity.id(),
                Bytes::from(router_info3.serialize(&signing3)),
                static3,
                signing3,
                2u8,
                event_handle.clone(),
            ),
            event3_tx,
        );

        // connect router2 and router3 together
        transport3.connect(router_info2);

        loop {
            tokio::select! {
                event = transport3.next() => match event {
                    Some(TransportEvent::ConnectionEstablished { router_id, address, encryption, .. }) => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport3.accept(&router_id);

                        match encryption {
                            EncryptionKind::X25519 => assert!(disable_pq && ml_kem.is_none()),
                            EncryptionKind::MlKem512X25519 =>
                                assert!(!disable_pq && *ml_kem.as_ref().unwrap() == "3,4".to_string()),
                            EncryptionKind::MlKem768X25519 =>
                                assert!(!disable_pq && *ml_kem.as_ref().unwrap() == "4,3".to_string()),
                            EncryptionKind::MlKem1024X25519 => unreachable!(),
                        }

                        break;
                    }
                    _ => {}
                },
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
            }
        }
        // ignore first connct
        let _tx = match timeout!(event3_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionEstablished {
                tx,
                router_id: router,
            } => {
                assert_eq!(router, bob);
                tx
            }
            _ => panic!("invalid event"),
        };

        // connect to router1 with the help of router2
        transport3.connect(router_info1);

        while let Some(event) = transport3.next().await {
            match event {
                TransportEvent::ConnectionEstablished {
                    router_id,
                    address,
                    encryption,
                    ..
                } => {
                    assert_eq!(address.is_ipv4(), ipv4);
                    transport3.accept(&router_id);

                    match encryption {
                        EncryptionKind::X25519 => assert!(disable_pq && ml_kem.is_none()),
                        EncryptionKind::MlKem512X25519 =>
                            assert!(!disable_pq && ml_kem.unwrap() == "3,4".to_string()),
                        EncryptionKind::MlKem768X25519 =>
                            assert!(!disable_pq && ml_kem.unwrap() == "4,3".to_string()),
                        EncryptionKind::MlKem1024X25519 => unreachable!(),
                    }

                    break;
                }
                _ => {}
            }
        }

        let tx = match timeout!(event3_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionEstablished {
                tx,
                router_id: router,
            } => {
                assert_eq!(router, charlie);
                tx
            }
            _ => panic!("invalid event"),
        };

        // send message from alice to charlie
        tx.send(OutboundMessage::Message(Message {
            message_type: MessageType::DatabaseStore,
            message_id: 1337,
            expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
            payload: vec![1, 3, 3, 7],
        }))
        .await
        .unwrap();

        // verify charlie receives the message
        let _ = timeout!(handle).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv4_x25519() {
        fragmented_router_info(true, true, None).await
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv4_ml_kem_512() {
        fragmented_router_info(true, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv4_ml_kem_768() {
        fragmented_router_info(true, false, Some("4,3".to_string())).await
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv6_x25519() {
        fragmented_router_info(false, true, None).await
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv6_ml_kem_512() {
        fragmented_router_info(false, false, Some("3,4".to_string())).await
    }

    #[tokio::test]
    async fn fragmented_router_info_ipv6_ml_kem_768() {
        fragmented_router_info(false, false, Some("4,3".to_string())).await
    }

    async fn fragmented_router_info(ipv4: bool, disable_pq: bool, ml_kem: Option<String>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (ctx1, address1_ipv4, address1_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();
        let (ctx2, address2_ipv4, address2_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: ml_kem.clone(),
                disable_pq,
                port: 0u16,
                ipv4_host: ipv4.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: (!ipv4).then_some("::1".parse().unwrap()),
                ipv4,
                ipv6: !ipv4,
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
                ipv4_mtu: None,
                ipv6_mtu: None,
            }))
            .await
            .unwrap();

        let (static1, signing1) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let (static2, signing2) = (
            StaticPrivateKey::random(MockRuntime::rng()),
            SigningPrivateKey::random(MockRuntime::rng()),
        );
        let mut router_info1 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address1_ipv4,
            address1_ipv6,
            &static1,
            &signing1,
            false,
        );

        // add random garbage to router info options so it gets fragmented
        for i in 0..10 {
            router_info1.options.insert(
                Str::from(format!("garbage{i}")),
                Str::from(base64_encode(vec![0xaa; 128])),
            );
        }
        assert!(router_info1.serialize(&signing1).len() > 1500);

        let router_info2 = RouterInfo::new::<MockRuntime>(
            &Default::default(),
            None,
            None,
            address2_ipv4,
            address2_ipv6,
            &static2,
            &signing2,
            false,
        );
        let (event1_tx, _event1_rx) = channel(64);
        let (event2_tx, _event2_rx) = channel(64);

        let mut transport1 = Ssu2Transport::<MockRuntime>::new(
            ctx1.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info1.identity.id(),
                Bytes::from(router_info1.serialize(&signing1)),
                static1,
                signing1,
                2u8,
                event_handle.clone(),
            ),
            event1_tx,
        );
        let mut transport2 = Ssu2Transport::<MockRuntime>::new(
            ctx2.unwrap(),
            true,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                router_info2.identity.id(),
                Bytes::from(router_info2.serialize(&signing2)),
                static2,
                signing2.clone(),
                2u8,
                event_handle.clone(),
            ),
            event2_tx,
        );
        let router_info2 =
            RouterInfo::parse::<MockRuntime>(router_info2.serialize(&signing2)).unwrap();
        tokio::spawn(async move {
            loop {
                match transport2.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id, address, ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport2.accept(&router_id)
                    }
                    _ => {}
                }
            }
        });

        transport1.connect(router_info2);
        let future = async move {
            loop {
                match transport1.next().await.unwrap() {
                    TransportEvent::ConnectionEstablished {
                        router_id,
                        address,
                        encryption,
                        ..
                    } => {
                        assert_eq!(address.is_ipv4(), ipv4);
                        transport1.accept(&router_id);
                        break encryption;
                    }
                    _ => {}
                }
            }
        };

        match tokio::time::timeout(Duration::from_secs(15), future).await {
            Err(_) => panic!("timeout"),
            Ok(encryption) => match encryption {
                EncryptionKind::X25519 => assert!(disable_pq && ml_kem.is_none()),
                EncryptionKind::MlKem512X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "3,4".to_string()),
                EncryptionKind::MlKem768X25519 =>
                    assert!(!disable_pq && ml_kem.unwrap() == "4,3".to_string()),
                EncryptionKind::MlKem1024X25519 => unreachable!(),
            },
        }
    }

    #[tokio::test]
    async fn too_small_ipv4_mtu() {
        let (ctx, address_ipv4, address_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: None,
                disable_pq: false,
                port: 0u16,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: Some("::1".parse().unwrap()),
                ipv4: true,
                ipv6: true,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: Some(512),
                ipv6_mtu: Some(1500),
            }))
            .await
            .unwrap();

        assert!(ctx.is_some());
        assert!(address_ipv4.is_none());
        assert!(address_ipv6.is_some());
    }

    #[tokio::test]
    async fn too_small_ipv6_mtu() {
        let (ctx, address_ipv4, address_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: None,
                disable_pq: false,
                port: 0u16,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: Some("::1".parse().unwrap()),
                ipv4: true,
                ipv6: true,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: Some(1500),
                ipv6_mtu: Some(512),
            }))
            .await
            .unwrap();

        assert!(ctx.is_some());
        assert!(address_ipv4.is_some());
        assert!(address_ipv6.is_none());
    }

    #[tokio::test]
    async fn too_small_ipv4_and_ipv6_mtu() {
        let (ctx, address_ipv4, address_ipv6) =
            Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
                ml_kem: None,
                disable_pq: false,
                port: 0u16,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: Some("::1".parse().unwrap()),
                ipv4: true,
                ipv6: true,
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
                ipv4_mtu: Some(1024),
                ipv6_mtu: Some(512),
            }))
            .await
            .unwrap();

        assert!(ctx.is_none());
        assert!(address_ipv4.is_none());
        assert!(address_ipv6.is_none());
    }
}
