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

pub struct RelayHandle {}

impl RelayHandle {}

/// Relay tag request result.
///
/// Used by `InboundSsu2Session` to inform `RelayManager` whether the inbound router requested
/// a relay tag.
#[derive(Debug, Copy, Clone)]
pub enum RelayTagRequested {
    /// Remote router requested relay from us.
    Yes(u32),

    /// Remote router did not requested relay from us.
    No(u32),
}

impl RelayTagRequested {
    /// Get relay tay.
    pub fn tag(&self) -> u32 {
        match self {
            Self::Yes(tag) => *tag,
            Self::No(tag) => *tag,
        }
    }
}
