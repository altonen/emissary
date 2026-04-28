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
pub fn TunnelsTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (exp_in_len, exp_in_cnt, exp_out_len, exp_out_cnt, transit_max, transit_enabled) = {
        let state = state.read();
        (
            state.settings.exploratory.inbound_len.clone().unwrap_or_default(),
            state.settings.exploratory.inbound_count.clone().unwrap_or_default(),
            state.settings.exploratory.outbound_len.clone().unwrap_or_default(),
            state.settings.exploratory.outbound_count.clone().unwrap_or_default(),
            state.settings.transit.max_tunnels.clone().unwrap_or_default(),
            state.settings.transit.enabled,
        )
    };

    rsx! {
        div {
            // exploratory tunnels
            div {
                class: "settings-section",
                div {
                    class: "settings-section-title", span { "Exploratory tunnels" }
                }
                div {
                    class: "sf-grid",
                    span { class: "sf-label", "Inbound" }
                    div {
                        class: "sf-pair",
                        span { class: "sf-pair-label", "length" }
                        input {
                            r#type: "text",
                            value: "{exp_in_len}",
                            placeholder: "3",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.exploratory.inbound_len = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                        span { class: "sf-pair-label", "count" }
                        input {
                            r#type: "text",
                            value: "{exp_in_cnt}",
                            placeholder: "2",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.exploratory.inbound_count = Some(e.value());
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
                            value: "{exp_out_len}",
                            placeholder: "3",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.exploratory.outbound_len = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                        span { class: "sf-pair-label", "count" }
                        input {
                            r#type: "text",
                            value: "{exp_out_cnt}",
                            placeholder: "2",
                            oninput: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.exploratory.outbound_count = Some(e.value());
                                state.settings.dirty = true;
                            }
                        }
                    }
                }
            }

            // transit tunnels
            div {
                class: if transit_enabled {
                    "settings-section"
                } else {
                    "settings-section sf-disabled"
                },
                div {
                    class: "settings-section-title",
                    span { "Transit tunnels" }
                    label {
                        class: "checkbox-row",
                        input {
                            r#type: "checkbox",
                            checked: transit_enabled,
                            onchange: move |e: Event<FormData>| {
                                let mut state = state.write();
                                state.settings.transit.enabled = e.checked();
                                state.settings.dirty = true;
                            }
                        }
                        span { "Enable" }
                    }
                }
                div {
                    class: "sf-grid",
                    span { class: "sf-label", "Max tunnels" }
                    input {
                        r#type: "text",
                        class: if !transit_max.is_empty() && transit_max.parse::<u16>().is_err() {
                            "sf-input-short input-error"
                        } else {
                            "sf-input-short"
                        },
                        value: "{transit_max}",
                        placeholder: "5000",
                        oninput: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.transit.max_tunnels = Some(e.value());
                            state.settings.dirty = true;
                        }
                    }
                }
            }
        }
    }
}
