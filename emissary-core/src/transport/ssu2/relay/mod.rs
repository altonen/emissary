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

#![allow(unused)]

use crate::{
    router::context::RouterContext, runtime::Runtime,
    transport::ssu2::relay::types::RelayTagRequested,
};

use hashbrown::HashSet;
use rand_core::RngCore;

pub mod types;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::relay";

/// Relay manager.
pub struct RelayManager<R: Runtime> {
    /// Relay tags currently in use.
    relay_tags: HashSet<u32>,

    /// Router context.
    router_ctx: RouterContext<R>,
}

impl<R: Runtime> RelayManager<R> {
    /// Create new `RelayManager`.
    pub fn new(router_ctx: RouterContext<R>) -> Self {
        Self {
            relay_tags: HashSet::new(),
            router_ctx,
        }
    }

    /// Allocate relay tag.
    pub fn allocate_relay_tag(&mut self) -> u32 {
        loop {
            let tag = R::rng().next_u32();

            if self.relay_tags.insert(tag) {
                return tag;
            }
        }
    }

    /// Deallocate relay tag.
    pub fn deallocate_relay_tag(&mut self, tag: u32) {
        self.relay_tags.remove(&tag);
    }

    pub fn register_relay_tag_request_result(&mut self, relay_tag_request: RelayTagRequested) {
        match relay_tag_request {
            RelayTagRequested::Yes(_) => {
                // TODO: do something here
            }
            RelayTagRequested::No(tag) => self.deallocate_relay_tag(tag),
        }
    }
}
