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
    ui::dioxus::{
        style::{global_css, DESKTOP_HEAD},
        types::{RouterState, RouterStatus, SidebarSelection, Traffic},
    },
};

use dioxus::prelude::*;
use emissary_core::{
    crypto::base64_encode,
    events::{Event, EventSubscriber},
    primitives::RouterId,
};
use tokio::sync::mpsc::Sender;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

mod bandwidth;
mod bandwidth_monitor;
mod dashboard;
mod sidebar;
mod style;
mod svg;
mod types;

/// Application state.
#[allow(unused)]
struct AppState {
    /// Address book handle, if enabled.
    address_book_handle: Option<Arc<AddressBookHandle>>,

    /// Router base path.
    base_path: PathBuf,

    /// Router configuration.
    config: EmissaryConfig,

    /// Event subscriber for the router.
    events: EventSubscriber,

    /// TX channel for sending shutdown signal.
    shutdown_tx: Sender<()>,

    /// Has the sidebar been collapsed.
    sidebar_collapsed: bool,

    /// Router state.
    state: RouterState,

    /// Router status.
    status: RouterStatus,

    /// Traffic info.
    traffic: Traffic,

    /// Currently active view.
    view: SidebarSelection,
}

impl AppState {
    /// Create new `AppState`.
    fn new(options: AppOptions) -> Self {
        let AppOptions {
            events,
            config,
            base_path,
            address_book_handle,
            router_id,
            shutdown_tx,
        } = options;

        Self {
            address_book_handle,
            base_path,
            config,
            events,
            shutdown_tx,
            sidebar_collapsed: false,
            state: RouterState::new(base64_encode(router_id.to_vec()).leak()),
            status: RouterStatus::Active,
            traffic: Traffic::new(),
            view: SidebarSelection::Dashboard,
        }
    }

    /// Is the router active.
    fn is_active(&self) -> bool {
        std::matches!(self.status, RouterStatus::Active)
    }

    /// Get `RouterState`.
    fn router_state(&self) -> RouterState {
        self.state
    }

    /// Get network status.
    ///
    /// Returns the network status string and a color representing that status.
    fn network_status(&self) -> (&'static str, &'static str) {
        match &self.status {
            RouterStatus::ShuttingDown => ("Shutting Down", "#e34234"),
            RouterStatus::Active =>
                if self.state.num_routers < 10 {
                    ("Connecting", "#f59e0b")
                } else {
                    let total = self.state.num_tunnels_built + self.state.num_tunnel_build_failures;
                    let rate = if total > 0 {
                        self.state.num_tunnels_built as f64 / total as f64
                    } else {
                        1.0
                    };

                    if rate < 0.30 {
                        ("Degraded", "#f97316")
                    } else {
                        ("Active", "#22c55e")
                    }
                },
        }
    }

    /// Advance the state of the router UI.
    ///
    /// Poll the event channel and update router state.
    fn tick(&mut self) {
        while let Some(event) = self.events.router_status() {
            match event {
                Event::RouterStatus {
                    transit,
                    transport,
                    tunnel,
                    ..
                } => {
                    self.state.num_transit_tunnels = transit.num_tunnels;
                    self.state.num_routers = transport.num_connected_routers;
                    self.state.num_tunnels_built = tunnel.num_tunnels_built;
                    self.state.num_tunnel_build_failures = tunnel.num_tunnel_build_failures;

                    self.traffic.prev_inbound_bandwidth = self.traffic.inbound_bandwidth;
                    self.traffic.prev_outbound_bandwidth = self.traffic.outbound_bandwidth;

                    let inbound_diff =
                        transport.inbound_bandwidth.saturating_sub(self.traffic.inbound_bandwidth);
                    let outbound_diff = transport
                        .outbound_bandwidth
                        .saturating_sub(self.traffic.outbound_bandwidth);
                    let total_diff = inbound_diff + outbound_diff;
                    if total_diff > self.traffic.peak_traffic {
                        self.traffic.peak_traffic = total_diff;
                    }
                    self.traffic.inbound_bandwidth = transport.inbound_bandwidth;
                    self.traffic.outbound_bandwidth = transport.outbound_bandwidth;
                    self.traffic.total_bandwidth.update(inbound_diff as f64, outbound_diff as f64);

                    let transit_in_diff = transit
                        .inbound_bandwidth
                        .saturating_sub(self.traffic.transit_inbound_bandwidth);
                    let transit_out_diff = transit
                        .outbound_bandwidth
                        .saturating_sub(self.traffic.transit_outbound_bandwidth);
                    self.traffic.transit_inbound_bandwidth = transit.inbound_bandwidth;
                    self.traffic.transit_outbound_bandwidth = transit.outbound_bandwidth;
                    self.traffic
                        .transit_bandwidth
                        .update(transit_in_diff as f64, transit_out_diff as f64);
                }
                Event::ShuttingDown =>
                    if matches!(self.status, RouterStatus::Active) {
                        self.status = RouterStatus::ShuttingDown;
                    },
                Event::ShutDown => {}
            }
        }
    }
}

/// `App` properties.
struct AppOptions {
    /// Event subscriber for the router.
    events: EventSubscriber,

    /// Router configuration.
    config: EmissaryConfig,

    /// Router base path.
    base_path: PathBuf,

    /// Address book handle, if enabled.
    address_book_handle: Option<Arc<AddressBookHandle>>,

    /// Local router ID.
    router_id: RouterId,

    /// TX channel for sending shutdown signal.
    shutdown_tx: Sender<()>,
}

/// Start the Dioxus router UI.
pub fn start(
    events: EventSubscriber,
    config: EmissaryConfig,
    base_path: PathBuf,
    address_book_handle: Option<Arc<AddressBookHandle>>,
    router_id: RouterId,
    shutdown_tx: Sender<()>,
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
            events,
            config,
            base_path,
            address_book_handle,
            router_id,
            shutdown_tx,
        }))))
        .launch(App)
}

#[component]
fn App() -> Element {
    let options = use_context::<Arc<Mutex<Option<AppOptions>>>>();
    let mut state = use_context_provider(move || {
        SyncSignal::new_maybe_sync(AppState::new(
            options.lock().expect("unpoisoned lock").take().expect("value to exist"),
        ))
    });
    let view = state.read().view;

    use_hook(|| {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                state.write().tick();
            }
        });
    });

    rsx! {
        style { { global_css() } }
        div {
            class: "app",
            sidebar::Sidebar { }
            div { class: "main-content",
                match view {
                    SidebarSelection::Dashboard => rsx! { dashboard::Dashboard {} },
                    SidebarSelection::Bandwidth => rsx! { bandwidth::BandwidthView {} },
                    SidebarSelection::HiddenServices => rsx! {},
                    SidebarSelection::Settings => rsx! {},
                    SidebarSelection::AddressBook => rsx! {},
                }
            }
        }
    }
}
