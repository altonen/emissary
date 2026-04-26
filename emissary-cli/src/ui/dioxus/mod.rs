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
        config::save_router_config,
        style::global_css,
        types::{RouterState, RouterStatus, Settings, SettingsTab, SidebarSelection, Traffic},
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
mod config;
mod dashboard;
mod settings;
mod sidebar;
mod style;
mod svg;
mod types;

#[cfg(feature = "dioxus")]
mod native;

#[cfg(feature = "dioxus")]
pub use native::start;

#[cfg(feature = "web-ui")]
mod web;

#[cfg(feature = "web-ui")]
pub use web::start;

/// `App` options.
#[derive(Clone)]
struct AppOptions {
    /// Event subscriber for the router.
    events: Arc<Mutex<EventSubscriber>>,

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

    /// Shared traffic state, persisted across reconnections.
    traffic: Arc<Mutex<Traffic>>,
}

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
    events: Arc<Mutex<EventSubscriber>>,

    /// Settings info.
    settings: Settings,

    /// TX channel for sending shutdown signal.
    shutdown_tx: Sender<()>,

    /// Has the sidebar been collapsed.
    sidebar_collapsed: bool,

    /// Router state.
    state: RouterState,

    /// Router status.
    status: RouterStatus,

    /// Traffic info, shared across reconnections.
    traffic: Arc<Mutex<Traffic>>,

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
            traffic,
        } = options;

        Self {
            address_book_handle,
            base_path,
            events,
            settings: Settings::new(&config),
            config,
            shutdown_tx,
            sidebar_collapsed: false,
            state: RouterState::new(base64_encode(router_id.to_vec()).leak()),
            status: RouterStatus::Active,
            traffic,
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
        let mut traffic = self.traffic.lock().expect("to succeed");
        while let Some(event) = self.events.lock().expect("to succeed").router_status() {
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

                    traffic.prev_inbound_bandwidth = traffic.inbound_bandwidth;
                    traffic.prev_outbound_bandwidth = traffic.outbound_bandwidth;

                    let inbound_diff =
                        transport.inbound_bandwidth.saturating_sub(traffic.inbound_bandwidth);
                    let outbound_diff =
                        transport.outbound_bandwidth.saturating_sub(traffic.outbound_bandwidth);
                    let total_diff = inbound_diff + outbound_diff;
                    if total_diff > traffic.peak_traffic {
                        traffic.peak_traffic = total_diff;
                    }
                    traffic.inbound_bandwidth = transport.inbound_bandwidth;
                    traffic.outbound_bandwidth = transport.outbound_bandwidth;
                    traffic.total_bandwidth.update(inbound_diff as f64, outbound_diff as f64);

                    let transit_in_diff =
                        transit.inbound_bandwidth.saturating_sub(traffic.transit_inbound_bandwidth);
                    let transit_out_diff = transit
                        .outbound_bandwidth
                        .saturating_sub(traffic.transit_outbound_bandwidth);
                    traffic.transit_inbound_bandwidth = transit.inbound_bandwidth;
                    traffic.transit_outbound_bandwidth = transit.outbound_bandwidth;
                    traffic
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

    pub fn save_settings(&mut self) -> Result<(), String> {
        match self.settings.active_tab {
            SettingsTab::Transports => {
                if !self.settings.ntcp2.enabled && !self.settings.ssu2.enabled {
                    return Err(String::from(
                        "At least one transport (NTCP2 or SSU2) must be enabled",
                    ));
                }

                self.config.ntcp2 = TryInto::<Option<crate::config::Ntcp2Config>>::try_into(
                    self.settings.ntcp2.clone(),
                )?;
                self.config.ssu2 = TryInto::<Option<crate::config::Ssu2Config>>::try_into(
                    self.settings.ssu2.clone(),
                )?;
                self.config.port_forwarding =
                    TryInto::<Option<crate::config::PortForwardingConfig>>::try_into(
                        self.settings.port_forwarding.clone(),
                    )?;
            }
            SettingsTab::Client => {
                self.config.i2cp = TryInto::<Option<crate::config::I2cpConfig>>::try_into(
                    self.settings.i2cp.clone(),
                )?;
                self.config.sam = TryInto::<Option<crate::config::SamConfig>>::try_into(
                    self.settings.sam.clone(),
                )?;
            }
            SettingsTab::Proxies => {
                self.config.http_proxy =
                    TryInto::<Option<crate::config::HttpProxyConfig>>::try_into(
                        self.settings.http_proxy.clone(),
                    )?;
                self.config.socks_proxy =
                    TryInto::<Option<crate::config::SocksProxyConfig>>::try_into(
                        self.settings.socks_proxy.clone(),
                    )?;
            }
            SettingsTab::Tunnels => {}
            SettingsTab::Advanced => {}
        }

        save_router_config(self.base_path.join("router.toml"), &self.config);
        self.settings.dirty = false;

        Ok(())
    }
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

    #[cfg(feature = "web-ui")]
    use_future(move || async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            state.write().tick();
        }
    });
    #[cfg(feature = "dioxus")]
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
                    SidebarSelection::AddressBook => rsx! {},
                    SidebarSelection::HiddenServices => rsx! {},
                    SidebarSelection::Settings => rsx! { settings::SettingsView {} },
                }
            }
        }
    }
}
