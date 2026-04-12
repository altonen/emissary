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

use crate::ui::dioxus::{
    style::{global_css, DESKTOP_HEAD},
    types::{RouterState, RouterStatus, SidebarSelection},
};

use dioxus::prelude::*;

mod dashboard;
mod sidebar;
mod style;
mod svg;
mod types;

/// Application state.
struct AppState {
    /// Has the sidebar been collapsed.
    sidebar_collapsed: bool,

    /// Currently active view.
    view: SidebarSelection,

    /// Router status.
    status: RouterStatus,

    /// Router state.
    state: RouterState,
}

impl AppState {
    /// Create new `AppState`.
    fn new() -> Self {
        Self {
            sidebar_collapsed: false,
            view: SidebarSelection::Dashboard,
            status: RouterStatus::Active,
            state: RouterState::new("router-id"),
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
}

pub fn start() {
    let cfg = dioxus::desktop::Config::default()
        .with_menu(None)
        .with_custom_head(DESKTOP_HEAD.to_string())
        .with_window(
            dioxus::desktop::WindowBuilder::default()
                .with_title("emissary")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1200.0, 800.0)),
        );
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
}

#[component]
fn App() -> Element {
    let state = use_context_provider(move || Signal::new(AppState::new()));
    let view = state.read().view;

    rsx! {
        style { { global_css() } }
        div {
            class: "app",
            sidebar::Sidebar { }
            div { class: "main-content",
                match view {
                    SidebarSelection::Dashboard => rsx! { dashboard::Dashboard {} },
                    SidebarSelection::Bandwidth => rsx! {},
                    SidebarSelection::HiddenServices => rsx! {},
                    SidebarSelection::Settings => rsx! {},
                    SidebarSelection::AddressBook => rsx! {},
                }
            }
        }
    }
}
