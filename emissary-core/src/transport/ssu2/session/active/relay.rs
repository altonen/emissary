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

//! Relay protocol for an active SSU2 session.

use crate::{
    runtime::Runtime,
    transport::ssu2::{
        message::data::RelayBlock, relay::types::RelayCommand, session::active::Ssu2Session,
    },
};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::active::relay";

impl<R: Runtime> Ssu2Session<R> {
    /// Handle command received from `RelayManager`.
    pub fn handle_relay_command(&mut self, command: RelayCommand) {
        match command {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                signature,
                token,
            } => {
                tracing::trace!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    ?nonce,
                    "send peer test request to bob",
                );

                self.transmission.schedule(RelayBlock::Response {
                    rejection,
                    message,
                    signature,
                    token,
                });
            }
            RelayCommand::RelayIntro {
                router_id,
                router_info,
                message,
                signature,
            } => {
                self.transmission.schedule((
                    RelayBlock::Intro {
                        router_id,
                        message,
                        signature,
                    },
                    router_info,
                ));
            }
            RelayCommand::Dummy => unreachable!(),
        }
    }
}
