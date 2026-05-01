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

use crate::ui::dioxus::{svg::*, types::AddressBookTab, AppState};

use dioxus::prelude::*;

pub mod add_destination;
pub mod browse;
pub mod configure;

#[component]
pub fn AddressBookView() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let active_tab = state.read().address_book.tab;

    rsx! {
        div {
            class: "page",
            div {
                class: "page-title",
                h1 { "Address book" }
                p { "Browse and configure your local address book" }
            }

            div {
                class: "settings-container",
                // tab bar
                div {
                    class: "tabs",
                    button {
                        class: if active_tab == AddressBookTab::Browse {
                            "tab-btn active"
                        } else {
                            "tab-btn"
                        },
                        onclick: move |_| {
                            state.write().address_book.tab = AddressBookTab::Browse;
                        },
                        span {
                            style: "display:inline-flex;width:18px;height:18px;",
                            dangerous_inner_html: SEARCH_SVG
                        }
                        "Browse"
                    }
                    button {
                        class: if active_tab == AddressBookTab::AddDestination {
                            "tab-btn active"
                        } else {
                            "tab-btn"
                        },
                        onclick: move |_| {
                            state.write().address_book.tab = AddressBookTab::AddDestination;
                        },
                        span {
                            style: "display:inline-flex;width:18px;height:18px;",
                            dangerous_inner_html: PERSON_ADD_SVG
                        }
                        "Add destination"
                    }
                    button {
                        class: if active_tab == AddressBookTab::Configure {
                            "tab-btn active"
                        } else {
                            "tab-btn"
                        },
                        onclick: move |_| {
                            state.write().address_book.tab = AddressBookTab::Configure;
                        },
                        span {
                            style: "display:inline-flex;width:18px;height:18px;",
                            dangerous_inner_html: SETTINGS_SVG
                        }
                        "Configure"
                    }
                }

                // tab content
                match active_tab {
                    AddressBookTab::Browse => rsx! { browse::BrowseTab {} },
                    AddressBookTab::AddDestination => rsx! { add_destination::AddDestinationTab {} },
                    AddressBookTab::Configure => rsx! { configure::ConfigureTab {} },
                }
            }
        }
    }
}
