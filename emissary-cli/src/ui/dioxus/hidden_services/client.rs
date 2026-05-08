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
    types::{ClientTunnel, HiddenServicesStatus},
    AppState,
};

use dioxus::prelude::*;

#[component]
pub fn CreateClientForm() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (name, addr, port, dest, dest_port, error) = {
        let state = state.read();
        let error = match &state.hidden_services.status {
            Some(HiddenServicesStatus::CreateClient(Some(error))) => Some(error.clone()),
            _ => None,
        };
        (
            state.hidden_services.client.edit.name.clone(),
            state.hidden_services.client.edit.address.clone(),
            state.hidden_services.client.edit.port.clone(),
            state.hidden_services.client.edit.destination.clone(),
            state.hidden_services.client.edit.destination_port.clone(),
            error,
        )
    };

    rsx! {
        div {
            class: "modal-overlay",
            div {
                class: "modal",
                div { class: "modal-title", "Create a client tunnel" }
                div {
                    class: "settings-section",
                    label { class: "field-label", "Name" }
                    input {
                        r#type: "text",
                        value: "{name}",
                        placeholder: "Name",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.name = e.value();
                        }
                    }
                    label { class: "field-label", "Local address" }
                    input {
                        r#type: "text",
                        value: "{addr}",
                        placeholder: "127.0.0.1",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.address = e.value();
                        }
                    }
                    label { class: "field-label", "Local port" }
                    input {
                        r#type: "text",
                        value: "{port}",
                        placeholder: "8888",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.port = e.value();
                        }
                    }
                    label { class: "field-label", "Destination" }
                    input {
                        r#type: "text",
                        value: "{dest}",
                        placeholder: ".i2p or .b32.i2p address",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.destination = e.value();
                        }
                    }
                    label { class: "field-label", "Destination port" }
                    input {
                        r#type: "text",
                        value: "{dest_port}",
                        placeholder: "80",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.destination_port = e.value();
                        }
                    }
                }

                if let Some(error) = error {
                    div {
                        class: "status-error", "{error}"
                    }
                }

                div {
                    class: "modal-footer",
                    div {
                        class: "button-row",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let mut state = state.write();
                                match state.validate_client() {
                                    Ok(()) => {
                                        let name = state.hidden_services.client.edit.name.clone();
                                        let address = state.hidden_services.client.edit.address.clone();
                                        let port = state.hidden_services.client.edit.port.clone();
                                        let dest = state.hidden_services.client.edit.destination.clone();
                                        let dest_port = state.hidden_services.client.edit.destination_port.clone();
                                        state.hidden_services.client.clients.insert(name, ClientTunnel {
                                            address,
                                            port,
                                            destination: dest,
                                            destination_port: dest_port,
                                        });

                                        state.hidden_services.client.edit.name.clear();
                                        state.hidden_services.client.edit.address.clear();
                                        state.hidden_services.client.edit.port.clear();
                                        state.hidden_services.client.edit.destination.clear();
                                        state.hidden_services.client.edit.destination_port.clear();
                                        state.hidden_services.status = None;

                                        state.save_clients();
                                        state.push_toast("Client tunnel saved");
                                    }
                                    Err(error) => {
                                        state.hidden_services.status = Some(HiddenServicesStatus::CreateClient(Some(error)));
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
                                state.hidden_services.client.edit.name.clear();
                                state.hidden_services.client.edit.address.clear();
                                state.hidden_services.client.edit.port.clear();
                                state.hidden_services.client.edit.destination.clear();
                                state.hidden_services.client.edit.destination_port.clear();
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
pub fn EditClientForm() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (name, addr, port, dest, dest_port, error) = {
        let state = state.read();
        let error = match &state.hidden_services.status {
            Some(HiddenServicesStatus::EditClient(Some(error))) => Some(error.clone()),
            _ => None,
        };

        (
            state.hidden_services.client.edit.name.clone(),
            state.hidden_services.client.edit.address.clone(),
            state.hidden_services.client.edit.port.clone(),
            state.hidden_services.client.edit.destination.clone(),
            state.hidden_services.client.edit.destination_port.clone(),
            error,
        )
    };

    rsx! {
        div {
            class: "modal-overlay",
            div {
                class: "modal",
                div { class: "modal-title", "Edit a client tunnel" }
                div {
                    class: "settings-section",
                    label { class: "field-label", "Name" }
                    input {
                        r#type: "text",
                        value: "{name}",
                        placeholder: "Name",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.name = e.value();
                        }
                    }
                    label { class: "field-label", "Local address" }
                    input {
                        r#type: "text",
                        value: "{addr}",
                        placeholder: "127.0.0.1",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.address = e.value();
                        }
                    }
                    label { class: "field-label", "Local port" }
                    input {
                        r#type: "text",
                        value: "{port}",
                        placeholder: "8888",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.port = e.value();
                        }
                    }
                    label { class: "field-label", "Destination" }
                    input {
                        r#type: "text",
                        value: "{dest}",
                        placeholder: ".i2p or .b32.i2p address",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.destination = e.value();
                        }
                    }
                    label { class: "field-label", "Destination port" }
                    input {
                        r#type: "text",
                        value: "{dest_port}",
                        placeholder: "80",
                        oninput: move |e: Event<FormData>| {
                            state.write().hidden_services.client.edit.destination_port = e.value();
                        }
                    }
                }

                if let Some(error) = error {
                    div {
                        class: "status-error", "{error}" }
                }

                div {
                    class: "modal-footer",
                    div {
                        class: "button-row",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                let mut state = state.write();
                                match state.validate_client() {
                                    Ok(()) => {
                                        let old_name = state.hidden_services.client.edit.original_name.clone();
                                        let new_name = state.hidden_services.client.edit.name.clone();
                                        let address = state.hidden_services.client.edit.address.clone();
                                        let port = state.hidden_services.client.edit.port.clone();
                                        let dest = state.hidden_services.client.edit.destination.clone();
                                        let dest_port = state.hidden_services.client.edit.destination_port.clone();
                                        state.hidden_services.client.clients.remove(&old_name);
                                        state.hidden_services.client.clients.insert(
                                            new_name,
                                            ClientTunnel {
                                            address,
                                            port,
                                            destination: dest,
                                            destination_port: dest_port,
                                        });

                                        state.hidden_services.client.edit.name.clear();
                                        state.hidden_services.client.edit.address.clear();
                                        state.hidden_services.client.edit.port.clear();
                                        state.hidden_services.client.edit.destination.clear();
                                        state.hidden_services.client.edit.destination_port.clear();
                                        state.hidden_services.client.edit.original_name.clear();
                                        state.hidden_services.status = None;

                                        state.save_clients();
                                        state.push_toast("Client tunnel saved");
                                    }
                                    Err(error) => {
                                        state.hidden_services.status = Some(HiddenServicesStatus::EditClient(Some(error)));
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
                                state.hidden_services.client.edit.name.clear();
                                state.hidden_services.client.edit.address.clear();
                                state.hidden_services.client.edit.port.clear();
                                state.hidden_services.client.edit.destination.clear();
                                state.hidden_services.client.edit.destination_port.clear();
                                state.hidden_services.client.edit.original_name.clear();
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}
