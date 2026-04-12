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

use std::time::Instant;

/// Selected view in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSelection {
    Dashboard,
    Bandwidth,
    HiddenServices,
    AddressBook,
    Settings,
}

/// Router status.
pub enum RouterStatus {
    Active,
    ShuttingDown,
}

/// Router state.
#[derive(Clone, Copy)]
pub struct RouterState {
    /// Inbound bandwidth.
    pub inbound_bandwidth: usize,

    /// Number of connected routers.
    pub num_routers: usize,

    /// Number of transit tunnels.
    pub num_transit_tunnels: usize,

    /// Number of tunnel build failures.
    pub num_tunnel_build_failures: usize,

    /// Number of tunnels built.
    pub num_tunnels_built: usize,

    /// Outbound bandwidth.
    pub outbound_bandwidth: usize,

    /// Router ID
    pub router_id: &'static str,

    /// Should router ID be displayed.
    pub show_router_id: bool,

    /// Router uptime.
    pub uptime: Instant,
}

impl RouterState {
    /// Create new `RouterState`.
    pub fn new(router_id: &'static str) -> Self {
        Self {
            inbound_bandwidth: 0usize,
            num_routers: 0usize,
            num_transit_tunnels: 0usize,
            num_tunnel_build_failures: 0usize,
            num_tunnels_built: 0usize,
            outbound_bandwidth: 0usize,
            router_id,
            show_router_id: false,
            uptime: Instant::now(),
        }
    }
}
