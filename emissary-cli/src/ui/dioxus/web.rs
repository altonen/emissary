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
    address_book::AddressBookHandle,
    config::EmissaryConfig,
    ui::dioxus::{types::Traffic, App, AppOptions},
};

use axum::{
    extract::{State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use dioxus::prelude::*;
use dioxus_liveview::{axum_socket, interpreter_glue, LiveViewPool};
use emissary_core::{events::EventSubscriber, primitives::RouterId};
use tokio::{net::TcpListener, sync::mpsc::Sender};

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Application data.
struct AppData {
    pool: LiveViewPool,
    params: AppOptions,
}

/// Start the web UI.
pub async fn start(
    events: EventSubscriber,
    config: EmissaryConfig,
    base_path: PathBuf,
    address_book_handle: Option<Arc<AddressBookHandle>>,
    router_id: RouterId,
    shutdown_tx: Sender<()>,
    web_ui: bool,
) {
    let port = config.router_ui.as_ref().and_then(|config| config.port).unwrap_or(7657);
    let router = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .with_state(Arc::new(AppData {
            pool: LiveViewPool::new(),
            params: AppOptions {
                events: Arc::new(Mutex::new(events)),
                config,
                base_path,
                address_book_handle,
                router_id,
                shutdown_tx,
                traffic: Arc::new(Mutex::new(Traffic::new())),
                web_ui,
            },
        }));

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("to bind liveview port");

    axum::serve(listener, router.into_make_service())
        .await
        .expect("liveview server exited");
}

async fn index_handler() -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Emissary Router Console</title>
</head>
<body>
  <div id="main"></div>
  {glue}
</body>
</html>"#,
        glue = interpreter_glue("/ws"),
    ))
}

async fn ws_handler(ws: WebSocketUpgrade, State(data): State<Arc<AppData>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let params = Arc::new(Mutex::new(Some(data.params.clone())));
        let _ = data
            .pool
            .launch_virtualdom(axum_socket(socket), move || {
                let mut vdom = VirtualDom::new(App);
                vdom.insert_any_root_context(Box::new(params));
                vdom
            })
            .await;
    })
}
