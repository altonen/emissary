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

use crate::config::SocksProxyConfig;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
use yosemite::{style, Session, SessionOptions, StreamOptions};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::proxy::socks";

/// SOCKSv5 TCP CONNECT
const SOCKSV5_TCP: u8 = 0x01;

/// SOCKSv5 Domain for TCP CONNECT.
const SOCKSV5_DOMAIN: u8 = 0x03;

/// SOCKSv5 success.
const SOCKSV5_SUCCESS: u8 = 0x00;

/// SOCKSv5 proxy.
pub struct SocksProxy {
    /// Pending `TCP CONNECT` futures.
    futures: JoinSet<anyhow::Result<(TcpStream, String, u16)>>,

    /// TCP listener for the server.
    listener: TcpListener,

    /// Outproxy, if specified.
    outproxy: Option<String>,

    /// SAMv3 streaming session for the SOCKS proxy.
    session: Session<style::Stream>,
}

impl SocksProxy {
    /// Create new [`SocksProxy`].
    pub async fn new(config: SocksProxyConfig, samv3_tcp_port: u16) -> crate::Result<Self> {
        let session = Session::<style::Stream>::new(SessionOptions {
            publish: false,
            samv3_tcp_port,
            nickname: "socks-proxy".to_string(),
            lease_set_enc_type: config.i2cp.and_then(|config| config.lease_set_enc_type).clone(),
            ..Default::default()
        })
        .await?;
        let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

        Ok(Self {
            futures: JoinSet::new(),
            outproxy: config.outproxy,
            listener,
            session,
        })
    }

    /// Attempt to parse `TCP CONNECT` SOCKSv5 command from `stream` and return the parsed host.
    async fn parse_tcp_connect(mut stream: TcpStream) -> anyhow::Result<(TcpStream, String, u16)> {
        let mut buf = [0u8; 262];
        stream.read_exact(&mut buf[..2]).await?;

        if buf[0] != 0x05 {
            anyhow::bail!("Not SOCKSv5");
        }

        let nmethods = buf[1] as usize;
        stream.read_exact(&mut buf[..nmethods]).await?;

        // version 5, no-auth (0x00)
        stream.write_all(&[0x05, 0x00]).await?;

        // version, cmd, rsv, atyp
        stream.read_exact(&mut buf[..4]).await?;
        let cmd = buf[1];
        let atyp = buf[3];

        if cmd != SOCKSV5_TCP {
            anyhow::bail!("Only TCP CONNECT supported");
        }

        // only domain is supported for i2p
        if atyp != SOCKSV5_DOMAIN {
            anyhow::bail!("Only Domain supported");
        }

        stream.read_exact(&mut buf[..1]).await?;
        let len = buf[0] as usize;
        stream.read_exact(&mut buf[..len]).await?;
        let target = String::from_utf8_lossy(&buf[..len]).to_string();

        // Read port
        stream.read_exact(&mut buf[..2]).await?;
        let port = u16::from_be_bytes([buf[0], buf[1]]);

        let reply = [
            0x05, 0x00, 0x00, 0x01, // version, success, reserved, IPv4
            0, 0, 0, 0, // BIND addr (0.0.0.0)
            0, 0, // BIND port (0)
        ];
        stream.write_all(&reply).await?;

        Ok((stream, target, port))
    }

    /// Handle parsed `TCP CONNECT` for `host:port`.
    ///
    /// If `host` is not an .i2p address and an outproxy was enabled, the request is routed there.
    async fn handle_tcp_connect(&mut self, mut stream: TcpStream, host: String, port: u16) {
        tracing::trace!(
            target: LOG_TARGET,
            %host,
            %port,
            "connect to remote destination"
        );

        // route connection request to sam server
        if host.ends_with(".i2p") {
            let future = self.session.connect_detached_with_options(
                &host,
                StreamOptions {
                    dst_port: port,
                    ..Default::default()
                },
            );

            tokio::spawn(async move {
                match future.await {
                    Err(error) => {
                        tracing::debug!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to connect to destination",
                        );
                        Err(error)
                    }
                    Ok(mut i2p_stream) =>
                        tokio::io::copy_bidirectional(&mut i2p_stream, &mut stream)
                            .await
                            .map_err(From::from),
                }
            });

            return;
        }

        // route non-.i2p host connections to outproxy if enabled
        let outproxy = match &self.outproxy {
            Some(outproxy) => {
                tracing::trace!(
                    target: LOG_TARGET,
                    %host,
                    %port,
                    %outproxy,
                    "connecting to outproxy",
                );

                outproxy.clone()
            }
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    %host,
                    %port,
                    "tried to connect to non-.i2p host but outproxy not enabled",
                );
                return;
            }
        };

        tokio::spawn(async move {
            async fn connect(
                mut stream: TcpStream,
                host: &str,
                port: u16,
                outproxy: String,
            ) -> anyhow::Result<()> {
                let mut upstream = TcpStream::connect(outproxy).await?;

                // handshake
                //
                // version 5, 1 method, no auth
                upstream.write_all(&[0x05, 0x01, 0x00]).await?;
                let mut handshake_res = [0u8; 2];
                upstream.read_exact(&mut handshake_res).await?;

                if handshake_res[1] != SOCKSV5_SUCCESS {
                    anyhow::bail!("upstream proxy requires authentication");
                }

                // send connect request for `host`
                //
                // version 5, connect, reserved, domain
                let mut request = vec![0x05, 0x01, 0x00, 0x03];
                request.push(host.len() as u8);
                request.extend_from_slice(host.as_bytes());
                request.extend_from_slice(&port.to_be_bytes());
                upstream.write_all(&request).await?;

                let mut response = [0u8; 10];
                upstream.read_exact(&mut response).await?;

                if response[1] != SOCKSV5_SUCCESS {
                    anyhow::bail!("upstream failed to connect")
                }

                tokio::io::copy_bidirectional(&mut stream, &mut upstream)
                    .await
                    .map(|_| ())
                    .map_err(From::from)
            }

            if let Err(error) = connect(stream, &host, port, outproxy).await {
                tracing::warn!(
                    target: LOG_TARGET,
                    %host,
                    %port,
                    ?error,
                    "outproxy connection failed"
                )
            }
        });
    }

    /// Run event loop of [`SocksProxy`].
    pub async fn run(mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                result = self.listener.accept() => {
                    let (stream, _) = result?;
                    self.futures.spawn(Self::parse_tcp_connect(stream));
                }
                result = self.futures.join_next(), if !self.futures.is_empty() => match result {
                    None => {}
                    Some(Err(error)) => tracing::warn!(
                        target: LOG_TARGET,
                        %error,
                        "failed to read TCP CONNECT from client",
                    ),
                    Some(Ok(Err(error))) => tracing::warn!(
                        target: LOG_TARGET,
                        %error,
                        "failed to parse TCP CONNECT",
                    ),
                    Some(Ok(Ok((stream, host, port)))) => self.handle_tcp_connect(stream, host, port).await,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fast_socks5::{
        client::{Config, Socks5Stream},
        server::Socks5ServerProtocol,
        socks4::client::Socks4Stream,
    };
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, BufReader};

    /// Fake SAMv3 server.
    struct SamServer {
        /// TCP listener for the server.
        listener: TcpListener,
    }

    impl SamServer {
        /// Create new [`SamServer`].
        async fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

            Self { listener }
        }

        /// Run the event loop of [`SamServer`].
        async fn run(self) {
            while let Ok((stream, _)) = self.listener.accept().await {
                tokio::spawn(async move {
                    let mut lines = BufReader::new(stream).lines();

                    while let Ok(Some(command)) = lines.next_line().await {
                        if command.starts_with("HELLO VERSION") {
                            lines
                                .get_mut()
                                .write_all("HELLO REPLY RESULT=OK VERSION=3.2\n".as_bytes())
                                .await
                                .unwrap();
                            continue;
                        }

                        if command.starts_with("SESSION CREATE") {
                            lines
                                .get_mut()
                                .write_all(
                                    "SESSION STATUS RESULT=OK DESTINATION=Fam-qmfYnngAnwkq3qwhkkoUeWNP\
                                        ckuYbZhK4xWwTzHa3BN9DY4dozKDPywI22LWfT1ALnVDonnRhCux0Iv3wc74-s2CTJOGLp\
                                        YvPGviS99dFSqRwgxi1dESbt5Liw4FIDZQMcDjcNziHspnTFfE4B3sZUtoNM0GYkrgksS3\
                                        BgVo3SvNn57~FkHDJvNxcaEL0uq9OGPfxNXNtyIeBxaUSJjYNbgcHG9Q2kzb~Z39FzylbE\
                                        iS979HJnc~w9Wo4DO8VCHGM1j6-CeRlf3hZpMaqQQJU0Q~k035~voydSIzDLJzMPvVmKAV\
                                        4q-0A5ikidKKv1N3kREQF5xDuDT1z3BMVHMIsyUECi8HOm3Ixa7XdcqpvHRl~W4RksOEdM\
                                        ChLrUZbqVr-8uW0lMRhRszAuU2PnF16bw9XEZoVAsNNHgvFQvnOwfLnPpSxtZaGNHGO8w\
                                        QaYmT3cImMUUhBbc9dcTYAHy8geZ1KzW4j7lpH4SsNaJPszCevkIVdvlqEAXZqh1YBQAE\
                                        AAcAADwJfIcEBwdeM2rjFM~cPo4btsSszyKlGZeUPzoTfHZv~4eR5efcr5YlogkmARNw57\
                                        h4sjmYvTESdTE7353u2uI=\n".as_bytes(),
                                )
                                .await
                                .unwrap();
                            continue;
                        }

                        println!("unhandled command: {command}");
                    }
                });
            }
        }
    }

    #[tokio::test]
    async fn invalid_request() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: None,
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        // send invalid http request to the proxy
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all("hello, world!\n".as_bytes()).await.unwrap();

        let mut buffer = vec![0u8; 512];
        assert!(stream.read(&mut buffer).await.is_err());
    }

    #[tokio::test]
    async fn socksv4_client() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: None,
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        assert!(
            Socks4Stream::connect(address, "http://host.i2p".to_string(), 80, false)
                .await
                .is_err()
        )
    }

    #[tokio::test]
    async fn socksv5_auth_ignored() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: None,
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        assert!(Socks5Stream::connect_with_password(
            address,
            "http://host.i2p".to_string(),
            80,
            "username".to_string(),
            "password".to_string(),
            Config::default()
        )
        .await
        .is_ok())
    }

    #[tokio::test]
    async fn socksv5_ipv4_ignored() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: None,
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        assert!(
            Socks5Stream::connect(address, "127.0.0.1".to_string(), 80, Config::default())
                .await
                .is_err()
        )
    }

    #[tokio::test]
    async fn socksv5_ipv6_ignored() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: None,
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        assert!(
            Socks5Stream::connect(address, "::".to_string(), 80, Config::default())
                .await
                .is_err()
        )
    }

    #[tokio::test]
    async fn outproxy_works() {
        let sam_port = {
            let sam = SamServer::new().await;
            let port = sam.listener.local_addr().unwrap().port();
            tokio::spawn(sam.run());

            port
        };

        // create mock socksv5 server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (_stream, command, address) = Socks5ServerProtocol::accept_no_auth(stream)
                .await
                .unwrap()
                .read_command()
                .await
                .unwrap();

            match command {
                fast_socks5::Socks5Command::TCPConnect => address.to_string(),
                _ => panic!("invalid command"),
            }
        });

        let proxy = SocksProxy::new(
            SocksProxyConfig {
                port: 0,
                host: "127.0.0.1".to_string(),
                i2cp: None,
                outproxy: Some(format!("{}:{}", address.ip().to_string(), address.port())),
            },
            sam_port,
        )
        .await
        .unwrap();
        let address = proxy.listener.local_addr().unwrap();
        tokio::spawn(proxy.run());

        let _stream = Socks5Stream::connect(
            address,
            "http://host.onion".to_string(),
            1337,
            Config::default(),
        )
        .await
        .unwrap();

        let destination = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("no timeout")
            .expect("to succeed");
        assert_eq!(destination, "http://host.onion:1337".to_string());
    }
}
