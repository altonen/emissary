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

use crate::config::EmissaryConfig;

use std::{
    net::{Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};

/// NTCP2 config.
#[derive(Default, Clone)]
pub struct Ntcp2Config {
    pub port: Option<String>,
    pub ipv4_host: Option<String>,
    pub ipv6_host: Option<String>,
    pub publish_ipv4: Option<bool>,
    pub publish_ipv6: Option<bool>,
    pub ipv4: Option<bool>,
    pub ipv6: Option<bool>,
    pub ml_kem: Option<String>,
    pub disable_pq: Option<bool>,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for Ntcp2Config {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.ntcp2 else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            port: Some(config.port.to_string()),
            ipv4_host: config.ipv4_host.map(|address| address.to_string()),
            ipv6_host: config.ipv6_host.map(|address| address.to_string()),
            publish_ipv4: config.publish_ipv4,
            publish_ipv6: config.publish_ipv6,
            ipv4: config.ipv4,
            ipv6: config.ipv6,
            ml_kem: config.ml_kem.map(|ml_kem| ml_kem.to_string()),
            disable_pq: config.disable_pq,
            enabled: true,
        }
    }
}

impl TryInto<Option<crate::config::Ntcp2Config>> for Ntcp2Config {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::Ntcp2Config>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::Ntcp2Config {
            port: match self.port {
                Some(port) =>
                    port.parse::<u16>().map_err(|_| String::from("Invalid NTCP2 port"))?,
                None => 0,
            },
            ipv4_host: match self.ipv4_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv4Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv4 address"))?,
                ),
            },
            ipv6_host: match self.ipv6_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv6Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv6 address"))?,
                ),
            },
            ipv4: self.ipv4,
            ipv6: self.ipv6,
            publish_ipv4: self.publish_ipv4,
            publish_ipv6: self.publish_ipv6,
            disable_pq: self.disable_pq,
            ml_kem: match self.ml_kem {
                None => None,
                Some(value) => {
                    let value = value.parse::<usize>().expect("valid value");

                    if value < 3 || value > 5 {
                        return Err(String::from("ML-KEM only accepts 3, 4 or 5"));
                    }

                    Some(value)
                }
            },
        }))
    }
}

/// SSU2 config.
#[derive(Default, Clone)]
pub struct Ssu2Config {
    pub disable_pq: Option<bool>,
    pub enabled: bool,
    pub ipv4_host: Option<String>,
    pub ipv4_mtu: Option<String>,
    pub ipv4: Option<bool>,
    pub ipv6_host: Option<String>,
    pub ipv6_mtu: Option<String>,
    pub ipv6: Option<bool>,
    pub ml_kem: Option<String>,
    pub port: Option<String>,
    pub publish_ipv4: Option<bool>,
    pub publish_ipv6: Option<bool>,
}

impl TryInto<Option<crate::config::Ssu2Config>> for Ssu2Config {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::Ssu2Config>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::Ssu2Config {
            port: match self.port {
                Some(port) =>
                    port.parse::<u16>().map_err(|_| String::from("Invalid NTCP2 port"))?,
                None => 0,
            },
            ipv4_host: match self.ipv4_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv4Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv4 address"))?,
                ),
            },
            ipv6_host: match self.ipv6_host.as_ref() {
                None => None,
                Some(host) if host.is_empty() => None,
                Some(host) => Some(
                    host.parse::<Ipv6Addr>()
                        .map_err(|_| String::from("Invalid NTCP2 IPv6 address"))?,
                ),
            },
            ipv4_mtu: match self.ipv4_mtu {
                None => None,
                Some(mtu) if mtu.is_empty() => None,
                Some(mtu) => Some(
                    mtu.parse::<usize>().map_err(|_| String::from("IPv4 MTU must be a number"))?,
                ),
            },
            ipv6_mtu: match self.ipv6_mtu {
                None => None,
                Some(mtu) if mtu.is_empty() => None,
                Some(mtu) => Some(
                    mtu.parse::<usize>().map_err(|_| String::from("IPv6 MTU must be a number"))?,
                ),
            },
            ipv4: self.ipv4,
            ipv6: self.ipv6,
            publish_ipv4: self.publish_ipv4,
            publish_ipv6: self.publish_ipv6,
            disable_pq: self.disable_pq,
            ml_kem: match self.ml_kem {
                None => None,
                Some(value) => match &*value {
                    "3" | "4" | "3,4" | "4,3" => Some(value),
                    _ => return Err(String::from("Invalid ML-KEM")),
                },
            },
        }))
    }
}

impl From<&EmissaryConfig> for Ssu2Config {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.ssu2 else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            port: Some(config.port.to_string()),
            ipv4_host: config.ipv4_host.map(|address| address.to_string()),
            ipv4_mtu: config.ipv4_mtu.map(|mtu| mtu.to_string()),
            ipv6_host: config.ipv6_host.map(|address| address.to_string()),
            ipv6_mtu: config.ipv6_mtu.map(|mtu| mtu.to_string()),
            publish_ipv4: config.publish_ipv4,
            publish_ipv6: config.publish_ipv6,
            ipv4: config.ipv4,
            ipv6: config.ipv6,
            ml_kem: config.ml_kem.clone(),
            disable_pq: config.disable_pq,
            enabled: true,
        }
    }
}

/// Port forwarding config.
#[derive(Clone)]
pub struct PortForwardingConfig {
    pub nat_pmp: bool,
    pub upnp: bool,
}

impl From<&EmissaryConfig> for PortForwardingConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.port_forwarding else {
            return Self {
                nat_pmp: false,
                upnp: false,
            };
        };

        Self {
            nat_pmp: config.nat_pmp,
            upnp: config.upnp,
        }
    }
}

impl TryInto<Option<crate::config::PortForwardingConfig>> for PortForwardingConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::PortForwardingConfig>, Self::Error> {
        if !self.upnp && !self.nat_pmp {
            return Ok(None);
        }

        Ok(Some(crate::config::PortForwardingConfig {
            nat_pmp: self.nat_pmp,
            upnp: self.upnp,
            name: "emissary".to_string(),
        }))
    }
}

/// Save router configuration to disk.
pub fn save_router_config(path: PathBuf, config: &EmissaryConfig) {
    if let Ok(serialized) = toml::to_string(config) {
        let _ = std::fs::write(path, serialized);
    }
}
