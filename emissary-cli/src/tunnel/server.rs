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

use crate::config::{I2cpOptions, ServerTunnelConfig};

use yosemite::{style, DestinationKind, RouterApi, Session, SessionOptions};

use std::{path::PathBuf, sync::Arc, time::Duration};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::server-tunnel";

/// Number of destination generation retries.
const DESTINATION_CREATION_RETRY_COUNT: usize = 3usize;

/// Destination generation failure backoff.
const DESTINATION_CREATION_BACKOFF: Duration = Duration::from_secs(10);

/// Backoff for `STREAM FORWARD` failure.
const STREAM_FORWARD_BACKOFF: Duration = Duration::from_secs(10);

/// Server tunnel configuration
pub struct TunnelConfig {
    /// Base64 destination.
    destination: String,

    /// I2CP options.
    i2cp: Option<I2cpOptions>,

    /// Name of the tunnel.
    name: String,

    /// Server port.
    port: u16,

    /// SAMv3 TCP port.
    sam_tcp_port: u16,
}

/// Server tunnel manager.
pub struct ServerTunnelManager {
    /// Server tunnels.
    tunnels: Vec<Arc<TunnelConfig>>,
}

impl ServerTunnelManager {
    /// Create new [`ServerTunnelManager`].
    pub async fn new(
        configs: Vec<ServerTunnelConfig>,
        sam_tcp_port: u16,
        base_path: PathBuf,
    ) -> Self {
        let mut tunnels = Vec::<Arc<TunnelConfig>>::new();
        let mut router_api = RouterApi::new(sam_tcp_port);

        for ServerTunnelConfig {
            name,
            port,
            destination_path,
            i2cp,
            ..
        } in configs
        {
            match Self::load_or_create_destination(
                &mut router_api,
                base_path.join(&destination_path),
            )
            .await
            {
                None => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        %name,
                        %destination_path,
                        "failed to load or create destination for server tunnel",
                    );
                    continue;
                }
                Some(destination) => {
                    tunnels.push(Arc::from(TunnelConfig {
                        destination,
                        name,
                        port,
                        sam_tcp_port,
                        i2cp: i2cp.clone(),
                    }));
                }
            }
        }

        Self { tunnels }
    }

    /// Attempt to load destination from `path` and if it does't exist, call router over SAMv3 to
    /// create new persistent destination.
    ///
    /// Destination generation is attempted three times before bailing out.
    async fn load_or_create_destination(
        router_api: &mut RouterApi,
        path: PathBuf,
    ) -> Option<String> {
        if let Some(destination) = tokio::fs::read(&path).await.ok().and_then(|contents| {
            std::str::from_utf8(&contents).ok().map(|destination| destination.to_string())
        }) {
            return Some(destination);
        };

        tracing::debug!(
            target: LOG_TARGET,
            ?path,
            "destination not found from disk, create new destination",
        );

        for _ in 0..DESTINATION_CREATION_RETRY_COUNT {
            match router_api.generate_destination().await {
                Ok((_, private_key)) => {
                    if let Err(error) = tokio::fs::write(&path, private_key.as_bytes()).await {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?path,
                            ?error,
                            "failed to write destination to disk",
                        );
                    }

                    return Some(private_key);
                }
                Err(error) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        ?path,
                        ?error,
                        "failed to generate destination",
                    );
                    tokio::time::sleep(DESTINATION_CREATION_BACKOFF).await;
                }
            }
        }

        tracing::warn!(
            target: LOG_TARGET,
            ?path,
            retry_count = ?DESTINATION_CREATION_RETRY_COUNT,
            "failed to generate destination after multiple retries",
        );

        None
    }

    /// Run the event loop of server tunnel.
    async fn server_event_loop(config: Arc<TunnelConfig>) {
        tracing::info!(
            target: LOG_TARGET,
            name = %config.name,
            port = %config.port,
            "starting server tunnel",
        );

        let mut session = match Session::<style::Stream>::new(SessionOptions {
            samv3_tcp_port: config.sam_tcp_port,
            nickname: config.name.clone(),
            silent_forward: true,
            destination: DestinationKind::Persistent {
                private_key: config.destination.clone(),
            },
            lease_set_enc_type: config
                .i2cp
                .as_ref()
                .and_then(|i2cp| i2cp.lease_set_enc_type.clone()),
            ..Default::default()
        })
        .await
        {
            Ok(session) => session,
            Err(error) => {
                tracing::error!(
                    target: LOG_TARGET,
                    name = %config.name,
                    ?error,
                    "failed to start client samv3 session for server tunnel",
                );
                return;
            }
        };

        // send `STREAM FORWARD` command to session and if it fails, sleep and try again later
        loop {
            let Err(error) = session.forward(config.port).await else {
                break;
            };

            tracing::warn!(
                target: LOG_TARGET,
                name = %config.name,
                ?error,
                "failed to forward stream",
            );

            tokio::time::sleep(STREAM_FORWARD_BACKOFF).await;
        }

        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    /// Run the event loop of [`ServerTunnelManager`].
    pub async fn run(self) {
        if self.tunnels.is_empty() {
            return;
        }

        for tunnel in self.tunnels {
            tokio::spawn(Self::server_event_loop(Arc::clone(&tunnel)));
        }
    }
}
