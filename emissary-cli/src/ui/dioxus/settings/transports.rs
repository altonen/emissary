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

use std::net::{Ipv4Addr, Ipv6Addr};

#[component]
pub fn TransportsTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (
        ntcp2_port,
        ntcp2_ipv4_host,
        ntcp2_ipv6_host,
        ntcp2_ipv4_published,
        ntcp2_ipv6_published,
        ntcp2_ipv4_enabled,
        ntcp2_ipv6_enabled,
        ntcp_disable_pq,
        ntcp2_ml_kem,
        ntcp2_enabled,
    ) = {
        let state = state.read();
        (
            state.settings.ntcp2.port.clone().unwrap_or_default(),
            state.settings.ntcp2.ipv4_host.clone().unwrap_or_default(),
            state.settings.ntcp2.ipv6_host.clone().unwrap_or_default(),
            state.settings.ntcp2.publish_ipv4.unwrap_or(false),
            state.settings.ntcp2.publish_ipv6.unwrap_or(false),
            state.settings.ntcp2.ipv4.unwrap_or(true),
            state.settings.ntcp2.ipv6.unwrap_or(true),
            state.settings.ntcp2.disable_pq.unwrap_or(false),
            state.settings.ntcp2.ml_kem.clone().unwrap_or_default(),
            state.settings.ntcp2.enabled,
        )
    };

    let (
        ssu2_port,
        ssu2_ipv4_host,
        ssu2_ipv6_host,
        ssu2_ipv4_mtu,
        ssu2_ipv6_mtu,
        ssu2_ipv4_published,
        ssu2_ipv6_published,
        ssu2_ipv4_enabled,
        ssu2_ipv6_enabled,
        ssu2_disable_pq,
        ssu2_ml_kem,
        ssu2_enabled,
    ) = {
        let state = state.read();
        (
            state.settings.ssu2.port.clone().unwrap_or_default(),
            state.settings.ssu2.ipv4_host.clone().unwrap_or_default(),
            state.settings.ssu2.ipv6_host.clone().unwrap_or_default(),
            state.settings.ssu2.ipv4_mtu.clone().unwrap_or_default(),
            state.settings.ssu2.ipv6_mtu.clone().unwrap_or_default(),
            state.settings.ssu2.publish_ipv4.unwrap_or(false),
            state.settings.ssu2.publish_ipv6.unwrap_or(false),
            state.settings.ssu2.ipv4.unwrap_or(false),
            state.settings.ssu2.ipv6.unwrap_or(false),
            state.settings.ssu2.disable_pq.unwrap_or(false),
            state.settings.ssu2.ml_kem.clone().unwrap_or_default(),
            state.settings.ssu2.enabled,
        )
    };

    let (nat_pmp, upnp) = {
        let s = state.read();
        (
            s.settings.port_forwarding.nat_pmp,
            s.settings.port_forwarding.upnp,
        )
    };

    rsx! {
        div {
            // ntcp2
            div {
                class: if ntcp2_enabled {
                    "settings-section"
                } else {
                    "settings-section sf-disabled"
                },
                div {
                    class: "settings-section-title",
                    span { "NTCP2" }
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: ntcp2_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.ntcp2.enabled = e.checked();
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
                        class: if !ntcp2_port.is_empty() && ntcp2_port.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{ntcp2_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.ntcp2.port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "ML-KEM" }
                    input {
                        r#type: "text",
                        class: if !ntcp2_ml_kem.is_empty() && ntcp2_ml_kem.parse::<usize>().is_err() {
                            "input-error"
                        } else {
                            ""
                        },
                        value: "{ntcp2_ml_kem}",
                        placeholder: "4",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.ntcp2.ml_kem = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span {}
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: ntcp_disable_pq,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.ntcp2.disable_pq = Some(e.checked());
                                state.settings.dirty = true;
                            }
                        }
                        span { "Disable PQ" }
                    }
                    span {}
                    div {
                        class: "sf-ip-cols",

                        // ipv4
                        div {
                            class: "sf-ip-col",
                            span { class: "sf-ip-col-title", "IPv4" }
                            div {
                                class: "sf-grid",
                                span { class: "sf-label", "Host" }
                                input {
                                    r#type: "text",
                                    class: if !ntcp2_ipv4_host.is_empty() && ntcp2_ipv4_host.parse::<Ipv4Addr>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ntcp2_ipv4_host}",
                                    placeholder: "Host",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.ipv4_host = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ntcp2_ipv4_enabled,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.ipv4 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Enable" }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ntcp2_ipv4_published,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.publish_ipv4 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Publish" }
                            }
                        }

                        // ipv6
                        div {
                            class: "sf-ip-col",
                            span { class: "sf-ip-col-title", "IPv6" }
                            div {
                                class: "sf-grid",
                                span { class: "sf-label", "Host" }
                                input {
                                    r#type: "text",
                                    class: if !ntcp2_ipv6_host.is_empty() && ntcp2_ipv6_host.parse::<Ipv6Addr>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ntcp2_ipv6_host}",
                                    placeholder: "Host",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.ipv6_host = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ntcp2_ipv6_enabled,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.ipv6 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Enable" }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ntcp2_ipv6_published,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ntcp2.publish_ipv6 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Publish" }
                            }
                        }
                    }
                }
            }

            // ssu2
            div {
                class: if ssu2_enabled {
                    "settings-section"
                } else {
                    "settings-section sf-disabled"
                },
                div {
                    class: "settings-section-title",
                    span { "SSU2" }
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: ssu2_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.ssu2.enabled = e.checked();
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
                        class: if !ssu2_port.is_empty() && ssu2_port.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{ssu2_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.ssu2.port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span { class: "sf-label", "ML-KEM" }
                    input {
                        r#type: "text",
                        class: "",
                        value: "{ssu2_ml_kem}",
                        placeholder: "3,4",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.ssu2.ml_kem = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span {}
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: ssu2_disable_pq,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.ssu2.disable_pq = Some(e.checked());
                                state.settings.dirty = true;
                            }
                        }
                        span { "Disable PQ" }
                    }
                    span {}
                    div {
                        class: "sf-ip-cols",

                        // ipv4
                        div {
                            class: "sf-ip-col",
                            span { class: "sf-ip-col-title", "IPv4" }
                            div {
                                class: "sf-grid",
                                span { class: "sf-label", "Host" }
                                input {
                                    r#type: "text",
                                    class: if !ssu2_ipv4_host.is_empty() && ssu2_ipv4_host.parse::<Ipv4Addr>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ssu2_ipv4_host}",
                                    placeholder: "Host",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv4_host = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { class: "sf-label", "MTU" }
                                input {
                                    r#type: "text",
                                    class: if !ssu2_ipv4_mtu.is_empty() && ssu2_ipv4_mtu.parse::<usize>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ssu2_ipv4_mtu}",
                                    placeholder: "MTU",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv4_mtu = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ssu2_ipv4_enabled,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv4 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Enable" }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ssu2_ipv4_published,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.publish_ipv4 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Publish" }
                            }
                        }

                        // ipv6
                        div {
                            class: "sf-ip-col",
                            span { class: "sf-ip-col-title", "IPv6" }
                            div {
                                class: "sf-grid",
                                span { class: "sf-label", "Host" }
                                input {
                                    r#type: "text",
                                    class: if !ssu2_ipv6_host.is_empty() && ssu2_ipv6_host.parse::<Ipv6Addr>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ssu2_ipv6_host}",
                                    placeholder: "Host",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv6_host = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { class: "sf-label", "MTU" }
                                input {
                                    r#type: "text",
                                    class: if !ssu2_ipv6_mtu.is_empty() && ssu2_ipv6_mtu.parse::<usize>().is_err() {
                                        "input-error"
                                    } else {
                                        ""
                                    },
                                    value: "{ssu2_ipv6_mtu}",
                                    placeholder: "MTU",
                                    oninput: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv6_mtu = Some(e.value());
                                        state.settings.dirty = true;
                                    }
                                }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ssu2_ipv6_enabled,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.ipv6 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Enable" }
                            }
                            label {
                                class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: ssu2_ipv6_published,
                                    onchange: move |e: Event<FormData>| {
                                        let mut state = state.write();
                                        state.settings.ssu2.publish_ipv6 = Some(e.checked());
                                        state.settings.dirty = true;
                                    }
                                }
                                span { "Publish" }
                            }
                        }
                    }
                }
            }

            // Port forwarding
            div {
                class: "settings-section",
                div {
                    class: "settings-section-title", span { "Port forwarding" } }
                div {
                    class: "sf-grid",
                    span {}
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: nat_pmp,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.port_forwarding.nat_pmp = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "NAT-PMP" }
                    }
                    span {}
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: upnp,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.port_forwarding.upnp = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "UPnP" }
                    }
                }
            }
        }
    }
}
