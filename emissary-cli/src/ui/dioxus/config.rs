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
    num::NonZeroUsize,
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
            publish: None,
            disable_pq: self.disable_pq,
            ml_kem: match self.ml_kem {
                None => None,
                Some(value) => {
                    let value = value.parse::<usize>().expect("valid value");

                    if !(3..5).contains(&value) {
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
            publish: None,
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

/// I2CP config.
#[derive(Default, Clone)]
pub struct I2cpConfig {
    pub port: Option<String>,
    pub host: Option<String>,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for I2cpConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.i2cp else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            port: Some(config.port.to_string()),
            host: config.host.clone(),
            enabled: true,
        }
    }
}

impl TryInto<Option<crate::config::I2cpConfig>> for I2cpConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::I2cpConfig>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::I2cpConfig {
            port: self
                .port
                .ok_or_else(|| String::from("I2CP port missing"))?
                .parse::<u16>()
                .map_err(|_| String::from("Invalid I2CP port"))?,
            host: match self.host {
                None => None,
                Some(host) => Some(
                    host.parse::<Ipv4Addr>()
                        .map_err(|_| String::from("Invalid I2CP host"))?
                        .to_string(),
                ),
            },
        }))
    }
}

/// SAMv3 config.
#[derive(Default, Clone)]
pub struct SamConfig {
    pub tcp_port: Option<String>,
    pub udp_port: Option<String>,
    pub host: Option<String>,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for SamConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.sam else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            tcp_port: Some(config.tcp_port.to_string()),
            udp_port: Some(config.udp_port.to_string()),
            host: config.host.clone(),
            enabled: true,
        }
    }
}

impl TryInto<Option<crate::config::SamConfig>> for SamConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::SamConfig>, Self::Error> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::SamConfig {
            tcp_port: self
                .tcp_port
                .ok_or_else(|| String::from("SAMv3 TCP port missing"))?
                .parse::<u16>()
                .map_err(|_| String::from("Invalid SAMv3 TCP port"))?,
            udp_port: self
                .udp_port
                .ok_or_else(|| String::from("SAMv3 UDP port missing"))?
                .parse::<u16>()
                .map_err(|_| String::from("Invalid SAMv3 UDP port"))?,
            host: match self.host {
                None => None,
                Some(host) => Some(
                    host.parse::<Ipv4Addr>()
                        .map_err(|_| String::from("Invalid SAMv3 host"))?
                        .to_string(),
                ),
            },
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct TunnelConfig {
    pub inbound_len: Option<String>,
    pub inbound_count: Option<String>,
    pub outbound_len: Option<String>,
    pub outbound_count: Option<String>,
}

impl From<&Option<crate::config::TunnelConfig>> for TunnelConfig {
    fn from(value: &Option<crate::config::TunnelConfig>) -> Self {
        let Some(config) = value else {
            return Default::default();
        };

        Self {
            inbound_len: Some(config.inbound_len.to_string()),
            inbound_count: Some(config.inbound_count.to_string()),
            outbound_len: Some(config.outbound_len.to_string()),
            outbound_count: Some(config.outbound_count.to_string()),
        }
    }
}

impl TryInto<Option<crate::config::TunnelConfig>> for TunnelConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::TunnelConfig>, String> {
        if self.inbound_len.as_ref().is_none_or(|value| value.is_empty())
            && self.inbound_count.as_ref().is_none_or(|value| value.is_empty())
            && self.outbound_len.as_ref().is_none_or(|value| value.is_empty())
            && self.outbound_count.as_ref().is_none_or(|value| value.is_empty())
        {
            return Ok(None);
        }

        Ok(Some(crate::config::TunnelConfig {
            inbound_len: self
                .inbound_len
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(String::from("Invalid inbound tunnel length"))?,
            inbound_count: self
                .inbound_count
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(String::from("Invalid inbound tunnel count"))?,
            outbound_len: self
                .outbound_len
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(String::from("Invalid outbound tunnel length"))?,
            outbound_count: self
                .outbound_count
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(String::from("Invalid outbound tunnel count"))?,
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct I2cpOptions {
    pub encryption: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HttpProxyConfig {
    pub port: Option<String>,
    pub host: Option<String>,
    pub outproxy: Option<String>,
    pub tunnel_config: TunnelConfig,
    pub i2cp: I2cpOptions,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for HttpProxyConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.http_proxy else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            enabled: true,
            port: Some(config.port.to_string()),
            host: Some(config.host.clone()),
            outproxy: config.outproxy.clone(),
            tunnel_config: TunnelConfig::from(&config.tunnel_config),
            i2cp: match config.i2cp {
                None => I2cpOptions::default(),
                Some(ref config) => I2cpOptions {
                    encryption: config.lease_set_enc_type.clone(),
                },
            },
        }
    }
}

impl TryInto<Option<crate::config::HttpProxyConfig>> for HttpProxyConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::HttpProxyConfig>, String> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::HttpProxyConfig {
            port: match self.port {
                Some(port) =>
                    port.parse::<u16>().map_err(|_| String::from("Invalid HTTP proxy port"))?,
                None => 0,
            },
            host: {
                let host = self.host.ok_or_else(|| String::from("Invalid HTTP proxy host"))?;

                if host.is_empty() || host.parse::<Ipv4Addr>().is_err() {
                    return Err(String::from("Invalid HTTP proxy host"));
                }

                host
            },
            tunnel_config: self.tunnel_config.try_into()?,
            outproxy: {
                if self.outproxy.as_ref().is_some_and(|value| !value.is_empty()) {
                    self.outproxy
                } else {
                    None
                }
            },
            i2cp: match self.i2cp.encryption {
                None => None,
                Some(encryption) if encryption.is_empty() => None,
                Some(encryption) => Some(crate::config::I2cpOptions {
                    lease_set_enc_type: Some(encryption),
                }),
            },
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct SocksProxyConfig {
    pub port: Option<String>,
    pub host: Option<String>,
    pub outproxy: Option<String>,
    pub i2cp: I2cpOptions,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for SocksProxyConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.socks_proxy else {
            return Self {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            enabled: true,
            port: Some(config.port.to_string()),
            host: Some(config.host.clone()),
            outproxy: config.outproxy.clone(),
            i2cp: match config.i2cp {
                None => I2cpOptions::default(),
                Some(ref config) => I2cpOptions {
                    encryption: config.lease_set_enc_type.clone(),
                },
            },
        }
    }
}

impl TryInto<Option<crate::config::SocksProxyConfig>> for SocksProxyConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::SocksProxyConfig>, String> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(crate::config::SocksProxyConfig {
            port: match self.port {
                Some(port) =>
                    port.parse::<u16>().map_err(|_| String::from("Invalid SOCKS proxy port"))?,
                None => 0,
            },
            host: {
                let host = self.host.ok_or_else(|| String::from("Invalid SOCKS proxy host"))?;

                if host.is_empty() || host.parse::<Ipv4Addr>().is_err() {
                    return Err(String::from("Invalid SOCKS proxy host"));
                }

                host
            },
            outproxy: {
                if self.outproxy.as_ref().is_some_and(|value| !value.is_empty()) {
                    self.outproxy
                } else {
                    None
                }
            },
            i2cp: match self.i2cp.encryption {
                None => None,
                Some(encryption) if encryption.is_empty() => None,
                Some(encryption) => Some(crate::config::I2cpOptions {
                    lease_set_enc_type: Some(encryption),
                }),
            },
        }))
    }
}

#[derive(Debug, Clone)]
pub struct ExploratoryConfig {
    pub inbound_len: Option<String>,
    pub inbound_count: Option<String>,
    pub outbound_len: Option<String>,
    pub outbound_count: Option<String>,
}

impl From<&EmissaryConfig> for ExploratoryConfig {
    fn from(value: &EmissaryConfig) -> Self {
        match &value.exploratory {
            Some(config) => Self {
                inbound_len: Some(config.inbound_len.to_string()),
                inbound_count: Some(config.inbound_count.to_string()),
                outbound_len: Some(config.outbound_len.to_string()),
                outbound_count: Some(config.outbound_count.to_string()),
            },
            None => Self {
                inbound_len: None,
                inbound_count: None,
                outbound_len: None,
                outbound_count: None,
            },
        }
    }
}

impl TryInto<Option<crate::config::ExploratoryConfig>> for ExploratoryConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::ExploratoryConfig>, String> {
        if self.inbound_len.is_none()
            && self.inbound_count.is_none()
            && self.outbound_len.is_none()
            && self.outbound_count.is_none()
        {
            return Ok(None);
        }

        Ok(Some(crate::config::ExploratoryConfig {
            inbound_len: self
                .inbound_len
                .and_then(|x| x.parse::<NonZeroUsize>().ok())
                .ok_or(String::from("Invalid inbound tunnel length"))?
                .into(),
            inbound_count: self
                .inbound_count
                .and_then(|x| x.parse::<NonZeroUsize>().ok())
                .ok_or(String::from("Invalid inbound tunnel count"))?
                .into(),
            outbound_len: self
                .outbound_len
                .and_then(|x| x.parse::<NonZeroUsize>().ok())
                .ok_or(String::from("Invalid inbound tunnel length"))?
                .into(),
            outbound_count: self
                .outbound_count
                .and_then(|x| x.parse::<NonZeroUsize>().ok())
                .ok_or(String::from("Invalid inbound count length"))?
                .into(),
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransitConfig {
    pub max_tunnels: Option<String>,
    pub enabled: bool,
}

impl From<&EmissaryConfig> for TransitConfig {
    fn from(value: &EmissaryConfig) -> Self {
        let Some(ref config) = value.transit else {
            return TransitConfig {
                enabled: false,
                ..Default::default()
            };
        };

        Self {
            max_tunnels: config.max_tunnels.map(|v| v.to_string()),
            enabled: true,
        }
    }
}

impl TryInto<Option<crate::config::TransitConfig>> for TransitConfig {
    type Error = String;

    fn try_into(self) -> Result<Option<crate::config::TransitConfig>, String> {
        if !self.enabled {
            return Ok(None);
        }
        Ok(Some(crate::config::TransitConfig {
            max_tunnels: self
                .max_tunnels
                .map(|t| t.parse::<NonZeroUsize>().map(usize::from))
                .transpose()
                .map_err(|_| String::from("Invalid transit tunnel count"))?,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct AdvancedConfig {
    pub floodfill: bool,
    pub allow_local: bool,
    pub insecure_tunnels: bool,
}

impl From<&EmissaryConfig> for AdvancedConfig {
    fn from(value: &EmissaryConfig) -> Self {
        Self {
            floodfill: value.floodfill,
            allow_local: value.allow_local,
            insecure_tunnels: value.insecure_tunnels,
        }
    }
}
