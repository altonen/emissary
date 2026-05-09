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

use crate::ui::dioxus::{svg::*, AppState};

use dioxus::prelude::*;

use std::sync::Arc;

#[component]
pub fn BrowseTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (addresses, search_term, pending_delete) = {
        let state = state.read();

        (
            // TODO: not good
            state.address_book.browse.addresses.clone(),
            state.address_book.browse.search_term.clone(),
            state.address_book.browse.pending_delete_server.clone(),
        )
    };

    let filtered: Vec<(Arc<str>, Arc<str>)> = addresses
        .iter()
        .filter(|(k, _)| search_term.is_empty() || k.contains(&search_term))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    rsx! {
        div {
            h3 {
                style: "font-size:16px;color: var(--em-text-inv); margin-bottom: 10px",
                "Browse destinations"
            }

            div {
                class: "search-bar",
                span { dangerous_inner_html: SEARCH_SVG }
                input {
                    r#type: "text",
                    placeholder: "Search...",
                    value: "{search_term}",
                    oninput: move |e: Event<FormData>| {
                        state.write().address_book.browse.search_term = e.value();
                    }
                }
            }

            div {
                class: "table-header",
                style: "margin-top:10px;",
                span { class: "col-hostname", "Hostname" }
                span { class: "col-value", "Address" }
                span { class: "col-action", "Action" }
            }

            div {
                class: "table-scroll",
                if filtered.is_empty() {
                    div {
                        class: "empty-state",
                        span { class: "empty-state-icon", "📖" }
                        if search_term.is_empty() {
                            div {
                                class: "empty-state-title",
                                "Address book is empty"
                            }
                            div {
                                class: "empty-state-subtitle",
                                "Add destinations from the Add Destination tab"
                            }
                        } else {
                            div {
                                class: "empty-state-title",
                                "No results for \"{search_term}\""
                            }
                            div {
                                class: "empty-state-subtitle",
                                "Try a different search term"
                            }
                        }
                    }
                } else {
                    for (key, value) in filtered.iter() {
                        {
                            let key = key.clone();
                            let value = value.clone();
                            let value_for_copy: Arc<str> = value.clone();
                            let key_for_del: Arc<str> = key.clone();
                            let key_for_del2: Arc<str> = key.clone();
                            let is_pending = pending_delete.as_deref() == Some(key.as_ref());

                            rsx! {
                                div {
                                    class: "table-row",
                                    span { class: "col-hostname", "{key}" }
                                    span { class: "col-value", "{value}" }
                                    span { class: "col-action",
                                        if is_pending {
                                            span {
                                                class: "confirm-delete-row",
                                                "Delete?"
                                                button {
                                                    class: "btn-confirm-yes",
                                                    onclick: move |_| {
                                                        let mut state = state.write();
                                                        state.remove_host(key_for_del.clone());
                                                        state.address_book.browse.pending_delete_server = None;
                                                    },
                                                    "Yes"
                                                }
                                                button {
                                                    class: "btn-confirm-no",
                                                    onclick: move |_| {
                                                        let mut state = state.write();
                                                        state.address_book.browse.pending_delete_server = None;
                                                    },
                                                    "No"
                                                }
                                            }
                                        } else {
                                            button {
                                                class: "btn-icon",
                                                title: "Copy",
                                                onclick: move |_| {
                                                    let mut state = state.write();
                                                    state.copy_to_clipboard(&value_for_copy);
                                                    state.push_toast("Copied to clipboard");
                                                },
                                                span {
                                                    style: "display:inline-flex;width:18px;height:18px;",
                                                    dangerous_inner_html: CLIPBOARD_SVG
                                                }
                                            }
                                            button {
                                                class: "btn-icon",
                                                title: "Delete",
                                                onclick: move |_| {
                                                    let mut state = state.write();
                                                    state.address_book.browse.pending_delete_server = Some(key_for_del2.to_string());
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
        }
    }
}
