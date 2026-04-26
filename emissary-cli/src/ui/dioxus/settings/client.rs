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

#[component]
pub fn ClientTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (i2cp_port, i2cp_host, i2cp_enabled, sam_tcp, sam_udp, sam_host, sam_enabled) = {
        let state = state.read();
        (
            state.settings.i2cp.port.clone().unwrap_or_default(),
            state.settings.i2cp.host.clone().unwrap_or_default(),
            state.settings.i2cp.enabled,
            state.settings.sam.tcp_port.clone().unwrap_or_default(),
            state.settings.sam.udp_port.clone().unwrap_or_default(),
            state.settings.sam.host.clone().unwrap_or_default(),
            state.settings.sam.enabled,
        )
    };

    rsx! {
        div {
            // i2cp
            div {
                class: if i2cp_enabled { "settings-section" } else { "settings-section sf-disabled" },
                div {
                    class: "settings-section-title",
                    span { "I2CP" }
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: i2cp_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.i2cp.enabled = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "Enable" }
                    }
                }
                div {
                    class: "sf-grid",
                    span {
                        class: "sf-label",
                        "Port"
                    },
                    input {
                        r#type: "text",
                        class: if !i2cp_port.is_empty() && i2cp_port.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{i2cp_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.i2cp.port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span {
                        class: "sf-label",
                        "Host"
                    },
                    input {
                        r#type: "text",
                        value: "{i2cp_host}", placeholder: "Host",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.i2cp.host = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                }
            }

            // samv3
            div {
                class: if sam_enabled { "settings-section" } else { "settings-section sf-disabled" },
                div {
                    class: "settings-section-title",
                    span { "SAMv3" }
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox", checked: sam_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.sam.enabled = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "Enable" }
                    }
                }
                div {
                    class: "sf-grid",
                    span {
                        class: "sf-label",
                        "TCP port"
                    },
                    input {
                        r#type: "text",
                        class: if !sam_tcp.is_empty() && sam_tcp.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{sam_tcp}", placeholder: "7656",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.sam.tcp_port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span {
                        class: "sf-label",
                        "UDP port"
                    },
                    input {
                        r#type: "text",
                        class: if !sam_udp.is_empty() && sam_udp.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{sam_udp}", placeholder: "7655",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.sam.udp_port = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                    span {
                        class: "sf-label",
                        "Host"
                    },
                    input {
                        r#type: "text",
                        value: "{sam_host}",
                        placeholder: "Host",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.sam.host = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                }
            }
        }
    }
}
