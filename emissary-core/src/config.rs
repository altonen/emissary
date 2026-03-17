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

use core::net::{Ipv4Addr, Ipv6Addr};

use crate::{primitives::Str, profile::Profile, tunnel::TunnelPoolConfig};

use alloc::{string::String, vec::Vec};

/// Exploratory tunnel pool config.
#[derive(Clone, PartialEq, Eq)]
pub struct ExploratoryConfig {
    /// Length of an inbound exploratory tunnel.
    pub inbound_len: Option<usize>,

    /// Number of inbound exploratory tunnels.
    pub inbound_count: Option<usize>,

    /// Length of an outbound exploratory tunnel.
    pub outbound_len: Option<usize>,

    /// Number of outbound exploratory tunnels.
    pub outbound_count: Option<usize>,
}

impl From<Option<ExploratoryConfig>> for TunnelPoolConfig {
    fn from(value: Option<ExploratoryConfig>) -> Self {
        let default_config = TunnelPoolConfig::default();

        match value {
            None => default_config,
            Some(config) => TunnelPoolConfig {
                name: Str::from("exploratory"),
                num_inbound: config.inbound_count.unwrap_or(default_config.num_inbound),
                num_inbound_hops: config.inbound_len.unwrap_or(default_config.num_inbound_hops),
                num_outbound: config.outbound_count.unwrap_or(default_config.num_outbound),
                num_outbound_hops: config.outbound_len.unwrap_or(default_config.num_outbound_hops),
            },
        }
    }
}

/// NTCP2 configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct Ntcp2Config {
    /// Is IPV4 enabled.
    pub ipv4: bool,

    /// NTCP2 listen address for IPv4.
    pub ipv4_host: Option<Ipv4Addr>,

    /// Is IPV6 enabled.
    pub ipv6: bool,

    /// NTCP2 listen address for IPv6.
    pub ipv6_host: Option<Ipv6Addr>,

    /// NTCP2 IV.
    pub iv: [u8; 16],

    /// NTCP2 key.
    pub key: [u8; 32],

    /// NTCP2 port.
    pub port: u16,

    /// Should NTCP2 be published in router info.
    pub publish: bool,
}

/// SSU2 configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct Ssu2Config {
    /// SSU2 port.
    pub port: u16,

    /// SSU2 listen address.
    pub host: Option<Ipv4Addr>,

    /// Should SSU2 be published in router info.
    pub publish: bool,

    /// SSU2 static key.
    pub static_key: [u8; 32],

    /// SSU2 introduction key.
    pub intro_key: [u8; 32],
}

/// I2CP configuration.
#[derive(Debug, Clone)]
pub struct I2cpConfig {
    /// I2CP server listen port.
    pub port: u16,

    /// Host where the I2CP server shoud be bound to.
    pub host: String,
}

/// SAMv3 configuration.
#[derive(Debug, Clone)]
pub struct SamConfig {
    /// SAMv3 TCP server listen port.
    pub tcp_port: u16,

    /// SAMv3 UDP server listen port.
    pub udp_port: u16,

    /// Host where the SAM server shoud be bound to.
    pub host: String,
}

/// Metrics configuration.
#[derive(Default, Debug, Clone)]
pub struct MetricsConfig {
    /// Port where the metrics server should be bound to.
    pub port: u16,
}

/// Metrics configuration.
#[derive(Default, Debug, Clone)]
pub struct TransitConfig {
    /// Maximum number of transit tunnels.
    ///
    /// If `None`, there are no limit on transit tunnels.
    pub max_tunnels: Option<usize>,
}

/// Router configuration.
#[derive(Default)]
pub struct Config {
    /// Allow local addresses.
    pub allow_local: bool,

    /// Router capabilities.
    pub caps: Option<String>,

    /// Event refresh interval in seconds.
    pub refresh_interval: Option<usize>,

    /// Exploratory tunnel pool config.
    pub exploratory: Option<ExploratoryConfig>,

    /// Should the node be run as a floodfill router.
    pub floodfill: bool,

    /// I2CP configuration.
    ///
    /// `None` if I2CP is disabled.
    pub i2cp_config: Option<I2cpConfig>,

    /// Are tunnels allowed to be insecure.
    pub insecure_tunnels: bool,

    /// Metrics configuration.
    pub metrics: Option<MetricsConfig>,

    /// Network ID.
    pub net_id: Option<u8>,

    /// NTCP2 configuration.
    pub ntcp2: Option<Ntcp2Config>,

    /// SSU2 configuration.
    pub ssu2: Option<Ssu2Config>,

    /// Known router profiles.
    pub profiles: Vec<(String, Profile)>,

    /// Router Info, if it exists.
    pub router_info: Option<Vec<u8>>,

    /// Known routers.
    pub routers: Vec<Vec<u8>>,

    /// SAMv3 configuration.
    ///
    /// `None` if SAMv3 is disabled.
    pub samv3_config: Option<SamConfig>,

    /// Transit tunnel configuration.
    ///
    /// `None` if transit tunnels are disabled.
    pub transit: Option<TransitConfig>,

    /// Router signing key.
    pub signing_key: Option<[u8; 32]>,

    /// Router static key.
    pub static_key: Option<[u8; 32]>,
}
