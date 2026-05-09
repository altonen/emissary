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
    hidden_services::{client, server},
    svg::*,
    types::HiddenServicesStatus,
    util::trim_display_name,
    AppState,
};

use dioxus::prelude::*;

#[component]
pub fn HiddenServiceOverview() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (servers, clients, pending_server, pending_client) = {
        let state = state.read();
        (
            state.hidden_services.server.servers.clone(),
            state.hidden_services.client.clients.clone(),
            state.hidden_services.server.pending_delete.clone(),
            state.hidden_services.client.pending_delete.clone(),
        )
    };
    let service_status = state.read().hidden_services.status.clone();

    rsx! {
        div {
            // server tunnels
            div {
                class: "card",
                style: "margin-bottom:16px;",
                h3 {
                    style: "font-size:18px;color: var(--em-text-inv); margin-bottom: 10px",
                    "Hidden services"
                }
                div {
                    class: "table-header",
                    span { class: "col-name", "Name" }
                    span { class: "col-port", "Port" }
                    span { class: "col-addr", "Address" }
                    span { class: "col-action", "Action" }
                }
                div {
                    class: "table-scroll",
                    if servers.is_empty() {
                        div {
                            class: "empty-state",
                            span { class: "empty-state-icon", "" }
                            div {
                                class:
                                "empty-state-title",
                                "No hidden services configured"
                            }
                            div {
                                class: "empty-state-subtitle",
                                "Click Create to add a hidden service"
                            }
                        }
                    } else {
                        for (name, svc) in &servers {
                            {
                                let name = name.clone();
                                let addr = svc.address.clone();
                                let port = svc.port.clone();
                                let name_clone = name.clone();
                                let addr_clone = addr.clone();
                                let is_pending = pending_server.as_deref() == Some(&name);

                                rsx! {
                                    div {
                                        class: "table-row",
                                        span {
                                            class: "col-name",
                                            span {
                                                class: "service-dot service-dot-running",
                                                title: "Running"
                                            }
                                            "{name}"
                                        }
                                        span { class: "col-port", "{port}" }
                                        span { class: "col-addr", "{addr}" }
                                        span { class: "col-action",
                                            if is_pending {
                                                span {
                                                    class: "confirm-delete-row",
                                                    "Delete?"
                                                    button {
                                                        class: "btn-confirm-yes",
                                                        onclick: {
                                                            let name = name.clone();
                                                            move |_| {
                                                                state.write().remove_server(&name);
                                                            }
                                                        },
                                                        "Yes"
                                                    }
                                                    button {
                                                        class: "btn-confirm-no",
                                                        onclick: move |_| {
                                                            state.write().hidden_services.server.pending_delete = None;
                                                        },
                                                        "No"
                                                    }
                                                }
                                            } else {
                                                button {
                                                    class: "btn-icon",
                                                    title: "Copy address",
                                                    onclick: move |_| {
                                                        let mut state = state.write();
                                                        state.copy_to_clipboard(&addr_clone);
                                                        state.push_toast("Copied to clipboard");
                                                    },
                                                    span {
                                                        style: "display:inline-flex;width:18px;height:18px;",
                                                        dangerous_inner_html: CLIPBOARD_SVG
                                                    }
                                                }
                                                button {
                                                    class: "btn-icon",
                                                    title: "Edit",
                                                    onclick: {
                                                        let name = name_clone.clone();
                                                        move |_| {
                                                            let (port, path) = {
                                                                let state = state.read();
                                                                if let Some(entry) = state.hidden_services.server.servers.get(&name) {
                                                                    (entry.port.clone(), entry.path.clone())
                                                                } else {
                                                                    return;
                                                                }
                                                            };

                                                            let mut state = state.write();
                                                            state.hidden_services.server.edit.name = name.clone();
                                                            state.hidden_services.server.edit.original_name = name.clone();
                                                            state.hidden_services.server.edit.port = port;
                                                            state.hidden_services.server.edit.path = path;
                                                            state.hidden_services.status = Some(HiddenServicesStatus::EditServer(None));
                                                        }
                                                    },
                                                    span {
                                                        style: "display:inline-flex;width:18px;height:18px;",
                                                        dangerous_inner_html: EDIT_SVG
                                                    }
                                                }
                                                button {
                                                    class: "btn-icon",
                                                    title: "Delete",
                                                    onclick: {
                                                        let name = name.clone();
                                                        move |_| {
                                                            state.write().hidden_services.server.pending_delete = Some(name.clone());
                                                        }
                                                    },
                                                    span {
                                                        style: "display:inline-flex;width:18px;height:18px;",
                                                        dangerous_inner_html: DELETE_SVG
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div {
                    class: "button-row",
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            state.write().hidden_services.status = Some(HiddenServicesStatus::CreateServer(None));
                        },
                        "Create"
                    }
                }
            }

            // client tunnels
            div {
                class: "card",
                h3 {
                    style: "font-size:18px;color: var(--em-text-inv); margin-bottom: 10px",
                    "Client tunnels"
                }
                div {
                    class: "table-header",
                    span { class: "col-name", "Name" }
                    span { class: "col-port", "Address" }
                    span { class: "col-port", "Port" }
                    span { class: "col-dest", "Destination" }
                    span { class: "col-port", "Dest port" }
                    span { class: "col-action", "Action" }
                }
                div {
                    class: "table-scroll",
                    if clients.is_empty() {
                        div {
                            class: "empty-state",
                            span { class: "empty-state-icon", "" }
                            div {
                                class: "empty-state-title",
                                "No client tunnels configured"
                            }
                            div {
                                class: "empty-state-subtitle",
                                "Click Create to add a client tunnel"
                            }
                        }
                    } else {
                        for (name, tunnel) in &clients {
                            {
                                let name = name.clone();
                                let dest = trim_display_name(&tunnel.destination);
                                let addr = tunnel.address.clone();
                                let port = tunnel.port.clone();
                                let dest_port = tunnel.destination_port.clone();
                                let name_for_edit = name.clone();
                                let is_pending = pending_client.as_deref() == Some(&name);

                                rsx! {
                                    div {
                                        class: "table-row",
                                        span {
                                            class: "col-name",
                                            span {
                                                class: "service-dot service-dot-running",
                                                title: "Running"
                                            }
                                            "{name}"
                                        }
                                        span { class: "col-port", "{addr}" }
                                        span { class: "col-port", "{port}" }
                                        span { class: "col-dest", "{dest}" }
                                        span { class: "col-port", "{dest_port}" }
                                        span { class: "col-action",
                                            if is_pending {
                                                span {
                                                    class: "confirm-delete-row",
                                                    "Delete?"
                                                    button {
                                                        class: "btn-confirm-yes",
                                                        onclick: {
                                                            let name = name.clone();
                                                            move |_| {
                                                                state.write().remove_client(&name);
                                                            }
                                                        },
                                                        "Yes"
                                                    }
                                                    button {
                                                        class: "btn-confirm-no",
                                                        onclick: move |_| {
                                                            state.write().hidden_services.client.pending_delete = None;
                                                        },
                                                        "No"
                                                    }
                                                }
                                            } else {
                                                button {
                                                    class: "btn-icon",
                                                    title: "Edit",
                                                    onclick: {
                                                        let name = name_for_edit.clone();
                                                        move |_| {
                                                            let (addr, port, dest, dest_port) = {
                                                                let state = state.read();
                                                                if let Some(entry) = state.hidden_services.client.clients.get(&name) {
                                                                    (
                                                                        entry.address.clone(),
                                                                        entry.port.clone(),
                                                                        entry.destination.clone(),
                                                                        entry.destination_port.clone(),
                                                                    )
                                                                } else {
                                                                    return;
                                                                }
                                                            };

                                                            let mut state = state.write();
                                                            state.hidden_services.client.edit.name = name.clone();
                                                            state.hidden_services.client.edit.original_name = name.clone();
                                                            state.hidden_services.client.edit.address = addr;
                                                            state.hidden_services.client.edit.port = port;
                                                            state.hidden_services.client.edit.destination = dest;
                                                            state.hidden_services.client.edit.destination_port = dest_port;
                                                            state.hidden_services.status = Some(HiddenServicesStatus::EditClient(None));
                                                        }
                                                    },
                                                    span {
                                                        style: "display:inline-flex;width:18px;height:18px;",
                                                        dangerous_inner_html: EDIT_SVG
                                                    }
                                                }
                                                button {
                                                    class: "btn-icon",
                                                    title: "Delete",
                                                    onclick: {
                                                        let name = name.clone();
                                                        move |_| {
                                                            state.write().hidden_services.client.pending_delete = Some(name.clone());
                                                        }
                                                    },
                                                    span {
                                                        style: "display:inline-flex;width:18px;height:18px;",
                                                        dangerous_inner_html: DELETE_SVG
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div {
                    class: "button-row",
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            state.write().hidden_services.status = Some(HiddenServicesStatus::CreateClient(None));
                        },
                        "Create"
                    }
                }
            }

            if let Some(status) = service_status {
                match status {
                    HiddenServicesStatus::CreateServer(_) => rsx! { server::CreateServerForm {} },
                    HiddenServicesStatus::EditServer(_) => rsx! { server::EditServerForm {} },
                    HiddenServicesStatus::CreateClient(_) => rsx! { client::CreateClientForm {} },
                    HiddenServicesStatus::EditClient(_) => rsx! { client::EditClientForm {} },
                }
            }
        }
    }
}
