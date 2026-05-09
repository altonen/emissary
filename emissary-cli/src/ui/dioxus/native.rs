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
    ui::dioxus::{style::DESKTOP_HEAD, types::Traffic, App, AppOptions},
};

use emissary_core::{events::EventSubscriber, primitives::RouterId};
use tokio::sync::mpsc::Sender;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Start the native router UI.
pub async fn start(
    events: EventSubscriber,
    config: EmissaryConfig,
    base_path: PathBuf,
    address_book_handle: Option<Arc<AddressBookHandle>>,
    router_id: RouterId,
    shutdown_tx: Sender<()>,
    web_ui: bool,
) {
    let cfg = dioxus::desktop::Config::default()
        .with_menu(None)
        .with_custom_head(DESKTOP_HEAD.to_string())
        .with_window(
            dioxus::desktop::WindowBuilder::default()
                .with_title("emissary")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1200.0, 800.0)),
        );

    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .with_context(Arc::new(Mutex::new(Some(AppOptions {
            events: Arc::new(Mutex::new(events)),
            config,
            base_path,
            address_book_handle,
            router_id,
            shutdown_tx,
            traffic: Arc::new(Mutex::new(Traffic::new())),
            web_ui,
        }))))
        .launch(App)
}
