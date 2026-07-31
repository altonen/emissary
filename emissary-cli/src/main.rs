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

#![allow(clippy::crate_in_macro_def)]
#![allow(clippy::too_many_arguments)]

use crate::{
    address_book::{AddressBookHandle, AddressBookManager},
    cli::Arguments,
    config::{Config, EmissaryConfig, ReseedConfig, RouterUiConfig},
    error::Error,
    proxy::{http::HttpProxy, socks::SocksProxy},
    tunnel::{client::ClientTunnelManager, server::ServerTunnelManager},
};

use anyhow::anyhow;
use clap::Parser;
use emissary_core::{
    events::EventSubscriber,
    primitives::RouterId,
    router::Router,
    runtime::{AddressBook, Runtime},
};
use emissary_util::{
    port_mapper::PortMapper, reseeder::Reseeder, runtime::tokio::Runtime as TokioRuntime,
    storage::Storage, su3::ReseedRouterInfo,
};
use futures::{channel::oneshot, StreamExt};
use tokio::sync::mpsc::{channel, Receiver};

use std::{fs::File, io::Write, mem, path::PathBuf, sync::Arc};

mod address_book;
mod cli;
mod config;
mod error;
#[cfg(feature = "i2pcontrol")]
mod i2pcontrol;
mod logger;
mod proxy;
mod tools;
mod tunnel;
mod ui;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary";

/// Result type for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Router context.
struct RouterContext {
    /// Router.
    router: Router<TokioRuntime>,

    /// Base path.
    #[allow(unused)]
    base_path: PathBuf,

    /// Local router ID.
    #[allow(unused)]
    router_id: RouterId,

    /// Event subscriber.
    ///
    /// Passed onto a router UI if it has been enabled.
    #[allow(unused)]
    events: EventSubscriber,

    /// Router configuration.
    #[allow(unused)]
    config: EmissaryConfig,

    /// Address book handle, if address book was enabled.
    #[allow(unused)]
    address_book_handle: Option<Arc<AddressBookHandle>>,

    /// Port mapper for NAT-PMP and UPnP.
    port_mapper: PortMapper,

    /// Router UI config, if enabled.
    #[allow(unused)]
    router_ui_config: Option<RouterUiConfig>,

    /// I2PControl shutdown sender.
    #[cfg(feature = "i2pcontrol")]
    #[allow(unused)]
    i2pcontrol_shutdown: tokio::sync::broadcast::Sender<()>,
}

/// Parse `Arguments` and if no subcommand has been specified, return `Arguments`, allowing the
/// caller to setup the router.
///
/// If subcommand has been specified, execute the command and exit.
async fn parse_arguments() -> Arguments {
    let arguments = Arguments::parse();

    match arguments.command {
        Some(command) => command.execute().await,
        None => arguments,
    }
}

/// Setup router and related subsystems.
async fn setup_router<R: Runtime>(arguments: Arguments) -> anyhow::Result<RouterContext> {
    // initialize logger with any logging directive given as a cli argument
    #[cfg_attr(not(feature = "i2pcontrol"), allow(unused_variables))]
    let (handle, log_ring) = init_logger!(arguments.log.clone());

    // initialize storage for the router
    let storage = Storage::new::<R>(arguments.base_path.clone()).await?;

    // parse router config and merge it with cli options
    let mut config = Config::parse::<R>(&arguments, &storage).await.map_err(|error| {
        tracing::warn!(
            target: LOG_TARGET,
            ?error,
            "invalid router config, pass `--overwrite-config` to create new config",
        );

        error
    })?;

    // reinitialize the logger with any directives given in the configuration file
    init_logger!(config.log.clone(), handle);

    // is the # of known routers less than reseed threshold or is reseed forced
    let should_reseed = config.reseed.as_ref().is_some_and(
        |ReseedConfig {
             reseed_threshold, ..
         }| reseed_threshold > &config.routers.len(),
    ) || arguments.reseed.force_reseed.unwrap_or(false);

    if should_reseed {
        tracing::info!(
            target: LOG_TARGET,
            num_routers = ?config.routers.len(),
            forced_reseed = ?arguments.reseed.force_reseed.unwrap_or(false),
            force_ipv4 = ?(!arguments.reseed.disable_force_ipv4.unwrap_or(false)),
            "reseed router"
        );

        match Reseeder::reseed::<R>(
            config.reseed.as_ref().and_then(|config| config.hosts.clone()),
            !arguments.reseed.disable_force_ipv4.unwrap_or(false),
        )
        .await
        {
            Ok(routers) => {
                tracing::info!(
                    target: LOG_TARGET,
                    num_routers = ?routers.len(),
                    "router reseeded",
                );

                for ReseedRouterInfo { name, router_info } in routers {
                    if let Err(error) = storage.store_router_info(name, router_info.clone()).await {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to store router info to disk",
                        );
                    }
                    config.routers.push(router_info);
                }
            }
            Err(error) if config.routers.is_empty() => {
                tracing::error!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to reseed and no routers available",
                );
                return Err(anyhow!("no routers available"));
            }
            Err(error) => tracing::warn!(
                target: LOG_TARGET,
                ?error,
                "failed to reseed, trying to start router anyway",
            ),
        }
    }

    let path = config.base_path.clone();
    let http = config.http_proxy.take();
    let socks = config.socks_proxy.take();
    let port_forwarding = config.port_forwarding.take();
    let client_tunnels = mem::take(&mut config.client_tunnels);
    let client_tunnel_options = mem::take(&mut config.client_tunnel_options);
    let server_tunnels = mem::take(&mut config.server_tunnels);
    let router_ui_config = config.router_ui.clone();
    let router_config = config.config.take().expect("to exist");
    let base_path = config.base_path.clone();

    let (router, events, local_router_info, address_book_manager) =
        match config.address_book.take() {
            None => Router::<TokioRuntime>::new(config.into(), None, Some(Arc::new(storage)))
                .await
                .map(|(router, event_subscriber, info)| (router, event_subscriber, info, None)),

            Some(address_book_config) => {
                // create address book, allocate address book handle and pass it to `Router`
                let address_book_manager =
                    AddressBookManager::new(config.base_path.clone(), address_book_config).await;
                let address_book_handle = address_book_manager.handle();

                Router::<TokioRuntime>::new(
                    config.into(),
                    Some(address_book_handle),
                    Some(Arc::new(storage)),
                )
                .await
                .map(|(router, event_subscriber, info)| {
                    (router, event_subscriber, info, Some(address_book_manager))
                })
            }
        }
        .map_err(|error| anyhow!(error))?;

    // save newest router info to disk
    File::create(path.join("router.info"))?.write_all(&local_router_info)?;

    // Create the passive client-service registry in the application
    // composition root. Producers (proxy tasks, listener snapshot
    // readouts) and the I2PControl state share clones of the same
    // registry through `Arc`, so observations emitted from the spawn
    // sites become visible to `ClientServicesInfo` immediately.
    #[cfg(feature = "i2pcontrol")]
    let service_registry = i2pcontrol::service_registry::ServiceRegistry::new();

    // Record I2CP and SAM listener state from the actual bound
    // addresses produced by core router startup. This is a single
    // passive observation at composition time; the registry continues
    // to be observed by proxy tasks below.
    #[cfg(feature = "i2pcontrol")]
    {
        let info = router.protocol_address_info();
        i2pcontrol::observers::observe_i2cp_listener(&service_registry, info.i2cp);
        i2pcontrol::observers::observe_sam_listener(&service_registry, info.sam_tcp, info.sam_udp);
    }

    // if sam was enabled, start all enabled proxies, client tunnels and the address book
    let address_book_handle = if let Some(address) = router.protocol_address_info().sam_tcp {
        // start http proxy if it was enabled
        let address_book_handle = if let Some(config) = http {
            // start event loop of address book manager if address book was enabled
            //
            // address book depends on the http proxy as it downloads hosts.txt from inside i2p
            //
            // if address book is enabled, create oneshot channel pair, pass the receiver to address
            // book and sender to http proxy and once the http proxy is ready (its tunnel pool has
            // been built), it'll signal the address book that it can start download hosts file(s)
            //
            // additionally, acquire handle to address book which is passed to http proxy so it can
            // resolve .i2p hosts to .b32.i2p hosts
            let (http_proxy_ready_tx, address_book_handle) = match address_book_manager {
                None => (None, None),
                Some(address_book_manager) => {
                    let (tx, rx) = oneshot::channel();
                    let handle = address_book_manager.handle();

                    tokio::spawn(address_book_manager.run(config.port, config.host.clone(), rx));

                    (Some(tx), Some(handle))
                }
            };

            // start event loop of http proxy
            let handle = address_book_handle.clone();

            // Spawn a passive observer that marks Starting now and
            // Stopped when the proxy task exits. The composition root
            // owns the observer registry clone; the spawned task owns
            // its own handle for the same category.
            #[cfg(feature = "i2pcontrol")]
            let http_observer_handle =
                i2pcontrol::observers::spawn_http_observer(&service_registry, true);
            #[cfg(feature = "i2pcontrol")]
            let _stop_guard = Arc::new(i2pcontrol::service_registry::ServiceUpdateHandle::clone(
                &http_observer_handle,
            ));

            let http_proxy_fut = HttpProxy::new(
                config,
                address.port(),
                http_proxy_ready_tx,
                handle.map(|handle| handle as Arc<dyn AddressBook>),
            );

            tokio::spawn(async move {
                match http_proxy_fut.await {
                    Ok(proxy) => {
                        // Record Listening transition now that the proxy
                        // has a bound address. The observer is purely
                        // passive; the proxy lifecycle is unchanged.
                        #[cfg(feature = "i2pcontrol")]
                        i2pcontrol::observers::observe_http_listening(
                            &http_observer_handle,
                            proxy.local_addr(),
                        );
                        if let Err(error) = proxy.run().await {
                            tracing::debug!(
                                target: LOG_TARGET,
                                ?error,
                                "http proxy exited",
                            );
                        }
                    }
                    Err(error) => {
                        let error_for_observer = anyhow::Error::from(error);
                        #[cfg(feature = "i2pcontrol")]
                        i2pcontrol::observers::observe_proxy_failure(
                            &http_observer_handle,
                            &error_for_observer,
                        );
                        tracing::warn!(
                            target: LOG_TARGET,
                            error = %error_for_observer,
                            "failed to start http proxy",
                        );
                    }
                }
            });

            address_book_handle
        } else {
            None
        };

        // start socks proxy if it was enabled
        if let Some(config) = socks {
            // start event loop of socks proxy
            #[cfg(feature = "i2pcontrol")]
            let socks_observer_handle =
                i2pcontrol::observers::spawn_socks_observer(&service_registry, true);
            #[cfg(feature = "i2pcontrol")]
            let _stop_guard = Arc::new(i2pcontrol::service_registry::ServiceUpdateHandle::clone(
                &socks_observer_handle,
            ));

            let socks_proxy_fut = SocksProxy::new(config, address.port());

            tokio::spawn(async move {
                match socks_proxy_fut.await {
                    Ok(proxy) => {
                        #[cfg(feature = "i2pcontrol")]
                        i2pcontrol::observers::observe_socks_listening(
                            &socks_observer_handle,
                            proxy.local_addr(),
                        );
                        if let Err(error) = proxy.run().await {
                            tracing::debug!(
                                target: LOG_TARGET,
                                ?error,
                                "socks proxy exited",
                            );
                        }
                    }
                    Err(error) => {
                        let error_for_observer = anyhow::Error::from(error);
                        #[cfg(feature = "i2pcontrol")]
                        i2pcontrol::observers::observe_proxy_failure(
                            &socks_observer_handle,
                            &error_for_observer,
                        );
                        tracing::warn!(
                            target: LOG_TARGET,
                            error = %error_for_observer,
                            "failed to start socks proxy",
                        );
                    }
                }
            });
        }

        // start client and server tunnels
        tokio::spawn(
            ClientTunnelManager::new(client_tunnels, client_tunnel_options, address.port()).run(),
        );
        tokio::spawn(
            ServerTunnelManager::new(server_tunnels, address.port(), path.clone())
                .await
                .run(),
        );

        address_book_handle
    } else {
        None
    };

    // create port mapper from config and transport protocol info
    //
    // `PortMapper` can be polled for external address discoveries
    let port_mapper = PortMapper::new(
        port_forwarding,
        router.protocol_address_info().ntcp2_port,
        router.protocol_address_info().ssu2_port,
    );

    // Start I2PControl server if enabled (independent of UI mode)
    //
    // init_server performs validation, TLS setup, and port binding synchronously
    // so that failures are surfaced as startup errors via setup_router.
    #[cfg(feature = "i2pcontrol")]
    let i2pcontrol_shutdown = {
        let (i2pcontrol_shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        if let Some(ref i2pcontrol_config) = router_config.i2pcontrol {
            if i2pcontrol_config.enabled {
                let bind: std::net::SocketAddr = i2pcontrol_config
                    .bind
                    .parse()
                    .map_err(|e| anyhow!("Invalid I2PControl bind address: {e}"))?;
                let server_config = i2pcontrol::server::I2pControlConfig {
                    enabled: true,
                    bind,
                    password: i2pcontrol_config.password.clone(),
                    tls: i2pcontrol::tls::TlsConfig {
                        certificate: i2pcontrol_config
                            .certificate
                            .as_ref()
                            .map(std::path::PathBuf::from),
                        private_key: i2pcontrol_config
                            .private_key
                            .as_ref()
                            .map(std::path::PathBuf::from),
                    },
                };

                let share_ratio =
                    router_config.bandwidth.as_ref().map(|b| b.share_ratio).unwrap_or(0.0);
                let (bw_in, bw_out) = router_config
                    .bandwidth
                    .as_ref()
                    .map(|b| (b.bandwidth as u64, b.bandwidth as u64))
                    .unwrap_or((0, 0));
                let metrics = Arc::new(i2pcontrol::production::EventHandleMetrics::new(
                    router.event_handle().clone(),
                ));

                // Build a clone of the registry to hand to the I2PControl
                // state. The composition-root clone continues to be used
                // by the proxy tasks and listener snapshot tasks already
                // spawned above; producer handles on that clone target the
                // same backing storage as the clone held by I2pControlState.
                let registry_for_i2pcontrol = service_registry.clone();

                let mut ctx = i2pcontrol::server::ServerInitContext::new(
                    router.router_id().to_base64().to_owned(),
                    local_router_info.clone(),
                )
                .with_event_metrics(metrics)
                .with_share_ratio(share_ratio)
                .with_configured_bandwidth(bw_in, bw_out)
                .with_service_registry(registry_for_i2pcontrol)
                .with_sam_listener_enabled(router.protocol_address_info().sam_tcp.is_some())
                .with_log_ring(log_ring.expect("I2PControl logger ring is initialized"));

                if let Some(handle) = router.sam_session_observation_handle() {
                    ctx = ctx.with_sam_session_observation(handle);
                }

                let instance =
                    i2pcontrol::server::init_server(&server_config, &base_path, ctx).await?;

                let shutdown_tx = i2pcontrol_shutdown_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        i2pcontrol::server::serve(instance, shutdown_tx.subscribe()).await
                    {
                        tracing::error!(
                            target: LOG_TARGET,
                            ?e,
                            "I2PControl server exited with error",
                        );
                    }
                });

                tracing::info!(
                    target: LOG_TARGET,
                    "I2PControl server started",
                );
            }
        }
        i2pcontrol_shutdown_tx
    };

    Ok(RouterContext {
        address_book_handle,
        base_path,
        config: router_config,
        events,
        port_mapper,
        router_id: router.router_id().clone(),
        router,
        router_ui_config,
        #[cfg(feature = "i2pcontrol")]
        i2pcontrol_shutdown,
    })
}

/// Run the event loop of `emissary-cli`
///
/// Start a loop which polls:
///  * `SIGINT` signal handler
///  * `Router`'s event loop
///  * [`PortMapper`]'s event loop
///  * RX channel for receiving a shutdown signal from router UI
#[cfg(not(feature = "i2pcontrol"))]
async fn router_event_loop(
    mut router: Router<TokioRuntime>,
    mut port_mapper: PortMapper,
    mut shutdown_rx: Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                port_mapper.shutdown().await;
                router.shutdown();
            }
            _ = shutdown_rx.recv() => {
                port_mapper.shutdown().await;
                router.shutdown();
            }
            address = port_mapper.next() => {
                // the value must exist since the stream never terminates
                router.add_external_address(address.expect("value"));
            },
            _ = &mut router => {
                tracing::info!(
                    target: LOG_TARGET,
                    "emissary shut down",
                );
                break;
            }
        }
    }
}

/// Run the event loop of `emissary-cli` with I2PControl shutdown support.
///
/// Sends the I2PControl shutdown signal when the application shuts down,
/// ensuring the server receives structured cancellation and executes
/// token cleanup.
#[cfg(feature = "i2pcontrol")]
async fn router_event_loop(
    mut router: Router<TokioRuntime>,
    mut port_mapper: PortMapper,
    mut shutdown_rx: Receiver<()>,
    i2pcontrol_shutdown: tokio::sync::broadcast::Sender<()>,
) {
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = i2pcontrol_shutdown.send(());
                port_mapper.shutdown().await;
                router.shutdown();
            }
            _ = shutdown_rx.recv() => {
                let _ = i2pcontrol_shutdown.send(());
                port_mapper.shutdown().await;
                router.shutdown();
            }
            address = port_mapper.next() => {
                // the value must exist since the stream never terminates
                router.add_external_address(address.expect("value"));
            },
            _ = &mut router => {
                tracing::info!(
                    target: LOG_TARGET,
                    "emissary shut down",
                );
                break;
            }
        }
    }
}

#[cfg(not(feature = "ui"))]
fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let (_tx, shutdown_rx) = channel(1);
    let arguments = runtime.block_on(parse_arguments());
    let RouterContext {
        port_mapper,
        router,
        #[cfg(feature = "i2pcontrol")]
        i2pcontrol_shutdown,
        ..
    } = runtime.block_on(setup_router::<TokioRuntime>(arguments))?;

    #[cfg(feature = "i2pcontrol")]
    runtime.block_on(router_event_loop(
        router,
        port_mapper,
        shutdown_rx,
        i2pcontrol_shutdown,
    ));
    #[cfg(not(feature = "i2pcontrol"))]
    runtime.block_on(router_event_loop(router, port_mapper, shutdown_rx));

    Ok(())
}

#[cfg(feature = "ui")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = channel(1);
    let arguments = parse_arguments().await;
    let RouterContext {
        router,
        port_mapper,
        events,
        router_ui_config,
        config,
        base_path,
        address_book_handle,
        router_id,
        #[cfg(feature = "i2pcontrol")]
        i2pcontrol_shutdown,
    } = setup_router::<TokioRuntime>(arguments).await?;

    match router_ui_config {
        None => {
            #[cfg(feature = "i2pcontrol")]
            router_event_loop(router, port_mapper, shutdown_rx, i2pcontrol_shutdown).await;
            #[cfg(not(feature = "i2pcontrol"))]
            router_event_loop(router, port_mapper, shutdown_rx).await;
        }
        Some(RouterUiConfig { native, .. }) => {
            #[cfg(feature = "i2pcontrol")]
            {
                let ics = i2pcontrol_shutdown;
                tokio::spawn(async move {
                    router_event_loop(router, port_mapper, shutdown_rx, ics).await;
                    std::process::exit(0);
                });
            }
            #[cfg(not(feature = "i2pcontrol"))]
            tokio::spawn(async move {
                router_event_loop(router, port_mapper, shutdown_rx).await;
                std::process::exit(0);
            });
            ui::dioxus::start(
                events,
                config,
                base_path,
                address_book_handle,
                router_id,
                shutdown_tx,
                !native.unwrap_or(false),
            )
            .await;
        }
    }

    Ok(())
}
