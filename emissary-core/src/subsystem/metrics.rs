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

use crate::runtime::MetricType;

use alloc::vec::Vec;

// bandwidth
pub const MAX_BANDWIDTH: &str = "subsystem_bandwidth_count";
pub const MAX_TRANSIT_BANDWIDTH: &str = "subsystem_transit_bandwidth_count";

/// Register subsystem metrics.
pub fn register_metrics(mut metrics: Vec<MetricType>) -> Vec<MetricType> {
    // gauges
    metrics.push(MetricType::Gauge {
        name: MAX_BANDWIDTH,
        description: "max bandwidth per second",
    });
    metrics.push(MetricType::Gauge {
        name: MAX_TRANSIT_BANDWIDTH,
        description: "max transit bandwidth per second",
    });

    metrics
}
