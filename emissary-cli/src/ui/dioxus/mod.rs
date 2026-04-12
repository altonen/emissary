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
    types::{RouterStatus, SidebarSelection},
};

use dioxus::prelude::*;

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
}

impl AppState {
    /// Create new `AppState`.
    fn new() -> Self {
        Self {
            sidebar_collapsed: false,
            view: SidebarSelection::Dashboard,
            status: RouterStatus::Active,
        }
    }

    /// Is the router active.
    fn is_active(&self) -> bool {
        std::matches!(self.status, RouterStatus::Active)
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
    let _state = use_context_provider(move || Signal::new(AppState::new()));

    rsx! {
        style { { global_css() } }
        div {
            class: "app",
            sidebar::Sidebar { }
        }
    }
}
