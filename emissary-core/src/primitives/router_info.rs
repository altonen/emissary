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

use crate::{
    config::Config,
    crypto::{SigningPrivateKey, StaticPrivateKey},
    error::parser::RouterInfoParseError,
    primitives::{
        router_address::TransportKind, Capabilities, Date, Mapping, RouterAddress, RouterIdentity,
        Str, LOG_TARGET,
    },
    runtime::Runtime,
};

use bytes::{BufMut, BytesMut};
use hashbrown::HashSet;
use nom::{number::complete::be_u8, Err, IResult};

use alloc::{string::ToString, vec::Vec};

/// Signature length.
const SIGNATURE_LEN: usize = 64usize;

/// Router information
#[derive(Debug, Clone)]
pub struct RouterInfo {
    /// Router addresses.
    pub addresses: Vec<RouterAddress>,

    /// Router capabilities.
    pub capabilities: Capabilities,

    /// Router identity.
    pub identity: RouterIdentity,

    /// Network ID.
    pub net_id: u8,

    /// Router options.
    pub options: Mapping,

    /// When the router info was published.
    pub published: Date,
}

impl RouterInfo {
    /// Create new [`RouterInfo`].
    ///
    /// `ntcp2` is `Some` if NTCP has been enabled.
    pub fn new<R: Runtime>(
        config: &Config,
        ntcp2_ipv4: Option<RouterAddress>,
        ntcp2_ipv6: Option<RouterAddress>,
        ssu2_ipv4: Option<RouterAddress>,
        ssu2_ipv6: Option<RouterAddress>,
        static_key: &StaticPrivateKey,
        signing_key: &SigningPrivateKey,
        transit_tunnels_disabled: bool,
    ) -> Self {
        let Config {
            caps, router_info, ..
        } = config;

        let identity = match router_info {
            None => {
                tracing::debug!(
                    target: LOG_TARGET,
                    "generating new router identity",
                );

                RouterIdentity::from_keys::<R>(static_key, signing_key).expect("to succeed")
            }
            Some(router_info) => RouterIdentity::parse(router_info).expect("to succeed"),
        };

        let mut options = Mapping::default();
        options.insert(
            Str::from("netId"),
            config
                .net_id
                .map_or_else(|| Str::from("2"), |value| Str::from(value.to_string())),
        );

        let caps = match transit_tunnels_disabled {
            true => Str::from("G"),
            false => match caps {
                Some(caps) => Str::from(caps.clone()),
                None => match config.floodfill {
                    true => Str::from("Xf"),
                    false => Str::from("L"),
                },
            },
        };

        options.insert(Str::from("router.version"), Str::from("0.9.68"));
        options.insert(Str::from("caps"), caps.clone());

        RouterInfo {
            addresses: {
                let mut addresses = Vec::new();

                if let Some(ntcp2) = ntcp2_ipv4 {
                    addresses.push(ntcp2);
                }

                if let Some(ntcp2) = ntcp2_ipv6 {
                    addresses.push(ntcp2);
                }

                if let Some(ssu2) = ssu2_ipv4 {
                    addresses.push(ssu2);
                }

                if let Some(ssu2) = ssu2_ipv6 {
                    addresses.push(ssu2);
                }

                addresses
            },
            capabilities: Capabilities::parse(&caps).expect("to succeed"),
            identity,
            net_id: config.net_id.unwrap_or(2),
            options,
            published: Date::new(R::time_since_epoch().as_millis() as u64),
        }
    }

    fn parse_frame<R: Runtime>(input: &[u8]) -> IResult<&[u8], RouterInfo, RouterInfoParseError> {
        let (rest, identity) = RouterIdentity::parse_frame(input).map_err(Err::convert)?;
        let (rest, published) = Date::parse_frame(rest).map_err(Err::convert)?;
        let (rest, num_addresses) = be_u8(rest)?;
        let (rest, mut addresses) = (0..num_addresses)
            .try_fold(
                (rest, Vec::<RouterAddress>::new()),
                |(rest, mut addresses), _| match RouterAddress::parse_frame::<R>(rest) {
                    Ok((rest, address)) => {
                        addresses.push(address);
                        Some((rest, addresses))
                    }
                    Err(nom::Err::Error((Some(rest), error)))
                    | Err(nom::Err::Failure((Some(rest), error))) => {
                        tracing::debug!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to parse address into a supported address",
                        );
                        Some((rest, addresses))
                    }
                    Err(nom::Err::Error((None, error))) | Err(nom::Err::Failure((None, error))) => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to parse router address",
                        );
                        None
                    }
                    Err(nom::Err::Incomplete(error)) => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?error,
                            "failed to parse router address",
                        );
                        None
                    }
                },
            )
            .ok_or(Err::Error(RouterInfoParseError::InvalidBitstream))?;

        if addresses.is_empty() {
            return Err(Err::Error(RouterInfoParseError::NoAddresses));
        }

        // sort addresses by their costs so the most preferred address will be
        // the first item when iterating over addresses
        addresses.sort_by_key(|router_info| router_info.cost());

        // ignore `peer_size`
        let (rest, _) = be_u8(rest)?;
        let (rest, options) = Mapping::parse_frame(rest).map_err(Err::convert)?;

        let capabilities = match options.get(&Str::from("caps")) {
            None => return Err(Err::Error(RouterInfoParseError::CapabilitiesMissing)),
            Some(caps) => match Capabilities::parse(caps) {
                Some(caps) => caps,
                None => {
                    return Err(Err::Error(RouterInfoParseError::InvalidCapabilities(
                        caps.clone(),
                    )));
                }
            },
        };

        let net_id = match options.get(&Str::from("netId")) {
            None => return Err(Err::Error(RouterInfoParseError::NetIdMissing)),
            Some(net_id) => net_id
                .parse::<u8>()
                .map_err(|_| Err::Error(RouterInfoParseError::NetIdMissing))?,
        };

        identity
            .signing_key()
            .verify(&input[..input.len() - SIGNATURE_LEN], rest)
            .map_err(|_| Err::Error(RouterInfoParseError::InvalidSignature))?;

        Ok((
            rest,
            RouterInfo {
                identity,
                published,
                addresses,
                options,
                capabilities,
                net_id,
            },
        ))
    }

    /// Serialize [`RouterInfo`] into a byte vector.
    pub fn serialize(&self, signing_key: &SigningPrivateKey) -> Vec<u8> {
        let identity = self.identity.serialize();
        let published = self.published.serialize();
        let addresses =
            self.addresses.iter().map(|address| address.serialize()).collect::<Vec<_>>();
        let addresses_size = addresses.iter().fold(0, |acc, address| acc + address.len());
        let options = self.options.serialize();

        let size = identity
            .len()
            .saturating_add(published.len())
            .saturating_add(1usize) // field for router address count
            .saturating_add(addresses_size)
            .saturating_add(options.len())
            .saturating_add(1usize) // psize
            .saturating_add(64usize); // signature

        let mut out = BytesMut::with_capacity(size);

        out.put_slice(&identity);
        out.put_slice(&published);

        out.put_u8(addresses.len() as u8);
        addresses.into_iter().for_each(|address| out.put_slice(&address));

        out.put_u8(0u8); // psize
        out.put_slice(&options);

        let signature = signing_key.sign(&out[..size - 64]);
        out.put_slice(&signature);

        out.to_vec()
    }

    /// Try to parse router information from `bytes`.
    pub fn parse<R: Runtime>(bytes: impl AsRef<[u8]>) -> Result<Self, RouterInfoParseError> {
        Ok(Self::parse_frame::<R>(bytes.as_ref())?.1)
    }

    /// Returns `true` if the router is a floodfill router.
    pub fn is_floodfill(&self) -> bool {
        self.capabilities.is_floodfill()
    }

    /// Returns `true` if the router is considered reachable.
    ///
    /// Router is considered reachable if its caps don't specify otherwise and it has at least one
    /// published address.
    pub fn is_reachable(&self) -> bool {
        if !self.capabilities.is_reachable() {
            return false;
        }

        self.addresses.iter().any(|address| match address {
            RouterAddress::Ntcp2 {
                iv, socket_address, ..
            } => iv.is_some() && socket_address.is_some(),
            RouterAddress::Ssu2 { socket_address, .. } => socket_address.is_some(),
        })
    }

    /// Returns `true` if the router is considered reachable over SSU2
    pub fn is_reachable_ssu2(&self) -> bool {
        if !self.capabilities.is_reachable() {
            return false;
        }

        self.addresses.iter().any(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } => socket_address.is_some(),
            _ => false,
        })
    }

    /// Is the router usable.
    ///
    /// Any router who hasn't published `G` or `E` congestion caps is considered usable.
    pub fn is_usable(&self) -> bool {
        self.capabilities.is_usable()
    }

    /// Get network ID of the [`RouterInfo`].
    pub fn net_id(&self) -> u8 {
        self.net_id
    }

    /// Get an iterator over all supported addresses.
    pub fn addresses(&self) -> impl Iterator<Item = &RouterAddress> {
        self.addresses.iter()
    }

    /// Check if the router is reachable via NTCP2.
    pub fn is_reachable_ntcp2(&self) -> bool {
        self.addresses.iter().any(|address| match address {
            RouterAddress::Ntcp2 {
                iv, socket_address, ..
            } => iv.is_some() && socket_address.is_some(),
            _ => false,
        })
    }

    /// Try to locate NTCP2 over IPv4.
    pub fn ntcp2_ipv4(&self) -> Option<&RouterAddress> {
        self.addresses.iter().find(|address| match address {
            RouterAddress::Ntcp2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv4()),
            _ => false,
        })
    }

    /// Try to locate NTCP2 over IPv4 and return mutable reference to it.
    pub fn ntcp2_ipv4_mut(&mut self) -> Option<&mut RouterAddress> {
        self.addresses.iter_mut().find(|address| match address {
            RouterAddress::Ntcp2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv4()),
            _ => false,
        })
    }

    /// Try to locate NTCP2 over IPv6 and return mutable reference to it.
    pub fn ntcp2_ipv6_mut(&mut self) -> Option<&mut RouterAddress> {
        self.addresses.iter_mut().find(|address| match address {
            RouterAddress::Ntcp2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv6()),
            _ => false,
        })
    }

    /// Try to locate SSU2 over IPv4.
    pub fn ssu2_ipv4(&self) -> Option<&RouterAddress> {
        self.addresses.iter().find(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv4()),
            _ => false,
        })
    }

    /// Try to locate SSU2 over IPv6.
    pub fn ssu2_ipv6(&self) -> Option<&RouterAddress> {
        self.addresses.iter().find(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv6()),
            _ => false,
        })
    }

    /// Try to locate SSU2 over IPv4 and return mutable reference to it.
    pub fn ssu2_ipv4_mut(&mut self) -> Option<&mut RouterAddress> {
        self.addresses.iter_mut().find(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv4()),
            _ => false,
        })
    }

    /// Try to locate SSU2 over IPv6 and return mutable reference to it.
    pub fn ssu2_ipv6_mut(&mut self) -> Option<&mut RouterAddress> {
        self.addresses.iter_mut().find(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv6()),
            _ => false,
        })
    }

    /// Does the router support the relay protocol.
    pub fn supports_relay(&self) -> bool {
        self.addresses.iter().any(|address| address.supports_relay())
    }

    /// Attempt to select best transport for an outbound connection.
    ///
    /// `supported` contains the transports the local router supports.
    ///
    /// `None` is returned if no compatible transport is found.
    pub fn select_transport(&self, supported: &HashSet<TransportKind>) -> Option<&RouterAddress> {
        self.addresses.iter().fold(None, |selected, transport| {
            match transport.classify() {
                None => return selected,
                Some(kind) =>
                    if !supported.contains(&kind) {
                        return selected;
                    },
            }

            match selected {
                Some(selected) if selected.cost() <= transport.cost() => Some(selected),
                _ => Some(transport),
            }
        })
    }

    /// Attempt to select transport from router addresses using `filter`.
    pub fn select_transport_with_filter(
        &self,
        filter: impl Fn(&RouterAddress) -> bool,
    ) -> Option<&RouterAddress> {
        self.addresses.iter().find(|transport| filter(transport))
    }
}

#[cfg(test)]
#[allow(unused)]
pub(crate) mod builder {
    use super::*;
    use crate::{runtime::mock::MockRuntime, Ntcp2Config, Ssu2Config};
    use rand::{Rng, RngExt};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    #[allow(unused)]
    #[derive(Default)]
    pub struct RouterInfoBuilder {
        floodfill: bool,
        static_key: Option<Vec<u8>>,
        signing_key: Option<Vec<u8>>,
        ntcp2: Option<crate::Ntcp2Config>,
        ssu2: Option<crate::Ssu2Config>,
        ipv6: bool,
        mixed: bool,
    }

    #[allow(unused)]
    impl RouterInfoBuilder {
        /// Mark the router as floodfill
        pub fn as_floodfill(mut self) -> Self {
            self.floodfill = true;
            self
        }

        /// Make the router IPv6-only.
        pub fn with_ipv6(mut self) -> Self {
            self.ipv6 = true;
            self
        }

        /// Make the router suppport both IPv4 and IPv6.
        pub fn with_mixed(mut self) -> Self {
            self.mixed = true;
            self
        }

        /// Specify static key.
        pub fn with_static_key(mut self, static_key: Vec<u8>) -> Self {
            self.static_key = Some(static_key);
            self
        }

        /// Specify signing key.
        pub fn with_signing_key(mut self, signing_key: Vec<u8>) -> Self {
            self.signing_key = Some(signing_key);
            self
        }

        /// Specify NTCP configuration.
        pub fn with_ntcp2(mut self, ntcp2: crate::Ntcp2Config) -> Self {
            self.ntcp2 = Some(ntcp2);
            self
        }

        /// Specify SSU2 configuration.
        pub fn with_ssu2(mut self, ssu2: crate::Ssu2Config) -> Self {
            self.ssu2 = Some(ssu2);
            self
        }

        /// Build [`RouterInfoBuilder`] into a [`RouterInfo].
        pub fn build(&mut self) -> (RouterInfo, StaticPrivateKey, SigningPrivateKey) {
            let static_key = match self.static_key.take() {
                Some(key) => StaticPrivateKey::try_from_bytes(&key).unwrap(),
                None => StaticPrivateKey::random(MockRuntime::rng()),
            };
            let signing_key = match self.signing_key.take() {
                Some(key) => SigningPrivateKey::from_bytes(&key).unwrap(),
                None => SigningPrivateKey::random(MockRuntime::rng()),
            };
            let identity = RouterIdentity::from_keys::<MockRuntime>(&static_key, &signing_key)
                .expect("to succeed");
            let mut addresses = Vec::<RouterAddress>::new();

            if let Some(Ntcp2Config {
                ipv4,
                ipv4_host,
                ipv6,
                ipv6_host,
                iv,
                key,
                port,
                publish,
                ml_kem,
                disable_pq,
            }) = self.ntcp2.take()
            {
                if ipv4 {
                    match (publish, ipv4_host) {
                        (true, Some(host)) => addresses.push(RouterAddress::new_published_ntcp2(
                            key,
                            iv,
                            ml_kem,
                            false,
                            IpAddr::V4(host),
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                        )),
                        (_, _) => addresses.push(RouterAddress::new_unpublished_ntcp2(
                            key,
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
                        )),
                    }
                }

                if ipv6 {
                    match (publish, ipv6_host) {
                        (true, Some(host)) => addresses.push(RouterAddress::new_published_ntcp2(
                            key,
                            iv,
                            ml_kem,
                            false,
                            IpAddr::V6(host),
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
                        )),
                        (_, _) => addresses.push(RouterAddress::new_unpublished_ntcp2(
                            key,
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
                        )),
                    }
                }
            };

            if let Some(Ssu2Config {
                port,
                ipv4_host,
                ipv6_host,
                ipv4,
                ipv6,
                publish,
                static_key,
                intro_key,
                disable_pq,
                ml_kem,
                ..
            }) = self.ssu2.take()
            {
                if ipv4 {
                    match (publish, ipv4_host) {
                        (true, Some(host)) => addresses.push(RouterAddress::new_published_ssu2(
                            static_key,
                            intro_key,
                            ml_kem.clone(),
                            disable_pq,
                            IpAddr::V4(host),
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                            1500,
                        )),
                        (_, _) => addresses.push(RouterAddress::new_unpublished_ssu2(
                            static_key,
                            intro_key,
                            ml_kem.clone(),
                            disable_pq,
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
                            1500,
                        )),
                    }
                }

                if ipv6 {
                    match (publish, ipv6_host) {
                        (true, Some(host)) => addresses.push(RouterAddress::new_published_ssu2(
                            static_key,
                            intro_key,
                            ml_kem.clone(),
                            disable_pq,
                            IpAddr::V6(host),
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
                            1500,
                        )),
                        (_, _) => addresses.push(RouterAddress::new_unpublished_ssu2(
                            static_key,
                            intro_key,
                            ml_kem.clone(),
                            disable_pq,
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
                            1500,
                        )),
                    }
                }
            }

            // create default ntcp2 transport if neither transport was explicitly enabled
            if addresses.is_empty() {
                let ntcp2_port = MockRuntime::rng().next_u32() as u16;
                let ntcp2_key = {
                    let mut key_bytes = [0u8; 32];
                    MockRuntime::rng().fill_bytes(&mut key_bytes);

                    key_bytes
                };
                let ntcp2_iv = {
                    let mut iv_bytes = [0u8; 16];
                    MockRuntime::rng().fill_bytes(&mut iv_bytes);

                    iv_bytes
                };

                if self.mixed {
                    let ntcp2_host = format!(
                        "{}.{}.{}.{}",
                        {
                            loop {
                                let address = MockRuntime::rng().next_u32() % 256;

                                if address != 0 {
                                    break address;
                                }
                            }
                        },
                        MockRuntime::rng().next_u32() % 256,
                        MockRuntime::rng().next_u32() % 256,
                        MockRuntime::rng().next_u32() % 256,
                    );

                    addresses.push(RouterAddress::new_published_ntcp2(
                        ntcp2_key,
                        ntcp2_iv,
                        None,
                        false,
                        ntcp2_host.parse().unwrap(),
                        SocketAddr::new(IpAddr::V4(ntcp2_host.parse().unwrap()), ntcp2_port),
                    ));

                    let ntcp2_host =
                        Ipv6Addr::from(MockRuntime::rng().random::<u128>()).to_string();

                    addresses.push(RouterAddress::new_published_ntcp2(
                        ntcp2_key,
                        ntcp2_iv,
                        None,
                        false,
                        ntcp2_host.parse().unwrap(),
                        SocketAddr::new(IpAddr::V6(ntcp2_host.parse().unwrap()), ntcp2_port),
                    ));
                } else if self.ipv6 {
                    let ntcp2_host =
                        Ipv6Addr::from(MockRuntime::rng().random::<u128>()).to_string();

                    addresses.push(RouterAddress::new_published_ntcp2(
                        ntcp2_key,
                        ntcp2_iv,
                        None,
                        false,
                        ntcp2_host.parse().unwrap(),
                        SocketAddr::new(IpAddr::V6(ntcp2_host.parse().unwrap()), ntcp2_port),
                    ));
                } else {
                    let ntcp2_host = format!(
                        "{}.{}.{}.{}",
                        {
                            loop {
                                let address = MockRuntime::rng().next_u32() % 256;

                                if address != 0 {
                                    break address;
                                }
                            }
                        },
                        MockRuntime::rng().next_u32() % 256,
                        MockRuntime::rng().next_u32() % 256,
                        MockRuntime::rng().next_u32() % 256,
                    );

                    addresses.push(RouterAddress::new_published_ntcp2(
                        ntcp2_key,
                        ntcp2_iv,
                        None,
                        false,
                        ntcp2_host.parse().unwrap(),
                        SocketAddr::new(IpAddr::V4(ntcp2_host.parse().unwrap()), ntcp2_port),
                    ));
                }
            }

            let mut options = Mapping::default();
            options.insert("netId".into(), "2".into());
            options.insert("router.version".into(), "0.9.62".into());

            let capabilities = if self.floodfill {
                options.insert(Str::from("caps"), Str::from("XfR"));
                Capabilities::parse(&Str::from("XfR")).expect("to succeed")
            } else {
                options.insert(Str::from("caps"), Str::from("L"));
                Capabilities::parse(&Str::from("L")).expect("to succeed")
            };

            (
                RouterInfo {
                    addresses,
                    capabilities,
                    identity,
                    net_id: 2,
                    options,
                    published: Date::new(MockRuntime::rng().next_u64()),
                },
                static_key,
                signing_key,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::parser::RouterIdentityParseError,
        primitives::RouterId,
        runtime::{mock::MockRuntime, Runtime},
    };
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        str::FromStr,
        time::Duration,
    };

    // make router info with addresses
    fn make_router_info(addresses: Vec<RouterAddress>, caps: Option<Capabilities>) -> RouterInfo {
        let (identity, _sk, _sgk) = RouterIdentity::random();

        RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses,
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: caps.unwrap_or(Capabilities::parse(&Str::from("LU")).unwrap()),
        }
    }

    #[test]
    fn parse_router_1() {
        let router_info_bytes = include_bytes!("../../test-vectors/router1.dat");
        let router_info = RouterInfo::parse::<MockRuntime>(router_info_bytes).unwrap();

        assert_eq!(router_info.addresses.len(), 2);

        // ssu
        match router_info.ssu2_ipv4() {
            Some(RouterAddress::Ssu2 {
                cost,
                socket_address,
                ..
            }) => {
                assert_eq!(*cost, 5);
                assert_eq!(*socket_address, Some("2.36.209.134:23154".parse().unwrap()));
            }
            _ => panic!("ssu2 not found"),
        }

        // ntcp2
        match router_info.ntcp2_ipv4() {
            Some(RouterAddress::Ntcp2 {
                cost,
                socket_address,
                ..
            }) => {
                assert_eq!(*cost, 11);
                assert_eq!(*socket_address, Some("2.36.209.134:1403".parse().unwrap()));
            }
            _ => panic!("invalid ntcp2 address"),
        }

        // options
        assert_eq!(
            router_info.options.get(&Str::from_str("router.version").unwrap()),
            Some(&Str::from_str("0.9.64").unwrap())
        );
        assert_eq!(
            router_info.options.get(&Str::from_str("caps").unwrap()),
            Some(&Str::from_str("NRD").unwrap())
        );
        assert_eq!(
            router_info.options.get(&Str::from_str("netId").unwrap()),
            Some(&Str::from_str("2").unwrap())
        );
    }

    #[test]
    fn parse_router_2() {
        let router_info_bytes = include_bytes!("../../test-vectors/router2.dat");
        let router_info = RouterInfo::parse::<MockRuntime>(router_info_bytes).unwrap();

        assert_eq!(router_info.addresses.len(), 4);

        // ssu
        match router_info.ssu2_ipv4() {
            Some(RouterAddress::Ssu2 { cost, .. }) => {
                assert_eq!(*cost, 8);
            }
            _ => panic!("ssu2 not found"),
        }

        // ntcp2
        match router_info.ntcp2_ipv4() {
            Some(RouterAddress::Ntcp2 {
                cost,
                socket_address,
                ..
            }) => {
                assert_eq!(*cost, 3);
                assert_eq!(*socket_address, Some("64.53.67.11:25313".parse().unwrap()));
            }
            _ => panic!("invalid ntcp2 address"),
        }

        // options
        assert_eq!(
            router_info.options.get(&Str::from_str("router.version").unwrap()),
            Some(&Str::from_str("0.9.58").unwrap())
        );
        assert_eq!(
            router_info.options.get(&Str::from_str("caps").unwrap()),
            Some(&Str::from_str("XR").unwrap())
        );
        assert_eq!(
            router_info.options.get(&Str::from_str("netId").unwrap()),
            Some(&Str::from_str("2").unwrap())
        );
    }

    #[test]
    fn parse_router_3() {
        let router_info_bytes = include_bytes!("../../test-vectors/router3.dat");
        assert_eq!(
            RouterInfo::parse::<MockRuntime>(router_info_bytes).unwrap_err(),
            RouterInfoParseError::InvalidIdentity(RouterIdentityParseError::InvalidPublicKey(0))
        );
    }

    #[test]
    fn is_not_floodfill() {
        let router_info_bytes = include_bytes!("../../test-vectors/router2.dat");

        assert!(!RouterInfo::parse::<MockRuntime>(router_info_bytes).unwrap().is_floodfill())
    }

    #[test]
    fn is_floodfill() {
        let router_info_bytes = include_bytes!("../../test-vectors/router4.dat");

        assert!(RouterInfo::parse::<MockRuntime>(router_info_bytes).unwrap().is_floodfill())
    }

    #[test]
    fn net_id_missing() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_published_ntcp2(
                [1u8; 32],
                [2u8; 16],
                None,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([(Str::from("caps"), Str::from("L"))]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("L")).unwrap(),
        }
        .serialize(&sgk);

        assert_eq!(
            RouterInfo::parse::<MockRuntime>(&serialized).unwrap_err(),
            RouterInfoParseError::NetIdMissing
        );
    }

    #[test]
    fn caps_missing() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_published_ntcp2(
                [1u8; 32],
                [2u8; 16],
                None,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([(Str::from("netId"), Str::from("2"))]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("L")).unwrap(),
        }
        .serialize(&sgk);

        assert_eq!(
            RouterInfo::parse::<MockRuntime>(&serialized).unwrap_err(),
            RouterInfoParseError::CapabilitiesMissing
        );
    }

    #[test]
    fn hidden_router_not_reachable() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_published_ntcp2(
                [1u8; 32],
                [2u8; 16],
                None,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("HL")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("HL")).unwrap(),
        }
        .serialize(&sgk);

        assert!(!RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn unreachable_router_not_reachable() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_unpublished_ntcp2(
                [1u8; 32],
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("UL")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("UL")).unwrap(),
        }
        .serialize(&sgk);

        assert!(!RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn reachable_but_no_published_address() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_unpublished_ntcp2(
                [1u8; 32],
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("LR")).unwrap(),
        }
        .serialize(&sgk);

        assert!(!RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn reachable_explicitly_specified() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_published_ntcp2(
                [1u8; 32],
                [2u8; 16],
                None,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("LR")).unwrap(),
        }
        .serialize(&sgk);

        assert!(RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    // router doesn't explicitly specify the `R` flag
    #[test]
    fn maybe_reachable() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([RouterAddress::new_published_ntcp2(
                [1u8; 32],
                [2u8; 16],
                None,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            )]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("Xf")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("Xf")).unwrap(),
        }
        .serialize(&sgk);

        assert!(RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn ssu2_reachable() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([
                RouterAddress::new_unpublished_ntcp2(
                    [1u8; 32],
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                ),
                RouterAddress::new_published_ssu2(
                    [1u8; 32],
                    [2u8; 32],
                    None,
                    false,
                    "127.0.0.1".parse().unwrap(),
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                    1500,
                ),
            ]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("LR")).unwrap(),
        }
        .serialize(&sgk);

        assert!(RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn ntcp2_reachable() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([
                RouterAddress::new_published_ntcp2(
                    [1u8; 32],
                    [2u8; 16],
                    None,
                    false,
                    "127.0.0.1".parse().unwrap(),
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                ),
                RouterAddress::new_published_ssu2(
                    [1u8; 32],
                    [2u8; 32],
                    None,
                    false,
                    "127.0.0.1".parse().unwrap(),
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                    1500,
                ),
            ]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("LR")).unwrap(),
        }
        .serialize(&sgk);

        assert!(RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn both_transports_unpublished() {
        let (identity, _sk, sgk) = RouterIdentity::random();

        let serialized = RouterInfo {
            identity,
            published: Date::new(
                (MockRuntime::time_since_epoch() - Duration::from_secs(60)).as_millis() as u64,
            ),
            addresses: Vec::from_iter([
                RouterAddress::new_unpublished_ntcp2(
                    [1u8; 32],
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                ),
                RouterAddress::new_unpublished_ssu2(
                    [1u8; 32],
                    [2u8; 32],
                    None,
                    false,
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                    1500,
                ),
            ]),
            options: Mapping::from_iter([
                (Str::from("netId"), Str::from("2")),
                (Str::from("caps"), Str::from("LR")),
            ]),
            net_id: 2,
            capabilities: Capabilities::parse(&Str::from("LU")).unwrap(),
        }
        .serialize(&sgk);

        assert!(!RouterInfo::parse::<MockRuntime>(&serialized).unwrap().is_reachable());
    }

    #[test]
    fn select_transport() {
        // no compatible transport (ipv4/ipv6 mismatch)
        {
            // router supports ipv4
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ntcp2 {
                        cost: 8,
                        options: Mapping::default(),
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        iv: Some([0xaa; 16]),
                        socket_address: Some("127.0.0.1:8888".parse().unwrap()),
                    },
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                    },
                ],
                None,
            );

            // we support ipv6
            assert!(router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V6,
                    TransportKind::Ssu2V6
                ]))
                .is_none());
        }

        // no compatible transport (ntcp2/ssu2 mismatch)
        {
            // router only supports ssu2
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                    },
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 12,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                    },
                ],
                None,
            );

            // we only support ntcp2
            assert!(router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V4,
                    TransportKind::Ntcp2V6
                ]))
                .is_none());
        }

        // ntcp2 reachable/ssu2 reachable
        {
            // ntcp2 has lower cost
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ntcp2 {
                        cost: 8,
                        options: Mapping::default(),
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        iv: Some([0xaa; 16]),
                        socket_address: Some("127.0.0.1:8888".parse().unwrap()),
                    },
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                    },
                ],
                None,
            );

            match router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V4,
                    TransportKind::Ssu2V4,
                ]))
                .unwrap()
            {
                RouterAddress::Ntcp2 {
                    cost,
                    iv,
                    socket_address,
                    ..
                } => {
                    assert_eq!(*cost, 8);
                    assert_eq!(*iv, Some([0xaa; 16]));
                    assert_eq!(*socket_address, Some("127.0.0.1:8888".parse().unwrap()));
                }
                _ => panic!("expected ntcp2 to be chosen"),
            }
        }

        // ntcp2 unreachable/ssu2 reachable
        {
            // ntcp2 is not reachable, ssu2 reachable
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ntcp2 {
                        cost: 14,
                        options: Mapping::default(),
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        iv: None,
                        socket_address: None,
                    },
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                    },
                ],
                None,
            );

            match router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V4,
                    TransportKind::Ssu2V4,
                ]))
                .unwrap()
            {
                RouterAddress::Ssu2 {
                    cost,
                    socket_address,
                    ..
                } => {
                    assert_eq!(*cost, 10);
                    assert_eq!(*socket_address, Some("127.0.0.1:8889".parse().unwrap()));
                }
                _ => panic!("expected ssu2 to be chosen"),
            }
        }

        // ntcp2 unreachable/ssu2 unreachable (no introducers)
        {
            // ntcp2 and ssu2 not reachable
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ntcp2 {
                        cost: 14,
                        options: Mapping::default(),
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        iv: None,
                        socket_address: None,
                    },
                    RouterAddress::Ssu2 {
                        introducers: Vec::new(),
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::default(),
                        socket_address: None,
                    },
                ],
                None,
            );

            assert!(router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V4,
                    TransportKind::Ssu2V4,
                ]))
                .is_none());
        }

        // ntcp2 unreachable/ssu2 unreachable (with introducers)
        {
            // ntcp2 and ssu2 not reachable
            let router_info = make_router_info(
                vec![
                    RouterAddress::Ntcp2 {
                        cost: 14,
                        options: Mapping::default(),
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        iv: None,
                        socket_address: None,
                    },
                    RouterAddress::Ssu2 {
                        introducers: vec![(RouterId::random(), 1337)],
                        cost: 10,
                        mtu: 1500,
                        ml_kem: None,
                        static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                        intro_key: [0xbb; 32],
                        options: Mapping::from_iter([(Str::from("caps"), Str::from("4"))]),
                        socket_address: None,
                    },
                ],
                None,
            );

            match router_info
                .select_transport(&HashSet::from_iter([
                    TransportKind::Ntcp2V4,
                    TransportKind::Ssu2V4,
                ]))
                .unwrap()
            {
                RouterAddress::Ssu2 {
                    cost,
                    socket_address,
                    introducers,
                    ..
                } => {
                    assert_eq!(*cost, 10);
                    assert_eq!(introducers.len(), 1);
                    assert_eq!(*socket_address, None);
                }
                _ => panic!("expected ssu2 to be chosen"),
            }
        }
    }

    #[test]
    fn reachable_ssu2() {
        // reachable ssu2
        {
            let router_info = make_router_info(
                vec![RouterAddress::Ssu2 {
                    introducers: Vec::new(),
                    cost: 10,
                    mtu: 1500,
                    ml_kem: None,
                    static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                    intro_key: [0xbb; 32],
                    options: Mapping::default(),
                    socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                }],
                Some(Capabilities::parse(&Str::from("XR")).expect("to succeed")),
            );
            assert!(router_info.is_reachable_ssu2());
        }

        // unreachable ssu2
        {
            let router_info = make_router_info(
                vec![RouterAddress::Ssu2 {
                    introducers: Vec::new(),
                    cost: 10,
                    mtu: 1500,
                    ml_kem: None,
                    static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                    intro_key: [0xbb; 32],
                    options: Mapping::default(),
                    socket_address: None,
                }],
                Some(Capabilities::parse(&Str::from("XR")).expect("to succeed")),
            );
            assert!(!router_info.is_reachable_ssu2());
        }

        // address published but caps say `U`
        {
            let router_info = make_router_info(
                vec![RouterAddress::Ssu2 {
                    introducers: Vec::new(),
                    cost: 10,
                    mtu: 1500,
                    ml_kem: None,
                    static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                    intro_key: [0xbb; 32],
                    options: Mapping::default(),
                    socket_address: Some("127.0.0.1:8889".parse().unwrap()),
                }],
                Some(Capabilities::parse(&Str::from("XU")).expect("to succeed")),
            );
            assert!(!router_info.is_reachable_ssu2());
        }
    }

    #[tokio::test]
    async fn try_parse_router_info_with_ssu() {
        let input = [
            180, 197, 236, 26, 158, 239, 197, 249, 78, 249, 15, 29, 10, 61, 63, 131, 224, 207, 136,
            247, 82, 86, 80, 92, 22, 169, 63, 188, 167, 22, 6, 18, 222, 69, 135, 163, 17, 187, 10,
            156, 197, 115, 201, 182, 29, 165, 14, 159, 179, 16, 253, 246, 191, 1, 180, 10, 206,
            196, 34, 238, 155, 230, 23, 29, 132, 46, 170, 166, 235, 16, 37, 111, 83, 13, 23, 34,
            189, 250, 38, 0, 100, 130, 177, 128, 244, 148, 199, 254, 23, 114, 10, 6, 252, 131, 3,
            67, 165, 68, 98, 34, 115, 39, 147, 182, 111, 13, 109, 116, 249, 232, 181, 158, 184,
            137, 84, 227, 206, 50, 202, 39, 106, 229, 224, 140, 27, 17, 180, 176, 13, 218, 223, 2,
            127, 149, 62, 41, 1, 102, 246, 178, 79, 55, 191, 33, 231, 11, 106, 164, 109, 117, 166,
            54, 22, 197, 123, 163, 45, 31, 231, 246, 59, 90, 229, 58, 44, 254, 86, 243, 72, 117,
            21, 153, 172, 211, 134, 174, 21, 150, 218, 129, 197, 7, 151, 51, 239, 104, 22, 129,
            151, 238, 207, 13, 158, 221, 95, 221, 155, 19, 98, 166, 134, 81, 5, 203, 239, 65, 167,
            202, 191, 223, 167, 105, 61, 207, 165, 113, 154, 182, 41, 208, 68, 86, 227, 132, 25,
            30, 11, 81, 58, 245, 176, 107, 233, 246, 186, 240, 164, 196, 112, 171, 43, 240, 87,
            227, 63, 255, 164, 158, 150, 82, 136, 172, 89, 252, 194, 108, 210, 31, 229, 127, 29,
            34, 201, 249, 137, 200, 154, 69, 176, 106, 30, 103, 226, 225, 237, 222, 165, 232, 139,
            82, 117, 222, 168, 22, 131, 96, 7, 20, 183, 51, 128, 199, 24, 226, 160, 180, 234, 216,
            60, 128, 229, 94, 223, 105, 60, 79, 190, 43, 74, 250, 74, 180, 54, 128, 112, 232, 154,
            38, 28, 106, 138, 109, 56, 83, 9, 215, 33, 10, 73, 96, 159, 238, 69, 237, 175, 197, 50,
            102, 45, 94, 98, 215, 131, 159, 159, 59, 14, 240, 28, 83, 167, 220, 219, 36, 163, 29,
            214, 125, 9, 126, 55, 168, 97, 41, 144, 57, 33, 233, 224, 107, 23, 172, 100, 153, 90,
            57, 46, 97, 225, 155, 2, 47, 209, 115, 5, 0, 4, 0, 7, 0, 4, 0, 0, 1, 156, 175, 98, 165,
            212, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 5, 78, 84, 67, 80, 50, 0, 117, 4, 104, 111, 115,
            116, 61, 13, 56, 49, 46, 50, 51, 46, 49, 53, 55, 46, 49, 49, 53, 59, 1, 105, 61, 24,
            74, 115, 82, 126, 83, 48, 108, 56, 121, 106, 119, 114, 112, 126, 110, 111, 69, 79, 117,
            119, 109, 119, 61, 61, 59, 4, 112, 111, 114, 116, 61, 4, 52, 53, 54, 55, 59, 1, 115,
            61, 44, 109, 106, 79, 109, 75, 100, 107, 109, 68, 54, 54, 108, 99, 103, 116, 97, 81,
            82, 73, 88, 104, 45, 78, 71, 105, 77, 107, 85, 82, 106, 109, 85, 114, 109, 65, 79, 105,
            88, 118, 116, 98, 84, 103, 61, 59, 1, 118, 61, 1, 50, 59, 9, 0, 0, 0, 0, 0, 0, 0, 0, 3,
            83, 83, 85, 0, 94, 4, 99, 97, 112, 115, 61, 2, 66, 67, 59, 4, 104, 111, 115, 116, 61,
            13, 56, 49, 46, 50, 51, 46, 49, 53, 55, 46, 49, 49, 53, 59, 3, 107, 101, 121, 61, 44,
            57, 121, 111, 85, 108, 87, 113, 52, 73, 120, 84, 76, 113, 45, 105, 45, 99, 74, 107,
            107, 85, 110, 104, 76, 72, 105, 73, 77, 122, 103, 65, 116, 126, 113, 97, 86, 117, 107,
            108, 99, 74, 75, 119, 61, 59, 4, 112, 111, 114, 116, 61, 4, 52, 53, 54, 55, 59, 0, 0,
            44, 4, 99, 97, 112, 115, 61, 2, 88, 82, 59, 5, 110, 101, 116, 73, 100, 61, 1, 50, 59,
            14, 114, 111, 117, 116, 101, 114, 46, 118, 101, 114, 115, 105, 111, 110, 61, 6, 48, 46,
            57, 46, 53, 51, 59, 42, 54, 184, 214, 193, 97, 122, 90, 11, 26, 48, 212, 184, 238, 54,
            152, 79, 240, 81, 244, 62, 152, 3, 24, 171, 244, 67, 61, 112, 148, 34, 129, 36, 198,
            65, 237, 13, 190, 140, 46, 37, 41, 19, 19, 167, 207, 55, 177, 21, 5, 181, 72, 170, 73,
            206, 106, 39, 165, 112, 91, 192, 33, 39, 15,
        ];
        assert!(RouterInfo::parse::<MockRuntime>(input).is_ok());
    }
}
