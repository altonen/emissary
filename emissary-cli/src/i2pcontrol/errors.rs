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

use std::fmt;

/// I2PControl-specific errors.
#[derive(Debug)]
pub enum I2pControlError {
    /// Configuration error (missing credentials, invalid bind, etc.)
    Config(String),

    /// TLS material error (missing, unreadable, mismatched, or insecure).
    Tls(String),

    /// Listener bind error.
    Bind(String),

    /// Token store full or unavailable.
    TokenStore,

    /// Internal server error (sanitized).
    Internal(String),
}

impl fmt::Display for I2pControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "I2PControl configuration error: {msg}"),
            Self::Tls(msg) => write!(f, "I2PControl TLS error: {msg}"),
            Self::Bind(msg) => write!(f, "I2PControl bind error: {msg}"),
            Self::TokenStore => write!(f, "I2PControl token store error"),
            Self::Internal(_) => write!(f, "I2PControl internal error"),
        }
    }
}

impl std::error::Error for I2pControlError {}
