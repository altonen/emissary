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
    crypto::{base64_decode, base64_encode, StaticPrivateKey, StaticPublicKey},
    error::parser::RouterAddressParseError,
    primitives::{Date, Mapping, RouterId, Str},
    runtime::Runtime,
};

use bytes::{BufMut, BytesMut};
use nom::{number::complete::be_u8, Err, IResult};

use alloc::{format, string::ToString, vec::Vec};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Maximum amount of introducers.
const MAX_INTRODUCERS: usize = 3usize;

/// Router address.
#[derive(Debug, Clone)]
pub enum RouterAddress {
    /// NTCP2.
    Ntcp2 {
        /// Router cost.
        cost: u8,

        /// Options.
        options: Mapping,

        /// NTCP2 static key.
        ///
        /// Must always be present, even for unpublished addresses.
        static_key: StaticPublicKey,

        /// NTCP2 IV.
        ///
        /// `None` if router hasn't published an address.
        iv: Option<[u8; 16]>,

        /// Router's socket address.
        ///
        /// `None` if router hasn't published an address.
        socket_address: Option<SocketAddr>,
    },

    /// SSU2.
    Ssu2 {
        /// Introducers.
        introducers: Vec<(RouterId, u32)>,

        /// Router cost.
        cost: u8,

        /// SSU2 static key.
        ///
        /// Must always be present, even for unpublished addresses.
        static_key: StaticPublicKey,

        /// SSU2 intro key.
        ///
        /// Must always be present, even for unpublished addresses.
        intro_key: [u8; 32],

        /// Options.
        options: Mapping,

        /// Router's socket address.
        ///
        /// `None` if router hasn't published an address.
        socket_address: Option<SocketAddr>,
    },
}

impl RouterAddress {
    /// Create new unpublished NTCP2 [`RouterAddress`].
    pub fn new_unpublished_ntcp2(key: [u8; 32], port: u16) -> Self {
        let static_key = StaticPrivateKey::from(key).public();
        let key = base64_encode(&static_key);

        let mut options = Mapping::default();
        options.insert("v".into(), "2".into());
        options.insert("s".into(), key.into());

        Self::Ntcp2 {
            cost: 14,
            options,
            static_key,
            iv: None,
            socket_address: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)),
        }
    }

    /// Create new unpublished NTCP2 [`RouterAddress`].
    pub fn new_published_ntcp2(key: [u8; 32], iv: [u8; 16], port: u16, host: Ipv4Addr) -> Self {
        let static_key = StaticPrivateKey::from(key).public();

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(base64_encode(&static_key)));
        options.insert(Str::from("host"), Str::from(host.to_string()));
        options.insert(Str::from("port"), Str::from(port.to_string()));
        options.insert(Str::from("i"), Str::from(base64_encode(iv)));

        Self::Ntcp2 {
            cost: 3,
            options,
            static_key,
            iv: Some(iv),
            socket_address: Some(SocketAddr::new(IpAddr::V4(host), port)),
        }
    }

    /// Create new unpublished SSU2 [`RouterAddress`].
    pub fn new_unpublished_ssu2(static_key: [u8; 32], intro_key: [u8; 32], port: u16) -> Self {
        let static_key = StaticPrivateKey::from(static_key).public();
        let encoded_static_key = base64_encode(&static_key);
        let encoded_intro_key = base64_encode(intro_key);

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(encoded_static_key));
        options.insert(Str::from("i"), Str::from(encoded_intro_key));
        options.insert(Str::from("caps"), Str::from("BC4"));

        Self::Ssu2 {
            cost: 14,
            intro_key,
            static_key,
            socket_address: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)),
            introducers: Vec::new(),
            options,
        }
    }

    /// Create new unpublished SSU2 [`RouterAddress`].
    pub fn new_published_ssu2(
        static_key: [u8; 32],
        intro_key: [u8; 32],
        port: u16,
        host: Ipv4Addr,
    ) -> Self {
        let static_key = StaticPrivateKey::from(static_key).public();
        let encoded_static_key = base64_encode(&static_key);
        let encoded_intro_key = base64_encode(intro_key);

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(encoded_static_key));
        options.insert(Str::from("i"), Str::from(encoded_intro_key));
        options.insert(Str::from("host"), Str::from(host.to_string()));
        options.insert(Str::from("port"), Str::from(port.to_string()));
        options.insert(Str::from("caps"), Str::from("BC4"));

        Self::Ssu2 {
            cost: 8,
            static_key,
            intro_key,
            options,
            introducers: Vec::new(),
            socket_address: Some(SocketAddr::new(IpAddr::V4(host), port)),
        }
    }

    /// Parse [`RouterAddress`] from `input`, returning rest of `input` and parsed address.
    pub fn parse_frame<R: Runtime>(
        input: &[u8],
    ) -> IResult<&[u8], RouterAddress, RouterAddressParseError> {
        let (rest, cost) = be_u8(input)?;
        let (rest, _expires) = Date::parse_frame(rest)
            .map_err(|_| Err::Error(RouterAddressParseError::InvalidExpiration))?;
        let (rest, transport) = Str::parse_frame(rest)
            .map_err(|_| Err::Error(RouterAddressParseError::InvalidTransport))?;
        let (rest, options) = Mapping::parse_frame(rest).map_err(Err::convert)?;

        // parse socket address
        let socket_address: Option<SocketAddr> = {
            let maybe_host = options.get(&Str::from("host"));
            let maybe_port = options.get(&Str::from("port"));

            match (maybe_host, maybe_port) {
                (Some(host), Some(port)) => {
                    let port = port.parse::<u16>().ok();
                    let host = host.parse::<IpAddr>().ok();

                    match (host, port) {
                        (Some(host), Some(port)) => Some(SocketAddr::new(host, port)),
                        (_, _) => None,
                    }
                }
                _ => None,
            }
        };

        match transport.as_ref() {
            "NTCP2" => {
                // static key must always be present
                let static_key = {
                    let static_key = options
                        .get(&Str::from("s"))
                        .ok_or(Err::Error(RouterAddressParseError::Ntcp2StaticKeyMissing))?;
                    let bytes = base64_decode(static_key.as_bytes())
                        .ok_or(Err::Error(RouterAddressParseError::InvalidNtcp2StaticKey))?;

                    StaticPublicKey::from_bytes(&bytes)
                        .ok_or(Err::Error(RouterAddressParseError::InvalidNtcp2StaticKey))?
                };
                let iv = options
                    .get(&Str::from("i"))
                    .and_then(|iv| base64_decode(iv.as_bytes()))
                    .and_then(|bytes| TryInto::<[u8; 16]>::try_into(bytes).ok());

                Ok((
                    rest,
                    Self::Ntcp2 {
                        cost,
                        options,
                        static_key,
                        iv,
                        socket_address,
                    },
                ))
            }
            "SSU2" => {
                // static key must always be present
                let static_key = {
                    let static_key = options
                        .get(&Str::from("s"))
                        .ok_or(Err::Error(RouterAddressParseError::Ssu2StaticKeyMissing))?;
                    let bytes = base64_decode(static_key.as_bytes())
                        .ok_or(Err::Error(RouterAddressParseError::InvalidSsu2StaticKey))?;

                    StaticPublicKey::from_bytes(&bytes)
                        .ok_or(Err::Error(RouterAddressParseError::InvalidSsu2StaticKey))?
                };
                // intro key must always be present
                let intro_key = {
                    let intro_key = options
                        .get(&Str::from("i"))
                        .ok_or(Err::Error(RouterAddressParseError::Ssu2IntroKeyMissing))?;
                    let bytes = base64_decode(intro_key.as_bytes())
                        .ok_or(Err::Error(RouterAddressParseError::InvalidSsu2IntroKey))?;

                    TryInto::<[u8; 32]>::try_into(bytes)
                        .map_err(|_| Err::Error(RouterAddressParseError::InvalidSsu2IntroKey))?
                };
                // introducers may be present if `socket_address` is not specified, from spec:
                //
                // "A router must not publish host or port in the address when publishing
                // introducers."
                let introducers = if socket_address.is_none() {
                    (0..MAX_INTRODUCERS)
                        .filter_map(|i| {
                            let expiration = options
                                .get(&Str::from(format!("iexp{i}")))
                                .and_then(|exp| exp.parse::<u32>().ok());
                            let router_id = options
                                .get(&Str::from(format!("ih{i}")))
                                .and_then(|hash| base64_decode(&**hash))
                                .map(RouterId::from);
                            let relay_tag = options
                                .get(&Str::from(format!("itag{i}")))
                                .and_then(|tag| tag.parse::<u32>().ok());

                            match (expiration, router_id, relay_tag) {
                                (Some(expiration), Some(router_id), Some(relay_tag)) => (expiration
                                    > R::time_since_epoch().as_secs() as u32)
                                    .then_some((router_id, relay_tag)),
                                _ => None,
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                Ok((
                    rest,
                    Self::Ssu2 {
                        cost,
                        introducers,
                        intro_key,
                        options,
                        socket_address,
                        static_key,
                    },
                ))
            }
            _ => Err(Err::Error(RouterAddressParseError::InvalidTransport)),
        }
    }

    /// Does the router support peer testing.
    pub fn supports_peer_testing(&self) -> bool {
        match self {
            Self::Ntcp2 { .. } => false,
            Self::Ssu2 { options, .. } =>
                options.iter().any(|(key, value)| &(**key) == "caps" && value.contains("B")),
        }
    }

    /// Does the router support relay.
    pub fn supports_relay(&self) -> bool {
        match self {
            Self::Ntcp2 { .. } => false,
            Self::Ssu2 { options, .. } =>
                options.iter().any(|(key, value)| &(**key) == "caps" && value.contains("C")),
        }
    }

    pub fn socket_address(&self) -> Option<SocketAddr> {
        match self {
            Self::Ntcp2 { socket_address, .. } => *socket_address,
            Self::Ssu2 { socket_address, .. } => *socket_address,
        }
    }

    /// Does the router support IPv4.
    pub fn supports_ipv4(&self) -> bool {
        self.socket_address().is_some_and(|address| address.is_ipv4())
    }

    /// Does the router support IPv6.
    pub fn supports_ipv6(&self) -> bool {
        self.socket_address().is_some_and(|address| address.is_ipv6())
    }

    /// Get reference to transport's options.
    pub fn options(&self) -> &Mapping {
        match self {
            Self::Ntcp2 { options, .. } => options,
            Self::Ssu2 { options, .. } => options,
        }
    }

    /// Try to convert `bytes` into a [`RouterAddress`].
    pub fn parse<R: Runtime>(
        bytes: impl AsRef<[u8]>,
    ) -> Result<RouterAddress, RouterAddressParseError> {
        Ok(Self::parse_frame::<R>(bytes.as_ref())?.1)
    }

    /// Serialize [`RouterAddress`].
    pub fn serialize(&self) -> BytesMut {
        match self {
            Self::Ntcp2 { cost, options, .. } => {
                let options = options.serialize();
                let transport = Str::from("NTCP2").serialize();
                let mut out = BytesMut::with_capacity(1 + 8 + transport.len() + options.len());

                out.put_u8(*cost);
                out.put_slice(&Date::new(0).serialize());
                out.put_slice(&transport);
                out.put_slice(&options);

                out
            }
            Self::Ssu2 { cost, options, .. } => {
                let options = options.serialize();
                let transport = Str::from("SSU2").serialize();
                let mut out = BytesMut::with_capacity(1 + 8 + transport.len() + options.len());

                out.put_u8(*cost);
                out.put_slice(&Date::new(0).serialize());
                out.put_slice(&transport);
                out.put_slice(&options);

                out
            }
        }
    }
}

#[cfg(test)]
impl RouterAddress {
    /// Get socket address of the SSU2 transport.
    pub fn ssu2_ipv4_address(&self) -> SocketAddr {
        match self {
            Self::Ssu2 { socket_address, .. } => socket_address.unwrap(),
            Self::Ntcp2 { .. } => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mock::MockRuntime;

    #[test]
    fn serialize_deserialize_unpublished_ntcp2() {
        let serialized = RouterAddress::new_unpublished_ntcp2([1u8; 32], 8888).serialize();
        let static_key = StaticPrivateKey::from([1u8; 32]).public();

        match RouterAddress::parse::<MockRuntime>(&serialized).unwrap() {
            RouterAddress::Ntcp2 {
                cost,
                options,
                static_key: s,
                iv,
                ..
            } => {
                assert_eq!(cost, 14);
                assert_eq!(s.to_vec(), static_key.to_vec());
                assert_eq!(iv, None);

                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert!(options.get(&Str::from("i")).is_none());
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
            }
            _ => panic!("invalid ntcp2 address"),
        }
    }

    #[test]
    fn serialize_deserialize_published_ntcp2() {
        let serialized = RouterAddress::new_published_ntcp2(
            [1u8; 32],
            [0xaa; 16],
            8888,
            "127.0.0.1".parse().unwrap(),
        )
        .serialize();
        let static_key = StaticPrivateKey::from([1u8; 32]).public();

        match RouterAddress::parse::<MockRuntime>(&serialized).unwrap() {
            RouterAddress::Ntcp2 {
                cost,
                options,
                static_key: s,
                iv,
                socket_address,
            } => {
                assert_eq!(cost, 3);
                assert_eq!(s.to_vec(), static_key.to_vec());
                assert_eq!(iv, Some([0xaa; 16]));
                assert_eq!(socket_address, Some("127.0.0.1:8888".parse().unwrap()));
                assert_eq!(
                    options.get(&Str::from("i")),
                    Some(&Str::from(base64_encode(&[0xaa; 16])))
                );
                assert_eq!(
                    options.get(&Str::from("s")),
                    Some(&Str::from(base64_encode(&static_key)))
                );
                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert_eq!(
                    options.get(&Str::from("host")),
                    Some(&Str::from("127.0.0.1"))
                );
                assert_eq!(options.get(&Str::from("port")), Some(&Str::from("8888")));
            }
            _ => panic!("invalid ntp2 address"),
        }
    }

    #[test]
    fn serialize_deserialize_unpublished_ssu2() {
        let serialized =
            RouterAddress::new_unpublished_ssu2([1u8; 32], [2u8; 32], 8888).serialize();
        let static_key = StaticPrivateKey::from([1u8; 32]).public();
        let intro_key = [2u8; 32];

        match RouterAddress::parse::<MockRuntime>(&serialized).unwrap() {
            RouterAddress::Ssu2 {
                cost,
                options,
                static_key: s,
                intro_key: i,
                ..
            } => {
                assert_eq!(cost, 14);
                assert_eq!(s.to_vec(), static_key.to_vec());
                assert_eq!(i, intro_key);
                assert_eq!(
                    options.get(&Str::from("s")),
                    Some(&Str::from(base64_encode(&static_key)))
                );
                assert_eq!(
                    options.get(&Str::from("i")),
                    Some(&Str::from(base64_encode(&intro_key)))
                );
                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert!(options.get(&Str::from("host")).is_none());
                assert!(options.get(&Str::from("port")).is_none());
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn serialize_deserialize_published_ssu2() {
        let serialized = RouterAddress::new_published_ssu2(
            [1u8; 32],
            [2u8; 32],
            8888,
            "127.0.0.1".parse().unwrap(),
        )
        .serialize();
        let static_key = StaticPrivateKey::from([1u8; 32]).public();
        let intro_key = [2u8; 32];

        match RouterAddress::parse::<MockRuntime>(&serialized).unwrap() {
            RouterAddress::Ssu2 {
                cost,
                static_key: s,
                intro_key: i,
                options,
                socket_address,
                ..
            } => {
                assert_eq!(cost, 8);
                assert_eq!(s.to_vec(), static_key.to_vec());
                assert_eq!(i, intro_key);
                assert_eq!(socket_address, Some("127.0.0.1:8888".parse().unwrap()));
                assert_eq!(
                    options.get(&Str::from("s")),
                    Some(&Str::from(base64_encode(&static_key)))
                );
                assert_eq!(
                    options.get(&Str::from("i")),
                    Some(&Str::from(base64_encode(&intro_key)))
                );
                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert_eq!(
                    options.get(&Str::from("host")),
                    Some(&Str::from("127.0.0.1"))
                );
                assert_eq!(options.get(&Str::from("port")), Some(&Str::from("8888")));
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn ntcp2_static_key_missing() {
        let mut address = RouterAddress::new_unpublished_ntcp2([0xaa; 32], 8888);
        match address {
            RouterAddress::Ntcp2 {
                ref mut options, ..
            } => {
                let _ = options.remove(&Str::from("s"));
            }
            _ => panic!("invalid ntcp2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::Ntcp2StaticKeyMissing
        );
    }

    #[test]
    fn invalid_ntcp2_static_key() {
        let mut address = RouterAddress::new_unpublished_ntcp2([0xaa; 32], 8888);
        match address {
            RouterAddress::Ntcp2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("s"), Str::from("hello, world"));
            }
            _ => panic!("invalid ntcp2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::InvalidNtcp2StaticKey
        );
    }

    #[test]
    fn ssu2_static_key_missing() {
        let mut address = RouterAddress::new_unpublished_ssu2([0xaa; 32], [0xbb; 32], 8888);
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                let _ = options.remove(&Str::from("s"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::Ssu2StaticKeyMissing
        );
    }

    #[test]
    fn ssu2_intro_key_missing() {
        let mut address = RouterAddress::new_unpublished_ssu2([0xaa; 32], [0xbb; 32], 8888);
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                let _ = options.remove(&Str::from("i"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::Ssu2IntroKeyMissing
        );
    }

    #[test]
    fn invalid_ssu2_static_key() {
        let mut address = RouterAddress::new_unpublished_ssu2([0xaa; 32], [0xbb; 32], 8888);
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("s"), Str::from("hello, world"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::InvalidSsu2StaticKey
        );
    }

    #[test]
    fn invalid_ssu2_intro_key() {
        let mut address = RouterAddress::new_unpublished_ssu2([0xaa; 32], [0xbb; 32], 8888);
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("i"), Str::from("hello, world"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::InvalidSsu2IntroKey
        );
    }

    #[test]
    fn introducers_parsed() {
        let mut address = RouterAddress::new_unpublished_ssu2([0xaa; 32], [0xbb; 32], 8888);
        let router_id = RouterId::random();

        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(
                    Str::from("iexp0"),
                    Str::from((MockRuntime::time_since_epoch().as_secs() + 10).to_string()),
                );
                options.insert(
                    Str::from("ih0"),
                    Str::from(base64_encode(router_id.to_vec())),
                );
                options.insert(Str::from("itag0"), Str::from("1337"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        match RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                assert_eq!(introducers.len(), 1);
                assert_eq!(introducers[0].0, router_id);
                assert_eq!(introducers[0].1, 1337);
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn introducers_ignored_for_published_address() {
        let mut address = RouterAddress::new_published_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            8888,
            "127.0.0.1".parse().unwrap(),
        );
        let router_id = RouterId::random();

        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("iepx0"), Str::from("1337"));
                options.insert(
                    Str::from("ih0"),
                    Str::from(base64_encode(router_id.to_vec())),
                );
                options.insert(Str::from("itag0"), Str::from("1337"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        match RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                assert!(introducers.is_empty());
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn stale_introducers_ignored() {}
}
