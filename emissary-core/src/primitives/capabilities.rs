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

use crate::primitives::Str;

use core::fmt;

/// Specified bandwidth of the router.
#[derive(Default, Debug, Clone, Copy)]
pub enum Bandwidth {
    /// Under 12 KBps shared bandwidth.
    K,

    /// 12 - 48 KBps shared bandwidth (default).
    #[default]
    L,

    /// 48 - 64 KBps shared bandwidth.
    M,

    /// 64 - 128 KBps shared bandwidth.
    N,

    /// 128 - 256 KBps shared bandwidth.
    O,

    /// 256 - 2000 KBps shared bandwidth (as of release 0.9.20).
    P,

    /// Over 2000 KBps shared bandwidth (as of release 0.9.20).
    X,
}

impl Bandwidth {
    /// Attempt to parse [`Bandwidth`] from `caps`.
    pub fn parse(caps: &Str) -> Option<Self> {
        if caps.contains("K") {
            return Some(Self::K);
        }

        if caps.contains("L") {
            return Some(Self::L);
        }

        if caps.contains("M") {
            return Some(Self::M);
        }

        if caps.contains("N") {
            return Some(Self::N);
        }

        if caps.contains("O") {
            return Some(Self::O);
        }

        if caps.contains("P") {
            return Some(Self::P);
        }

        if caps.contains("X") {
            return Some(Self::X);
        }

        None
    }
}

impl From<usize> for Bandwidth {
    fn from(value: usize) -> Self {
        match value {
            0..12_000 => Self::K,
            12_000..48_000 => Self::L,
            48_000..64_000 => Self::M,
            64_000..128_000 => Self::N,
            128_000..256_000 => Self::O,
            256_000..2_000_000 => Self::P,
            _ => Self::X,
        }
    }
}

impl From<Bandwidth> for Str {
    fn from(value: Bandwidth) -> Self {
        match value {
            Bandwidth::K => Str::from("K"),
            Bandwidth::L => Str::from("L"),
            Bandwidth::M => Str::from("M"),
            Bandwidth::N => Str::from("N"),
            Bandwidth::O => Str::from("O"),
            Bandwidth::P => Str::from("P"),
            Bandwidth::X => Str::from("X"),
        }
    }
}

/// Router capabilities
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Serialized capabilities.
    capabilities: Str,

    /// Is the router a floodfill router.
    floodfill: bool,

    /// Bandwidth.
    bandwidth: Option<Bandwidth>,

    /// Is the router reachable.
    reachable: bool,

    /// Is the router usable.
    ///
    /// Router is not usable if it has high congestion is rejecting all tunnels.
    usable: bool,
}

impl fmt::Display for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.capabilities)
    }
}

impl Capabilities {
    /// Attempt to parse [`Capabilities`] from `caps`.
    pub fn parse(caps: &Str) -> Option<Self> {
        let bandwidth = Bandwidth::parse(caps);
        let floodfill = caps.contains("f");
        let usable = !(caps.contains("E") || caps.contains("G"));
        let reachable = !(caps.contains("U") || caps.contains("H"));

        Some(Self {
            capabilities: caps.clone(),
            floodfill,
            bandwidth,
            usable,
            reachable,
        })
    }

    /// Returns `true` if the router is a floodfill router.
    pub fn is_floodfill(&self) -> bool {
        self.floodfill
    }

    /// Is the router considered fast.
    ///
    /// Router is considered fast if it's reachable and its capabilities specify either O, P or X.
    pub fn is_fast(&self) -> bool {
        self.bandwidth.is_some_and(|bandwidth| {
            core::matches!(bandwidth, Bandwidth::O | Bandwidth::P | Bandwidth::X)
        })
    }

    /// Is the router considered to have "standard" bandwidth.
    pub fn is_standard(&self) -> bool {
        self.bandwidth.is_some_and(|bandwidth| {
            core::matches!(
                bandwidth,
                Bandwidth::K | Bandwidth::L | Bandwidth::M | Bandwidth::N
            )
        })
    }

    /// Is the router considered reachable.
    pub fn is_reachable(&self) -> bool {
        self.reachable
    }

    /// Is the router considered usable.
    pub fn is_usable(&self) -> bool {
        self.usable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_bandwidth() {
        for cap in ["K", "L", "M", "N"] {
            assert!(Capabilities::parse(&Str::from(cap)).unwrap().is_standard());
        }
    }

    #[test]
    fn high_bandwidth() {
        for cap in ["O", "P", "X"] {
            assert!(Capabilities::parse(&Str::from(cap)).unwrap().is_fast());
        }
    }

    #[test]
    fn unrecognized_bandwidth() {
        let caps = Capabilities::parse(&Str::from("Z")).unwrap();
        assert!(caps.bandwidth.is_none());
        assert!(!caps.floodfill);
    }

    #[test]
    fn floodfill() {
        let caps = Capabilities::parse(&Str::from("Xf")).unwrap();

        assert!(caps.is_floodfill());
        assert!(caps.is_fast());
    }

    #[test]
    fn usable() {
        assert!(!Capabilities::parse(&Str::from("LG")).unwrap().is_usable());
        assert!(!Capabilities::parse(&Str::from("LE")).unwrap().is_usable());
    }

    #[test]
    fn reachable() {
        assert!(!Capabilities::parse(&Str::from("HX")).unwrap().is_reachable());
        assert!(!Capabilities::parse(&Str::from("UL")).unwrap().is_reachable());
    }
}
