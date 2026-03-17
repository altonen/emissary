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
    runtime::{Runtime, TcpListener},
    transport::ntcp2::LOG_TARGET,
    util::is_global,
};

use futures::Stream;

use core::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

/// NTCP2 listener.
pub struct Ntcp2Listener<R: Runtime> {
    /// Allow local addresses.
    allow_local: bool,

    /// TCP Listener.
    listener: R::TcpListener,
}

impl<R: Runtime> Ntcp2Listener<R> {
    /// Create new [`Ntcp2Listener`] from a TCP listener.
    pub fn new(listener: R::TcpListener, allow_local: bool) -> Self {
        Self {
            allow_local,
            listener,
        }
    }

    /// Get local address of the TCP listener.
    #[cfg(test)]
    pub fn local_address(&self) -> SocketAddr {
        self.listener.local_address().expect("to succeed")
    }
}

impl<R: Runtime> Stream for Ntcp2Listener<R> {
    type Item = (R::TcpStream, SocketAddr);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.listener.poll_accept(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some((stream, address))) => match address {
                    SocketAddr::V4(address) if !is_global(*address.ip()) && !self.allow_local => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?address,
                            "incoming connection from local address but local addresses were disabled",
                        );
                        continue;
                    }
                    _ => return Poll::Ready(Some((stream, address))),
                },
            }
        }
    }
}
