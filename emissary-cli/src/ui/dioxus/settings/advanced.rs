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

use crate::{config::Theme, ui::dioxus::AppState};

use dioxus::prelude::*;

#[component]
pub fn AdvancedTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (floodfill, allow_local, insecure_tunnels, theme) = {
        let state = state.read();
        (
            state.settings.advanced.floodfill,
            state.settings.advanced.allow_local,
            state.settings.advanced.insecure_tunnels,
            state.theme,
        )
    };

    rsx! {
        div {
            // appearance
            div {
                class: "settings-section",
                div { class: "settings-section-title", "Appearance" }
                div {
                    class: "form-row",
                    style: "align-items:center;gap:16px;flex-wrap:wrap;",
                    span { style: "font-size:13px;color:var(--em-text-3);", "Theme" }
                    button {
                        class: if theme == Theme::Dark {
                            "btn btn-primary"
                        } else {
                            "btn btn-secondary"
                        },
                        style: "min-width:72px;",
                        onclick: move |_| {
                            let mut state = state.write();

                            state.theme = Theme::Dark;
                            state.settings.dirty = true;
                        },
                        "Dark"
                    }
                    button {
                        class: if theme == Theme::Light {
                            "btn btn-primary"
                        } else {
                            "btn btn-secondary"
                        },
                        style: "min-width:72px;",
                        onclick: move |_| {
                            let mut state = state.write();

                            state.theme = Theme::Light;
                            state.settings.dirty = true;
                        },
                        "Light"
                    }
                }
            }

            // netdb
            div {
                class: "settings-section",
                div { class: "settings-section-title", "NetDB" }
                label {
                    class: "checkbox-row",
                    input {
                        r#type: "checkbox",
                        checked: floodfill,
                        onchange: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.advanced.floodfill = e.checked();
                            state.settings.dirty = true;
                        }
                    }
                    span { "Run the router as floodfill" }
                }
            }

            // development
            div {
                class: "settings-section",
                div { class: "settings-section-title", "Development" }
                label {
                    class: "checkbox-row",
                    input {
                        r#type: "checkbox",
                        checked: allow_local,
                        onchange: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.advanced.allow_local = e.checked();
                            state.settings.dirty = true;
                        }
                    }
                    span { "Allow use of local addresses" }
                }
                label {
                    class: "checkbox-row",
                    input {
                        r#type: "checkbox",
                        checked: insecure_tunnels,
                        onchange: move |e: Event<FormData>| {
                            let mut state = state.write();
                            state.settings.advanced.insecure_tunnels = e.checked();
                            state.settings.dirty = true;
                        }
                    }
                    span { "Enable insecure tunnels" }
                }
            }
        }
    }
}
