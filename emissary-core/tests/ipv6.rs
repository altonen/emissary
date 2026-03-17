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

use emissary_core::{
    events::EventSubscriber, router::Router, runtime::Runtime, Config, Ntcp2Config, SamConfig,
    TransitConfig,
};
use emissary_util::runtime::tokio::Runtime as TokioRuntime;
use rand::Rng;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use yosemite::{style::Stream, Session, SessionOptions};

use std::time::Duration;

#[derive(Clone, Copy)]
enum TransportKind {
    Ntcp2,
}

async fn make_router(
    floodfill: bool,
    net_id: u8,
    routers: Vec<Vec<u8>>,
    kind: TransportKind,
    mixed: bool,
) -> (Router<TokioRuntime>, EventSubscriber, Vec<u8>) {
    let (ntcp2, ssu2) = match kind {
        TransportKind::Ntcp2 => (
            Some(Ntcp2Config {
                port: 0u16,
                iv: {
                    let mut iv = [0u8; 16];
                    TokioRuntime::rng().fill_bytes(&mut iv);
                    iv
                },
                key: {
                    let mut key = [0u8; 32];
                    TokioRuntime::rng().fill_bytes(&mut key);
                    key
                },
                ipv4_host: mixed.then_some("127.0.0.1".parse().unwrap()),
                ipv6_host: Some("::1".parse().unwrap()),
                publish: true,
                ipv4: mixed,
                ipv6: true,
            }),
            None,
        ),
    };

    let config = Config {
        net_id: Some(net_id),
        floodfill,
        insecure_tunnels: true,
        allow_local: true,
        metrics: None,
        ntcp2,
        ssu2,
        routers,
        samv3_config: Some(SamConfig {
            tcp_port: 0u16,
            udp_port: 0u16,
            host: "127.0.0.1".to_string(),
        }),
        transit: Some(TransitConfig {
            max_tunnels: Some(5000),
        }),
        ..Default::default()
    };

    Router::<TokioRuntime>::new(config, None, None).await.unwrap()
}

#[tokio::test]
async fn ipv6_only_ntcp2() {
    ipv6_test(TransportKind::Ntcp2, false).await;
}

#[tokio::test]
async fn ipv4_ipv6_mixed_ntcp2() {
    ipv6_test(TransportKind::Ntcp2, true).await;
}

async fn ipv6_test(kind: TransportKind, mixed: bool) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let mut router_infos = Vec::<Vec<u8>>::new();
    let net_id = (TokioRuntime::rng().next_u32() % 255) as u8;

    for i in 0..4 {
        let (router, _events, router_info) =
            make_router(i < 2, net_id, router_infos.clone(), kind, mixed).await;

        router_infos.push(router_info);
        tokio::spawn(router);
    }

    // create two more routers, fetch their sam tcp ports and spawn them in the background
    let mut ports = Vec::<u16>::new();

    for _ in 0..2 {
        // client routers only support ipv6
        let router = make_router(false, net_id, router_infos.clone(), kind, false).await.0;

        ports.push(router.protocol_address_info().sam_tcp.unwrap().port());
        tokio::spawn(router);
    }

    // let the network boot up
    tokio::time::sleep(Duration::from_secs(20)).await;

    let mut session1 = tokio::time::timeout(
        Duration::from_secs(30),
        Session::<Stream>::new(SessionOptions {
            samv3_tcp_port: ports[0],
            ..Default::default()
        }),
    )
    .await
    .expect("no timeout")
    .expect("to succeed");
    let dest = session1.destination().to_owned();

    let handle = tokio::spawn(async move {
        let mut stream = tokio::time::timeout(Duration::from_secs(15), session1.accept())
            .await
            .expect("no timeout")
            .expect("to succeed");

        stream.write_all(b"hello, world!\n").await.unwrap();

        let mut buffer = vec![0u8; 64];
        let nread = stream.read(&mut buffer).await.unwrap();
        assert_eq!(
            std::str::from_utf8(&buffer[..nread]),
            Ok("goodbye, world!\n")
        );

        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let mut session2 = tokio::time::timeout(
        Duration::from_secs(30),
        Session::<Stream>::new(SessionOptions {
            samv3_tcp_port: ports[1],
            ..Default::default()
        }),
    )
    .await
    .expect("no timeout")
    .expect("to succeed");

    let mut stream = tokio::time::timeout(Duration::from_secs(10), session2.connect(&dest))
        .await
        .expect("no timeout")
        .expect("to succeed");

    let mut buffer = vec![0u8; 64];
    let nread = stream.read(&mut buffer).await.unwrap();

    assert_eq!(std::str::from_utf8(&buffer[..nread]), Ok("hello, world!\n"));

    stream.write_all(b"goodbye, world!\n").await.unwrap();

    assert!(handle.await.is_ok());
}
