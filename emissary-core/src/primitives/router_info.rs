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
    primitives::{Capabilities, Date, Mapping, RouterAddress, RouterIdentity, Str, LOG_TARGET},
    runtime::Runtime,
};

use bytes::{BufMut, BytesMut};
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
        ntcp2: Option<RouterAddress>,
        ssu2: Option<RouterAddress>,
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

        options.insert(Str::from("router.version"), Str::from("0.9.62"));
        options.insert(Str::from("caps"), caps.clone());

        RouterInfo {
            addresses: {
                let mut addresses = Vec::new();

                if let Some(ntcp2) = ntcp2 {
                    addresses.push(ntcp2);
                }

                if let Some(ssu2) = ssu2 {
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
        let (rest, addresses) = (0..num_addresses)
            .try_fold(
                (rest, Vec::<RouterAddress>::new()),
                |(rest, mut addresses), _| {
                    let (rest, address) = RouterAddress::parse_frame::<R>(rest).ok()?;
                    addresses.push(address);

                    Some((rest, addresses))
                },
            )
            .ok_or(Err::Error(RouterInfoParseError::InvalidBitstream))?;

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

    /// Try to locate SSU2 over IPv4.
    pub fn ssu2_ipv4(&self) -> Option<&RouterAddress> {
        self.addresses.iter().find(|address| match address {
            RouterAddress::Ssu2 { socket_address, .. } =>
                socket_address.is_some_and(|address| address.is_ipv4()),
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
}

#[cfg(test)]
#[derive(Default)]
pub struct RouterInfoBuilder {
    floodfill: bool,
    static_key: Option<Vec<u8>>,
    signing_key: Option<Vec<u8>>,
    ntcp2: Option<crate::Ntcp2Config>,
    ssu2: Option<crate::Ssu2Config>,
}

#[cfg(test)]
impl RouterInfoBuilder {
    /// Mark the router as floodfill
    pub fn as_floodfill(mut self) -> Self {
        self.floodfill = true;
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
        use crate::{runtime::mock::MockRuntime, Ntcp2Config, Ssu2Config};
        use rand::Rng;

        let static_key = match self.static_key.take() {
            Some(key) => StaticPrivateKey::from_bytes(&key).unwrap(),
            None => StaticPrivateKey::random(MockRuntime::rng()),
        };
        let signing_key = match self.signing_key.take() {
            Some(key) => SigningPrivateKey::from_bytes(&key).unwrap(),
            None => SigningPrivateKey::random(MockRuntime::rng()),
        };
        let identity = RouterIdentity::from_keys::<MockRuntime>(&static_key, &signing_key)
            .expect("to succeed");

        let mut ntcp2 = match self.ntcp2.take() {
            None => None,
            Some(Ntcp2Config {
                port,
                host,
                publish,
                key,
                iv,
            }) => match (publish, host) {
                (true, Some(host)) => Some(RouterAddress::new_published_ntcp2(key, iv, port, host)),
                (_, _) => Some(RouterAddress::new_unpublished_ntcp2(key, port)),
            },
        };
        let mut ssu2 = match self.ssu2.take() {
            None => None,
            Some(Ssu2Config {
                port,
                host,
                publish,
                static_key,
                intro_key,
            }) => match (publish, host) {
                (true, Some(host)) => Some(RouterAddress::new_published_ssu2(
                    static_key, intro_key, port, host,
                )),
                (_, _) => Some(RouterAddress::new_unpublished_ssu2(
                    static_key, intro_key, port,
                )),
            },
        };

        // create default ntcp2 transport if neither transport was explicitly enabled
        if ntcp2.is_none() && ssu2.is_none() {
            let ntcp2_port = MockRuntime::rng().next_u32() as u16;
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

            ntcp2 = Some(RouterAddress::new_published_ntcp2(
                ntcp2_key,
                ntcp2_iv,
                ntcp2_port,
                ntcp2_host.parse().unwrap(),
            ));
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

        let mut addresses = Vec::<RouterAddress>::new();

        if let Some(ntcp2) = ntcp2.take() {
            addresses.push(ntcp2);
        }

        if let Some(ssu2) = ssu2.take() {
            addresses.push(ssu2);
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::parser::RouterIdentityParseError,
        runtime::{mock::MockRuntime, Runtime},
    };
    use std::{str::FromStr, time::Duration};

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
                8888,
                "127.0.0.1".parse().unwrap(),
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
                8888,
                "127.0.0.1".parse().unwrap(),
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
                8888,
                "127.0.0.1".parse().unwrap(),
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
            addresses: Vec::from_iter([RouterAddress::new_unpublished_ntcp2([1u8; 32], 8888)]),
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
            addresses: Vec::from_iter([RouterAddress::new_unpublished_ntcp2([1u8; 32], 8888)]),
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
                8888,
                "127.0.0.1".parse().unwrap(),
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
                8888,
                "127.0.0.1".parse().unwrap(),
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
                RouterAddress::new_unpublished_ntcp2([1u8; 32], 8888),
                RouterAddress::new_published_ssu2(
                    [1u8; 32],
                    [2u8; 32],
                    8888,
                    "127.0.0.1".parse().unwrap(),
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
                    8888,
                    "127.0.0.1".parse().unwrap(),
                ),
                RouterAddress::new_published_ssu2(
                    [1u8; 32],
                    [2u8; 32],
                    8888,
                    "127.0.0.1".parse().unwrap(),
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
                RouterAddress::new_unpublished_ntcp2([1u8; 32], 8888),
                RouterAddress::new_unpublished_ssu2([1u8; 32], [2u8; 32], 8888),
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
}
