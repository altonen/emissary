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
    types::{HiddenServicesStatus, ServerTunnel},
    AppState,
};

use dioxus::prelude::*;

#[component]
pub fn CreateServerForm() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (server_name, server_port, server_path, error) = {
        let state = state.read();
        let error = match &state.hidden_services.status {
            Some(HiddenServicesStatus::CreateServer(Some(error))) => Some(error.clone()),
            _ => None,
        };

        (
            state.hidden_services.server.edit.name.clone(),
            state.hidden_services.server.edit.port.clone(),
            state.hidden_services.server.edit.path.clone(),
            error,
        )
    };

    rsx! {
        div {
            class: "modal-overlay",
            div {
                class: "modal",
                div { class: "modal-title", "Create a hidden service" }
                div {
                    class: "settings-section",
                    label { class: "field-label", "Name" }
                    input {
                        r#type: "text",
                        value: "{server_name}",
                        placeholder: "Name",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.name = e.value();
                        }
                    }
                    label { class: "field-label", "Port" }
                    input {
                        r#type: "text",
                        value: "{server_port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.port = e.value();
                        }
                    }
                    label { class: "field-label", "Destination path" }
                    input {
                        r#type: "text",
                        value: "{server_path}",
                        placeholder: "Path",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.path = e.value();
                        }
                    }
                }

                if let Some(error) = error {
                    div { class: "status-error", "{error}" }
                }

                div {
                    class: "modal-footer",
                    div {
                        class: "button-row",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let mut state = state.write();
                                match state.validate_server() {
                                    Ok(address) => {
                                        let name = state.hidden_services.server.edit.name.clone();
                                        let port = state.hidden_services.server.edit.port.clone();
                                        let path = state.hidden_services.server.edit.path.clone();

                                        state.hidden_services.server.servers.insert(name, ServerTunnel { port, path, address });
                                        state.hidden_services.server.edit.name.clear();
                                        state.hidden_services.server.edit.port.clear();
                                        state.hidden_services.server.edit.path.clear();
                                        state.hidden_services.status = None;

                                        state.save_servers();
                                        state.push_toast("Server tunnel saved");
                                    }
                                    Err(error) => {
                                        state.hidden_services.status = Some(HiddenServicesStatus::CreateServer(Some(error)));
                                    }
                                }
                            },
                            "Save"
                        }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let mut state = state.write();
                                state.hidden_services.status = None;
                                state.hidden_services.server.edit.name.clear();
                                state.hidden_services.server.edit.port.clear();
                                state.hidden_services.server.edit.path.clear();
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn EditServerForm() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (name, port, path, error) = {
        let state = state.read();
        let error = match &state.hidden_services.status {
            Some(HiddenServicesStatus::EditServer(Some(error))) => Some(error.clone()),
            _ => None,
        };
        (
            state.hidden_services.server.edit.name.clone(),
            state.hidden_services.server.edit.port.clone(),
            state.hidden_services.server.edit.path.clone(),
            error,
        )
    };

    rsx! {
        div {
            class: "modal-overlay",
            div {
                class: "modal",
                div { class: "modal-title", "Edit a hidden service" }
                div {
                    class: "settings-section",
                    label { class: "field-label", "Name" }
                    input {
                        r#type: "text",
                        value: "{name}",
                        placeholder: "Name",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.name = e.value();
                        }
                    }
                    label { class: "field-label", "Port" }
                    input {
                        r#type: "text",
                        value: "{port}",
                        placeholder: "Port",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.port = e.value();
                        }
                    }
                    label { class: "field-label", "Destination path" }
                    input {
                        r#type: "text",
                        value: "{path}",
                        placeholder: "Path",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.server.edit.path = e.value();
                        }
                    }
                }

                if let Some(error) = error {
                    div { class: "status-error", "{error}" }
                }

                div {
                    class: "modal-footer",
                    div {
                        class: "button-row",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let mut state = state.write();
                                match state.validate_server() {
                                    Ok(address) => {
                                        let old_name = state.hidden_services.server.edit.original_name.clone();
                                        let new_name = state.hidden_services.server.edit.name.clone();
                                        let port = state.hidden_services.server.edit.port.clone();
                                        let path = state.hidden_services.server.edit.path.clone();

                                        state.hidden_services.server.servers.remove(&old_name);
                                        state.hidden_services.server.servers.insert(new_name, ServerTunnel { port, path, address });
                                        state.hidden_services.server.edit.name.clear();
                                        state.hidden_services.server.edit.port.clear();
                                        state.hidden_services.server.edit.path.clear();
                                        state.hidden_services.server.edit.original_name.clear();
                                        state.hidden_services.status = None;

                                        state.save_servers();
                                        state.push_toast("Server tunnel saved");
                                    }
                                    Err(error) => {
                                        state.hidden_services.status = Some(HiddenServicesStatus::EditServer(Some(error)));
                                    }
                                }
                            },
                            "Save"
                        }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let mut state = state.write();
                                state.hidden_services.status = None;
                                state.hidden_services.server.edit.name.clear();
                                state.hidden_services.server.edit.port.clear();
                                state.hidden_services.server.edit.path.clear();
                                state.hidden_services.server.edit.original_name.clear();
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}
