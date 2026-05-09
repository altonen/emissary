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
    svg::*,
    types::{RouterStatus, SidebarSelection},
    AppState,
};

use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let collapsed = state.read().sidebar_collapsed;
    let is_active = state.read().is_active();
    let view = state.read().view;

    let items = vec![
        (SidebarSelection::Dashboard, DASHBOARD_SVG, "Dashboard"),
        (SidebarSelection::Bandwidth, BANDWIDTH_SVG, "Bandwidth"),
        (
            SidebarSelection::HiddenServices,
            SERVER_SVG,
            "Hidden services",
        ),
        (
            SidebarSelection::AddressBook,
            ADDRESS_BOOK_SVG,
            "Address book",
        ),
        (SidebarSelection::Settings, SETTINGS_SVG, "Settings"),
    ];

    let sidebar_class = if collapsed {
        "sidebar collapsed"
    } else {
        "sidebar"
    };
    let toggle_icon = if collapsed { FORWARD_SVG } else { BACKWARD_SVG };

    rsx! {
        div {
            class: sidebar_class,

            // title
            div {
                class: "sidebar-title",
                if collapsed { "e" } else { "emissary" }
            }

            // navigation
            for (msg, icon, label) in items {
                {
                    let is_selected = view == msg;
                    let class = if is_selected { "sidebar-item active" } else { "sidebar-item" };
                    rsx! {
                        button {
                            class: class,
                            title: label,
                            onclick: move |_| {
                                state.write().view = msg;
                            },
                            span {
                                class: "sidebar-icon",
                                dangerous_inner_html: icon,
                            }
                            span {
                                class: "sidebar-label",
                                "{label}"
                            }
                        }
                    }
                }
            }

            div { class: "sidebar-spacer" }

            // sidebar toggle + power button
            div {
                class: "sidebar-bottom",
                style: if collapsed { "flex-direction:column;" } else { "flex-direction:row;" },
                button {
                    class: "sidebar-toggle",
                    title: if collapsed { "Expand sidebar" } else { "Collapse sidebar" },
                    onclick: move |_| {
                        let val = state.read().sidebar_collapsed;
                        state.write().sidebar_collapsed = !val;
                    },
                    span {
                        style: "display:inline-flex;width:16px;height:16px;",
                        dangerous_inner_html: toggle_icon,
                    }
                }

                button {
                    class: if is_active { "power-btn" } else { "power-btn shutting-down" },
                    title: if is_active { "Shut down router" } else { "Force quit" },
                    onclick: move |_| {
                        let mut w = state.write();

                        match w.status {
                            RouterStatus::Active => {
                                let _ = w.shutdown_tx.try_send(());
                                w.status = RouterStatus::ShuttingDown;
                            }
                            RouterStatus::ShuttingDown => {
                                std::process::exit(0);
                            }
                        }
                    },
                    span {
                        style: "display:inline-flex;width:20px;height:20px;",
                        dangerous_inner_html: POWER_OFF_SVG,
                    }
                }
            }
        }
    }
}
