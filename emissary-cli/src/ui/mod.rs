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

use std::{
    fmt,
    time::{Duration, Instant},
};

#[cfg(feature = "ui")]
pub mod dioxus;

/// Router status.
#[allow(dead_code)]
enum Status {
    /// Router is active.
    Active,

    /// Router is shutting down.
    ShuttingDown(Instant),
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::ShuttingDown(shutdown_started) => {
                let remaining = (Duration::from_secs(10 * 60)
                    .saturating_sub(shutdown_started.elapsed()))
                .as_secs();

                if remaining > 60 {
                    write!(
                        f,
                        "Shutting down ({} m {} s)",
                        remaining / 60,
                        remaining % 60
                    )
                } else if remaining == 0 {
                    write!(f, "Shutting down now")
                } else {
                    write!(f, "Shutting down ({remaining} s)")
                }
            }
        }
    }
}

/// Calculate bandwidth.
#[allow(dead_code)]
fn calculate_bandwidth(bandwidth: f64) -> (f64, &'static str) {
    if bandwidth < 1000f64 {
        return (bandwidth, "B");
    }

    if bandwidth < 1000f64 * 1000f64 {
        return (bandwidth / 1000f64, "KB");
    }

    if bandwidth < 1000f64 * 1000f64 * 1000f64 {
        return (bandwidth / (1000f64 * 1000f64), "MB");
    }

    if bandwidth < 1000f64 * 1000f64 * 1000f64 {
        return (bandwidth / (1000f64 * 1000f64), "GB");
    }

    if bandwidth < 1000f64 * 1000f64 * 1000f64 * 1000f64 {
        return (bandwidth / (1000f64 * 1000f64 * 1000f64), "GB");
    }

    (bandwidth / (1000f64 * 1000f64 * 1000f64 * 1000f64), "TB")
}
