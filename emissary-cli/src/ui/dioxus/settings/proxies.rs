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

use crate::ui::dioxus::AppState;

use dioxus::prelude::*;

use std::net::Ipv4Addr;

#[component]
pub fn ProxiesTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (
        http_port,
        http_host,
        http_outproxy,
        http_encryption,
        http_in_len,
        http_in_cnt,
        http_out_len,
        http_out_cnt,
        http_enabled,
    ) = {
        let state = state.read();
        (
            state.settings.http_proxy.port.clone().unwrap_or_default(),
            state.settings.http_proxy.host.clone().unwrap_or_default(),
            state.settings.http_proxy.outproxy.clone().unwrap_or_default(),
            state.settings.http_proxy.i2cp.encryption.clone().unwrap_or_default(),
            state
                .settings
                .http_proxy
                .tunnel_config
                .inbound_len
                .as_deref()
                .unwrap_or("")
                .to_string(),
            state
                .settings
                .http_proxy
                .tunnel_config
                .inbound_count
                .as_deref()
                .unwrap_or("")
                .to_string(),
            state
                .settings
                .http_proxy
                .tunnel_config
                .outbound_len
                .as_deref()
                .unwrap_or("")
                .to_string(),
            state
                .settings
                .http_proxy
                .tunnel_config
                .outbound_count
                .as_deref()
                .unwrap_or("")
                .to_string(),
            state.settings.http_proxy.enabled,
        )
    };

    let (socks_port, socks_host, socks_outproxy, socks_encryption, socks_enabled) = {
        let state = state.read();
        (
            state.settings.socks_proxy.port.clone().unwrap_or_default(),
            state.settings.socks_proxy.host.clone().unwrap_or_default(),
            state.settings.socks_proxy.outproxy.clone().unwrap_or_default(),
            state.settings.socks_proxy.i2cp.encryption.clone().unwrap_or_default(),
            state.settings.socks_proxy.enabled,
        )
    };

    rsx! {
        div {
            // http proxy
            div {
                class: if http_enabled { "settings-section" } else { "settings-section sf-disabled" },
                div {
                    class: "settings-section-title",
                    span { "HTTP" }
                    label { class: "checkbox-row",
                        input {
                            r#type: "checkbox", checked: http_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.http_proxy.enabled = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "Enable" }
                    }
                }
                div {
                    class: "sf-grid",
                    span { class: "sf-label", "Port" }
                    input {
                        r#type: "text",
                        class: if !http_port.is_empty() && http_port.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{http_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.http_proxy.port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Host" }
                    input {
                        r#type: "text",
                        class: if !http_host.is_empty() && http_host.parse::<Ipv4Addr>().is_err() {
                            "sf-input-wide input-error"
                        } else {
                            "sf-input-wide"
                        },
                        placeholder: "Host",
                        value: "{http_host}",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.http_proxy.host = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Outproxy" }
                    input {
                        r#type: "text",
                        class: "sf-input-wide",
                        value: "{http_outproxy}",
                        placeholder: "Outproxy",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.http_proxy.outproxy = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Encryption" }
                    input {
                        r#type: "text",
                        class: "sf-input-wide",
                        value: "{http_encryption}",
                        placeholder: "6,4",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.http_proxy.i2cp.encryption = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Inbound" }
                    div {
                        class: "sf-pair",
                        span { class: "sf-pair-label", "length" }
                        input {
                            r#type: "text",
                            value: "{http_in_len}",
                            placeholder: "3",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.http_proxy.tunnel_config.inbound_len = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                        span { class: "sf-pair-label", "count" }
                        input {
                            r#type: "text",
                            value: "{http_in_cnt}",
                            placeholder: "2",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.http_proxy.tunnel_config.inbound_count = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                    }
                    span { class: "sf-label", "Outbound" }
                    div {
                        class: "sf-pair",
                        span { class: "sf-pair-label", "length" }
                        input {
                            r#type: "text",
                            value: "{http_out_len}",
                            placeholder: "3",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.http_proxy.tunnel_config.outbound_len = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                        span { class: "sf-pair-label", "count" }
                        input {
                            r#type: "text",
                            value: "{http_out_cnt}",
                            placeholder: "2",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.http_proxy.tunnel_config.outbound_count = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                    }
                }
            }

            // socks proxy
            div {
                class: if socks_enabled { "settings-section" } else { "settings-section sf-disabled" },
                div {
                    class: "settings-section-title",
                    span { "SOCKSv5" }
                    label { class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: socks_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.socks_proxy.enabled = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "Enable" }
                    }
                }
                div {
                    class: "sf-grid",
                    span { class: "sf-label", "Port" }
                    input {
                        r#type: "text",
                        class: if !socks_port.is_empty() && socks_port.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        class: "sf-input-short",
                        value: "{socks_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.socks_proxy.port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Host" }
                    input {
                        r#type: "text",
                        class: if !socks_host.is_empty() && socks_host.parse::<Ipv4Addr>().is_err() {
                            "sf-input-wide input-error"
                        } else {
                            "sf-input-wide"
                        },
                        value: "{socks_host}",
                        placeholder: "Host",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.socks_proxy.host = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Outproxy" }
                    input {
                        r#type: "text",
                        class: "sf-input-wide",
                        value: "{socks_outproxy}",
                        placeholder: "Outproxy",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.socks_proxy.outproxy = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "Encryption" }
                    input {
                        r#type: "text",
                        class: "sf-input-wide",
                        value: "{socks_encryption}",
                        placeholder: "6,4",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.socks_proxy.i2cp.encryption = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                }
            }
        }
    }
}
