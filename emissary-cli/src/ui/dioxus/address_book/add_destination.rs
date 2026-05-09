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

use crate::ui::dioxus::{types::AddDestinationStatus, AppState};

use dioxus::prelude::*;

#[component]
pub fn AddDestinationTab() -> Element {
    let mut state = use_context::<SyncSignal<AppState>>();

    let (hostname, destination, status) = {
        let state = state.read();

        (
            state.address_book.add_destination.hostname.clone(),
            state.address_book.add_destination.destination.clone(),
            state.address_book.add_destination.status.clone(),
        )
    };

    rsx! {
        div {
            h3 {
                style: "font-size:16px;color: var(--em-text-inv); margin-bottom: 10px",
                "Add new destination"
            }
            div { class: "settings-section",
                label { class: "field-label", "Hostname" }
                input {
                    r#type: "text",
                    value: "{hostname}",
                    placeholder: "",
                    oninput: move |e: Event<FormData>| {
                        state.write().address_book.add_destination.hostname = e.value();
                    }
                }
                label { class: "field-label", "Destination or Base32 address" }
                input {
                    r#type: "text",
                    value: "{destination}",
                    placeholder: "",
                    oninput: move |e: Event<FormData>| {
                        let mut state = state.write();
                        state.address_book.add_destination.destination = e.value();
                        state.address_book.add_destination.status = AddDestinationStatus::Idle;
                    }
                }
            }

            match &status {
                AddDestinationStatus::Idle => rsx! { },
                AddDestinationStatus::Saved => rsx! {
                    div { class: "status-ok", "Hostname added to address book" }
                },
                AddDestinationStatus::Error(e) => {
                    let e = e.clone();
                    rsx! {
                        div { class: "status-error", "{e}" }
                    }
                }
            }

            div { class: "button-row",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        let mut state = state.write();

                        match state.save_destination() {
                            Ok(()) => state.address_book.add_destination.status = AddDestinationStatus::Saved,
                            Err(e) => state.address_book.add_destination.status = AddDestinationStatus::Error(e),
                        }
                    },
                    "Save"
                }
            }
        }
    }
}
