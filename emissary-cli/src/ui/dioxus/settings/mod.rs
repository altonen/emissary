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
    types::{SettingsStatus, SettingsTab},
    AppState,
};

use dioxus::prelude::*;

mod advanced;
mod client;
mod proxies;
mod transports;
mod tunnels;

#[component]
pub fn SettingsView() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (active_tab, settings_status, settings_dirty) = {
        let s = state.read();
        (
            s.settings.active_tab,
            s.settings.status.clone(),
            s.settings.dirty,
        )
    };

    rsx! {
        div {
            class: "page",
            div {
                class: "page-title",
                h1 { "Settings" }
                p { "Configure your I2P router" }
            }

            div {
                class: "settings-card",
                div {
                    class: "settings-container",

                    div {
                        class: "tabs",
                        button {
                            class: if active_tab == SettingsTab::Transports { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| {
                                let mut w = state.write();
                                if w.settings.active_tab != SettingsTab::Transports {
                                    w.settings.status = SettingsStatus::Idle;
                                }
                                w.settings.active_tab = SettingsTab::Transports;
                            },
                            span { style: "display:inline-flex;width:18px;height:18px;", dangerous_inner_html: SETTINGS_SVG }
                            "Transports"
                        }
                        button {
                            class: if active_tab == SettingsTab::Client { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| {
                                let mut w = state.write();
                                if w.settings.active_tab != SettingsTab::Client {
                                    w.settings.status = SettingsStatus::Idle;
                                }
                                w.settings.active_tab = SettingsTab::Client;
                            },
                            span { style: "display:inline-flex;width:18px;height:18px;", dangerous_inner_html: HANDSHAKE_SVG }
                            "Clients"
                        }
                        button {
                            class: if active_tab == SettingsTab::Proxies { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| {
                                let mut w = state.write();
                                if w.settings.active_tab != SettingsTab::Proxies {
                                    w.settings.status = SettingsStatus::Idle;
                                }
                                w.settings.active_tab = SettingsTab::Proxies;
                            },
                            span { style: "display:inline-flex;width:18px;height:18px;", dangerous_inner_html: ALT_ROUTE_SVG }
                            "Proxies"
                        }
                        button {
                            class: if active_tab == SettingsTab::Tunnels { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| {
                                let mut w = state.write();
                                if w.settings.active_tab != SettingsTab::Tunnels {
                                    w.settings.status = SettingsStatus::Idle;
                                }
                                w.settings.active_tab = SettingsTab::Tunnels;
                            },
                            span { style: "display:inline-flex;width:18px;height:18px;", dangerous_inner_html: TUNNELS_SVG }
                            "Tunnels"
                        }
                        button {
                            class: if active_tab == SettingsTab::Advanced { "tab-btn active" } else { "tab-btn" },
                            onclick: move |_| {
                                let mut w = state.write();
                                if w.settings.active_tab != SettingsTab::Advanced {
                                    w.settings.status = SettingsStatus::Idle;
                                }
                                w.settings.active_tab = SettingsTab::Advanced;
                            },
                            span { style: "display:inline-flex;width:18px;height:18px;", dangerous_inner_html: ADVANCED_SVG }
                            "Advanced"
                        }
                    }

                    // tabs
                    match active_tab {
                        SettingsTab::Transports => rsx! { transports::TransportsTab {} },
                        SettingsTab::Client => rsx! { client::ClientTab {} },
                        SettingsTab::Proxies => rsx! { proxies::ProxiesTab {} },
                        SettingsTab::Tunnels => rsx! { tunnels::TunnelsTab {} },
                        SettingsTab::Advanced => rsx! { advanced::AdvancedTab {} },
                    }
                }

                // settings save footer
                div {
                    class: "settings-footer",
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            let mut state = state.write();
                            match state.save_settings() {
                                Ok(()) => {
                                    state.settings.status = SettingsStatus::Saved;
                                    state.push_toast("Settings saved");
                                }
                                Err(e) => state.settings.status = SettingsStatus::Error(e),
                            }
                        },
                        "Save"
                        if settings_dirty {
                            span {
                                style: "margin-left:6px;color:#f59e0b;font-size:11px;",
                                title: "You have unsaved changes",
                                "●"
                            }
                        }
                    }

                    match &settings_status {
                        SettingsStatus::Idle => rsx! { },
                        SettingsStatus::Saved => rsx! {
                            span { class: "status-ok", style: "padding:0;", "Configuration saved" }
                        },
                        SettingsStatus::Error(err) => {
                            let err = err.clone();
                            rsx! {
                                span { class: "status-error", style: "padding:0;", "{err}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
