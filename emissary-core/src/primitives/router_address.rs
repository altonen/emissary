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
    constants,
    crypto::{base64_decode, base64_encode, StaticPrivateKey, StaticPublicKey},
    error::parser::RouterAddressParseError,
    primitives::{Date, Mapping, RouterId, Str},
    runtime::Runtime,
};

use bytes::{BufMut, BytesMut};
use nom::{number::complete::be_u8, Err, IResult};

use alloc::{format, string::ToString, vec::Vec};
use core::{
    fmt,
    net::{IpAddr, SocketAddr},
};

/// Maximum amount of introducers.
const MAX_INTRODUCERS: usize = 3usize;

/// Transport kind.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TransportKind {
    /// NTCP2 over IPv4.
    Ntcp2V4,

    /// NTCP2 over IPv4.
    Ntcp2V6,

    /// SSU2 over IPv4.
    Ssu2V4,

    /// SSU2 over IPv6.
    Ssu2V6,
}

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

        /// MTU.
        mtu: usize,

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

impl fmt::Display for RouterAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ntcp2 { socket_address, .. } => write!(f, "ntcp2({socket_address:?})"),
            Self::Ssu2 { socket_address, .. } => write!(f, "ssu2({socket_address:?})"),
        }
    }
}

impl RouterAddress {
    /// Create new unpublished NTCP2 [`RouterAddress`].
    pub fn new_unpublished_ntcp2(key: [u8; 32], address: SocketAddr) -> Self {
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
            socket_address: Some(address),
        }
    }

    /// Create new unpublished NTCP2 [`RouterAddress`].
    ///
    /// `address` is the address the NTCP2 listener is bound to.
    /// `host` is the external address other routers should use.
    pub fn new_published_ntcp2(
        key: [u8; 32],
        iv: [u8; 16],
        host: IpAddr,
        address: SocketAddr,
    ) -> Self {
        let static_key = StaticPrivateKey::from(key).public();

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(base64_encode(&static_key)));
        options.insert(Str::from("host"), Str::from(host.to_string()));
        options.insert(Str::from("port"), Str::from(address.port().to_string()));
        options.insert(Str::from("i"), Str::from(base64_encode(iv)));

        Self::Ntcp2 {
            cost: 3,
            options,
            static_key,
            iv: Some(iv),
            socket_address: Some(address),
        }
    }

    /// Create new unpublished SSU2 [`RouterAddress`].
    pub fn new_unpublished_ssu2(
        static_key: [u8; 32],
        intro_key: [u8; 32],
        address: SocketAddr,
        mtu: usize,
    ) -> Self {
        let static_key = StaticPrivateKey::from(static_key).public();
        let encoded_static_key = base64_encode(&static_key);
        let encoded_intro_key = base64_encode(intro_key);

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(encoded_static_key));
        options.insert(Str::from("i"), Str::from(encoded_intro_key));

        if address.is_ipv4() {
            options.insert(Str::from("caps"), Str::from("4"));
        } else {
            options.insert(Str::from("caps"), Str::from("6"));
        }

        if mtu != constants::ssu2::MAX_MTU {
            options.insert(Str::from("mtu"), Str::from(mtu.to_string()));
        }

        Self::Ssu2 {
            cost: 14,
            intro_key,
            mtu,
            static_key,
            socket_address: Some(address),
            introducers: Vec::new(),
            options,
        }
    }

    /// Create new unpublished SSU2 [`RouterAddress`].
    ///
    /// `address` is the address the NTCP2 listener is bound to.
    /// `host` is the external address other routers should use.
    pub fn new_published_ssu2(
        static_key: [u8; 32],
        intro_key: [u8; 32],
        host: IpAddr,
        address: SocketAddr,
        mtu: usize,
    ) -> Self {
        let static_key = StaticPrivateKey::from(static_key).public();
        let encoded_static_key = base64_encode(&static_key);
        let encoded_intro_key = base64_encode(intro_key);

        let mut options = Mapping::default();
        options.insert(Str::from("v"), Str::from("2"));
        options.insert(Str::from("s"), Str::from(encoded_static_key));
        options.insert(Str::from("i"), Str::from(encoded_intro_key));
        options.insert(Str::from("host"), Str::from(host.to_string()));
        options.insert(Str::from("port"), Str::from(address.port().to_string()));

        if mtu != constants::ssu2::MAX_MTU {
            options.insert(Str::from("mtu"), Str::from(mtu.to_string()));
        }

        if address.is_ipv4() {
            options.insert(Str::from("caps"), Str::from("BC4"));
        } else {
            options.insert(Str::from("caps"), Str::from("BC6"));
        }

        Self::Ssu2 {
            cost: 8,
            static_key,
            intro_key,
            mtu,
            options,
            introducers: Vec::new(),
            socket_address: Some(address),
        }
    }

    /// Convert NTCP2 address into a reachable address.
    ///
    /// `host` is the external address the local router is reachable from.
    /// `port` is the port where the router the listener is bound to or the NAT-mapped
    /// port learned during peer testing.
    pub fn into_reachable_ntcp2(&mut self, iv: [u8; 16], port: u16, host: IpAddr) {
        match self {
            Self::Ssu2 { .. } => unreachable!(),
            Self::Ntcp2 {
                cost,
                options,
                iv: local_iv,
                ..
            } => {
                options.insert(Str::from("host"), Str::from(host.to_string()));
                options.insert(Str::from("port"), Str::from(port.to_string()));
                options.insert(Str::from("i"), Str::from(base64_encode(iv)));

                *local_iv = Some(iv);
                *cost = 3;
            }
        }
    }

    /// Convert SSU2 address into a reachable address.
    ///
    /// `host` is the external address the local router is reachable from.
    /// `port` is the port where the router the listener is bound to or the NAT-mapped
    /// port learned during peer testing.
    pub fn into_reachable_ssu2(&mut self, port: u16, host: IpAddr) {
        match self {
            Self::Ntcp2 { .. } => unreachable!(),
            Self::Ssu2 {
                introducers,
                cost,
                options,
                ..
            } => {
                options.insert(Str::from("host"), Str::from(host.to_string()));
                options.insert(Str::from("port"), Str::from(port.to_string()));

                introducers.clear();
                options.retain(|key, _| {
                    !(key.starts_with("iexp") || key.starts_with("itag") || key.starts_with("ih"))
                });

                *cost = 8
            }
        }
    }

    /// Parse [`RouterAddress`] from `input`, returning rest of `input` and parsed address.
    ///
    /// Returns `Ok((remaining_bytes, RouterAddress))` on success.
    ///
    /// Returns `Err((Option<remaining_bytes>, RouterAddressParseError))` on errror.
    /// If `remaining_bytes` is `Some`, it means the address was not a supported NTCP2/SSU2 address
    /// but still a valid address. Caller can then use `remaining_bytes` to parse the rest of the
    /// addresses/rest of `RouterInfo` normally. If `remaining_bytes` is `None`, the address format
    /// itself was invalid and caller should reject the `RouterInfo`.
    pub fn parse_frame<R: Runtime>(
        input: &[u8],
    ) -> IResult<&[u8], RouterAddress, (Option<&[u8]>, RouterAddressParseError)> {
        let (rest, cost) = be_u8::<_, ()>(input)
            .map_err(|_| Err::Error((None, RouterAddressParseError::InvalidTransport)))?;
        let (rest, _expires) = Date::parse_frame(rest)
            .map_err(|_| Err::Error((None, RouterAddressParseError::InvalidExpiration)))?;
        let (rest, transport) = Str::parse_frame(rest)
            .map_err(|_| Err::Error((None, RouterAddressParseError::InvalidTransport)))?;
        let (rest, options) = Mapping::parse_frame(rest)
            .map_err(|_| Err::Error((None, RouterAddressParseError::InvalidBitstream)))?;

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
                    let static_key = options.get(&Str::from("s")).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::Ntcp2StaticKeyMissing,
                    )))?;
                    let bytes = base64_decode(static_key.as_bytes()).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::InvalidNtcp2StaticKey,
                    )))?;

                    StaticPublicKey::from_bytes(&bytes).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::InvalidNtcp2StaticKey,
                    )))?
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
            "SSU2" | "SSU" => {
                // static key must always be present
                let static_key = {
                    let static_key = options.get(&Str::from("s")).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::Ssu2StaticKeyMissing,
                    )))?;
                    let bytes = base64_decode(static_key.as_bytes()).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::InvalidSsu2StaticKey,
                    )))?;

                    StaticPublicKey::from_bytes(&bytes).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::InvalidSsu2StaticKey,
                    )))?
                };
                // intro key must always be present
                let intro_key = {
                    let intro_key = options.get(&Str::from("i")).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::Ssu2IntroKeyMissing,
                    )))?;
                    let bytes = base64_decode(intro_key.as_bytes()).ok_or(Err::Error((
                        Some(rest),
                        RouterAddressParseError::InvalidSsu2IntroKey,
                    )))?;

                    TryInto::<[u8; 32]>::try_into(bytes).map_err(|_| {
                        Err::Error((Some(rest), RouterAddressParseError::InvalidSsu2IntroKey))
                    })?
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

                // get the MTU of remote router
                //
                // address is discarded if the mtu is invalid
                let mtu = match options.get(&Str::from("mtu")) {
                    None => constants::ssu2::MAX_MTU,
                    Some(value) => value
                        .parse::<usize>()
                        .ok()
                        .and_then(|mtu| (mtu >= constants::ssu2::MIN_MTU).then_some(mtu))
                        .ok_or(Err::Error((
                            Some(rest),
                            RouterAddressParseError::InvalidMtu,
                        )))?,
                };

                Ok((
                    rest,
                    Self::Ssu2 {
                        cost,
                        introducers,
                        intro_key,
                        mtu: mtu.min(constants::ssu2::MAX_MTU),
                        options,
                        socket_address,
                        static_key,
                    },
                ))
            }
            _ => Err(Err::Error((
                Some(rest),
                RouterAddressParseError::InvalidTransport,
            ))),
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

    /// Get cost of the transport.
    pub fn cost(&self) -> u8 {
        match self {
            Self::Ntcp2 { cost, .. } => *cost,
            Self::Ssu2 { cost, .. } => *cost,
        }
    }

    /// Attempt to classify `RouterAddress` into a `TransportKind`.
    ///
    /// For NTCP2, only the published socket address is used and if no address is published,
    /// return `None` to indicate that the address could not be classified.
    ///
    /// For SSU2, socket address is used for classification first and if it doesn't exist,
    /// `caps` are checked for `4`/`6` flags, with IPv4 preferred.
    pub fn classify(&self) -> Option<TransportKind> {
        match self {
            Self::Ntcp2 { socket_address, .. } =>
                socket_address.map(|address| match address.ip() {
                    IpAddr::V4(_) => TransportKind::Ntcp2V4,
                    IpAddr::V6(_) => TransportKind::Ntcp2V6,
                }),
            Self::Ssu2 {
                socket_address,
                options,
                ..
            } => match socket_address {
                Some(address) => match address {
                    SocketAddr::V4(_) => Some(TransportKind::Ssu2V4),
                    SocketAddr::V6(_) => Some(TransportKind::Ssu2V6),
                },
                None => {
                    let caps = options.get(&Str::from("caps"))?;

                    if caps.contains("4") {
                        return Some(TransportKind::Ssu2V4);
                    }

                    caps.contains("6").then_some(TransportKind::Ssu2V6)
                }
            },
        }
    }

    /// Try to convert `bytes` into a [`RouterAddress`].
    pub fn parse<R: Runtime>(
        bytes: impl AsRef<[u8]>,
    ) -> Result<RouterAddress, RouterAddressParseError> {
        match Self::parse_frame::<R>(bytes.as_ref()) {
            Ok((_, address)) => Ok(address),
            Err(error) => Err(error.map(|err| err.1).into()),
        }
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
    use std::{
        net::{Ipv4Addr, Ipv6Addr},
        time::Duration,
    };

    #[test]
    fn serialize_deserialize_unpublished_ntcp2() {
        let serialized = RouterAddress::new_unpublished_ntcp2(
            [1u8; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
        )
        .serialize();
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
            "127.0.0.1".parse().unwrap(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
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
    fn serialize_deserialize_published_ntcp2_ipv6() {
        let serialized = RouterAddress::new_published_ntcp2(
            [1u8; 32],
            [0xaa; 16],
            "::1".parse().unwrap(),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8888),
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
                assert_eq!(socket_address, Some("[::1]:8888".parse().unwrap()));
                assert_eq!(
                    options.get(&Str::from("i")),
                    Some(&Str::from(base64_encode(&[0xaa; 16])))
                );
                assert_eq!(
                    options.get(&Str::from("s")),
                    Some(&Str::from(base64_encode(&static_key)))
                );
                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("::1")));
                assert_eq!(options.get(&Str::from("port")), Some(&Str::from("8888")));
            }
            _ => panic!("invalid ntp2 address"),
        }
    }

    #[test]
    fn serialize_deserialize_unpublished_ssu2() {
        let serialized = RouterAddress::new_unpublished_ssu2(
            [1u8; 32],
            [2u8; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        )
        .serialize();
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
            "127.0.0.1".parse().unwrap(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
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
    fn serialize_deserialize_published_ssu2_ipv6() {
        let serialized = RouterAddress::new_published_ssu2(
            [1u8; 32],
            [2u8; 32],
            "::1".parse().unwrap(),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 8888),
            1500,
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
                assert_eq!(socket_address, Some("[::1]:8888".parse().unwrap()));
                assert_eq!(
                    options.get(&Str::from("s")),
                    Some(&Str::from(base64_encode(&static_key)))
                );
                assert_eq!(
                    options.get(&Str::from("i")),
                    Some(&Str::from(base64_encode(&intro_key)))
                );
                assert_eq!(options.get(&Str::from("v")), Some(&Str::from("2")));
                assert_eq!(options.get(&Str::from("host")), Some(&Str::from("::1")));
                assert_eq!(options.get(&Str::from("port")), Some(&Str::from("8888")));
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn ntcp2_static_key_missing() {
        let mut address = RouterAddress::new_unpublished_ntcp2(
            [0xaa; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
        );
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
        let mut address = RouterAddress::new_unpublished_ntcp2(
            [0xaa; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
        );
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
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
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
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
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
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
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
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
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
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
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
            "127.0.0.1".parse().unwrap(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
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
    fn stale_introducers_ignored() {
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
        let router_id1 = RouterId::random();
        let router_id2 = RouterId::random();

        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("iexp0"), Str::from("1337"));
                options.insert(
                    Str::from("ih0"),
                    Str::from(base64_encode(router_id1.to_vec())),
                );
                options.insert(Str::from("itag0"), Str::from("1337"));

                options.insert(
                    Str::from("iexp1"),
                    Str::from(
                        (MockRuntime::time_since_epoch() + Duration::from_secs(60))
                            .as_secs()
                            .to_string(),
                    ),
                );
                options.insert(
                    Str::from("ih1"),
                    Str::from(base64_encode(router_id2.to_vec())),
                );
                options.insert(Str::from("itag1"), Str::from("1338"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        match RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                assert_eq!(introducers.len(), 1);
                assert_eq!(introducers[0].0, router_id2);
                assert_eq!(introducers[0].1, 1338);
            }
            _ => panic!("invalid ssu2 address"),
        }
    }

    #[test]
    fn classify_router_address() {
        // published ntcp2 over ipv4
        assert_eq!(
            RouterAddress::Ntcp2 {
                cost: 8,
                options: Mapping::default(),
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                iv: Some([0xaa; 16]),
                socket_address: Some("127.0.0.1:8888".parse().unwrap()),
            }
            .classify(),
            Some(TransportKind::Ntcp2V4)
        );

        // published ntcp2 over ipv6
        assert_eq!(
            RouterAddress::Ntcp2 {
                cost: 8,
                options: Mapping::default(),
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                iv: Some([0xaa; 16]),
                socket_address: Some("[::]:8888".parse().unwrap()),
            }
            .classify(),
            Some(TransportKind::Ntcp2V6)
        );

        // unpublished ntcp2
        assert_eq!(
            RouterAddress::Ntcp2 {
                cost: 8,
                options: Mapping::default(),
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                iv: None,
                socket_address: None,
            }
            .classify(),
            None,
        );

        // ssu2 over ipv4
        assert_eq!(
            RouterAddress::Ssu2 {
                introducers: Vec::new(),
                cost: 8,
                mtu: 1500,
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                intro_key: [0xaa; 32],
                options: Mapping::default(),
                socket_address: Some("127.0.0.1:8888".parse().unwrap())
            }
            .classify(),
            Some(TransportKind::Ssu2V4)
        );

        // ssu2 over ipv6
        assert_eq!(
            RouterAddress::Ssu2 {
                introducers: Vec::new(),
                cost: 8,
                mtu: 1500,
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                intro_key: [0xaa; 32],
                options: Mapping::default(),
                socket_address: Some("[::]:8888".parse().unwrap())
            }
            .classify(),
            Some(TransportKind::Ssu2V6)
        );

        // unpublished ssu2 with `4` caps
        assert_eq!(
            RouterAddress::Ssu2 {
                introducers: Vec::new(),
                cost: 8,
                mtu: 1500,
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                intro_key: [0xaa; 32],
                options: Mapping::from_iter([(Str::from("caps"), Str::from("4"))]),
                socket_address: None,
            }
            .classify(),
            Some(TransportKind::Ssu2V4)
        );

        // unpublished ssu2 with `6` caps
        assert_eq!(
            RouterAddress::Ssu2 {
                introducers: Vec::new(),
                cost: 8,
                mtu: 1500,
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                intro_key: [0xaa; 32],
                options: Mapping::from_iter([(Str::from("caps"), Str::from("6"))]),
                socket_address: None,
            }
            .classify(),
            Some(TransportKind::Ssu2V6)
        );

        // unpublished ssu2 without caps
        assert_eq!(
            RouterAddress::Ssu2 {
                introducers: Vec::new(),
                cost: 8,
                mtu: 1500,
                static_key: StaticPrivateKey::random(&mut MockRuntime::rng()).public(),
                intro_key: [0xaa; 32],
                options: Mapping::default(),
                socket_address: None,
            }
            .classify(),
            None,
        );
    }

    #[test]
    fn standard_mtu_not_published() {
        // published
        {
            let address = RouterAddress::new_published_ssu2(
                [0xaa; 32],
                [0xbb; 32],
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                1500,
            );

            match address {
                RouterAddress::Ssu2 { options, .. } => {
                    assert!(options.get(&Str::from("mtu")).is_none());
                }
                _ => panic!("invalid ssu2 address"),
            }
        }

        // unpublished
        {
            let address = RouterAddress::new_unpublished_ssu2(
                [0xaa; 32],
                [0xbb; 32],
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                1500,
            );

            match address {
                RouterAddress::Ssu2 { options, .. } => {
                    assert!(options.get(&Str::from("mtu")).is_none());
                }
                _ => panic!("invalid ssu2 address"),
            }
        }
    }

    #[test]
    fn non_standard_mtu_published() {
        // published
        {
            let address = RouterAddress::new_published_ssu2(
                [0xaa; 32],
                [0xbb; 32],
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                1300,
            );

            match address {
                RouterAddress::Ssu2 { options, .. } => {
                    assert_eq!(options.get(&Str::from("mtu")), Some(&Str::from("1300")));
                }
                _ => panic!("invalid ssu2 address"),
            }
        }

        // unpublished
        {
            let address = RouterAddress::new_unpublished_ssu2(
                [0xaa; 32],
                [0xbb; 32],
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                1300,
            );

            match address {
                RouterAddress::Ssu2 { options, .. } => {
                    assert_eq!(options.get(&Str::from("mtu")), Some(&Str::from("1300")));
                }
                _ => panic!("invalid ssu2 address"),
            }
        }
    }

    #[test]
    fn ssu2_invalid_mtu() {
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("mtu"), Str::from("1024"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        assert_eq!(
            RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap_err(),
            RouterAddressParseError::InvalidMtu
        );
    }

    #[test]
    fn ssu2_mtu_clamped() {
        let mut address = RouterAddress::new_unpublished_ssu2(
            [0xaa; 32],
            [0xbb; 32],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
            1500,
        );
        match address {
            RouterAddress::Ssu2 {
                ref mut options, ..
            } => {
                options.insert(Str::from("mtu"), Str::from("5555"));
            }
            _ => panic!("invalid ssu2 address"),
        }

        match RouterAddress::parse::<MockRuntime>(&address.serialize()).unwrap() {
            RouterAddress::Ssu2 { mtu, .. } => assert_eq!(mtu, constants::ssu2::MAX_MTU),
            _ => panic!("invalid address"),
        }
    }
}
