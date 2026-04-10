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

/// Crypto constants.
pub mod crypto {
    /// Poly1305 MAC size.
    pub const POLY1305_MAC_SIZE: usize = 16usize;

    /// ML-KEM constants.
    pub mod ml_kem {
        /// Minimum MTU size for ML-KEM-768 over IPv4.
        ///
        /// <https://i2p.net/en/docs/specs/ssu2-hybrid>
        pub const ML_KEM_768_IPV4_MIN_MTU: usize = 1318usize;

        /// Minimum MTU size for ML-KEM-768 over IPv6.
        ///
        /// <https://i2p.net/en/docs/specs/ssu2-hybrid>
        pub const ML_KEM_768_IPV6_MIN_MTU: usize = 1338usize;
    }
}

/// SSU2 constants.
pub mod ssu2 {
    /// Minimum MTU size for SSU2.
    ///
    /// <https://i2p.net/en/docs/specs/ssu2/#ssu2-address>
    pub const MIN_MTU: usize = 1280usize;

    /// Maximum MTU size for SSU2.
    ///
    /// <https://i2p.net/en/docs/specs/ssu2/#ssu2-address>
    pub const MAX_MTU: usize = 1500usize;

    /// IPv4 overhead.
    ///
    /// IPv4 header (20 bytes) + UDP header (8 bytes).
    pub const IPV4_OVERHEAD: usize = 28usize;

    /// IPv6 overhead.
    ///
    /// IPv6 header (40 bytes) + UDP header (8 bytes).
    pub const IPV6_OVERHEAD: usize = 48usize;
}
