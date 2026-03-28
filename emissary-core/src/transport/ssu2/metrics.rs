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

use alloc::{vec, vec::Vec};

// general
pub const INBOUND_BANDWIDTH: &str = "ssu2_inbound_bytes_count";
pub const OUTBOUND_BANDWIDTH: &str = "ssu2_outbound_bytes_count";
pub const HANDSHAKE_DURATION: &str = "ssu2_handshake_duration_buckets";
pub const NUM_HANDSHAKE_FAILURES: &str = "ssu2_handshake_failure_count";
pub const NUM_HANDSHAKE_SUCCESSES: &str = "ssu2_handshake_success_count";
pub const NUM_CONNECTIONS: &str = "ssu2_connection_count";
pub const NUM_ACTIVE_CONNECTIONS: &str = "ssu2_active_connection_count";
pub const CONNECTIONS_OPENED: &str = "ssu2_connections_opened_count";
pub const CONNECTIONS_CLOSED: &str = "ssu2_connections_closed_count";

// active connection
pub const INBOUND_PKT_SIZES: &str = "ssu2_ib_pkt_sizes";
pub const INBOUND_PKTS: &str = "ssu2_ib_pkt_count";
pub const OUTBOUND_PKTS: &str = "ssu2_ob_pkt_count";
pub const DROPPED_PKTS: &str = "ssu2_dropped_pkts";
pub const DUPLICATE_PKTS: &str = "ssu2_duplicate_pkt_count";
pub const RETRANSMISSION_COUNT: &str = "ssu2_retransmission_count";
pub const INBOUND_FRAGMENT_COUNT: &str = "ssu2_inbound_fragment_count";
pub const OUTBOUND_FRAGMENT_COUNT: &str = "ssu2_outbound_fragment_count";
pub const GARBAGE_COLLECTED_COUNT: &str = "ssu2_gc_fragments_count";
pub const ACK_RECEIVE_TIME: &str = "ssu2_ack_receive_times";
pub const SESSION_DURATION: &str = "ssu2_session_durations";

// relay
pub const RELAY_SUCCESS: &str = "ssu2_relay_success_count";
pub const RELAY_FAILURE: &str = "ssu2_relay_failure_count";

// peer test
pub const NUM_ACTIVE_PEER_TESTS: &str = "ssu2_active_peer_test_count";
pub const IPV4_PEER_TEST_RESULT: &str = "ssu2_ipv4_peer_test_result_count";
pub const IPV6_PEER_TEST_RESULT: &str = "ssu2_ipv6_peer_test_result_count";

/// Register SSU2 metrics.
pub fn register_metrics(mut metrics: Vec<MetricType>) -> Vec<MetricType> {
    // counters
    metrics.push(MetricType::Counter {
        name: NUM_HANDSHAKE_SUCCESSES,
        description: "how many times the ssu2 handshake has succeeded",
    });
    metrics.push(MetricType::Counter {
        name: NUM_HANDSHAKE_FAILURES,
        description: "how many times the ssu2 handshake has failed",
    });
    metrics.push(MetricType::Counter {
        name: INBOUND_BANDWIDTH,
        description: "total amount of received bytes",
    });
    metrics.push(MetricType::Counter {
        name: OUTBOUND_BANDWIDTH,
        description: "total amount of sent bytes",
    });
    metrics.push(MetricType::Counter {
        name: RETRANSMISSION_COUNT,
        description: "how many packets have been resent",
    });
    metrics.push(MetricType::Counter {
        name: INBOUND_PKTS,
        description: "how many packets have been received",
    });
    metrics.push(MetricType::Counter {
        name: OUTBOUND_PKTS,
        description: "how many packets have been sent",
    });
    metrics.push(MetricType::Counter {
        name: DROPPED_PKTS,
        description: "how many packets have been dropped",
    });
    metrics.push(MetricType::Counter {
        name: DUPLICATE_PKTS,
        description: "how many duplicate packets have been received",
    });
    metrics.push(MetricType::Counter {
        name: GARBAGE_COLLECTED_COUNT,
        description: "how many fragments have been garbage collected",
    });
    metrics.push(MetricType::Counter {
        name: RELAY_FAILURE,
        description: "how many relay requests have failed",
    });
    metrics.push(MetricType::Counter {
        name: RELAY_SUCCESS,
        description: "how many relay requests have succeeded",
    });
    metrics.push(MetricType::Counter {
        name: IPV4_PEER_TEST_RESULT,
        description: "how many ipv4 peer test result processes have concluded",
    });
    metrics.push(MetricType::Counter {
        name: IPV6_PEER_TEST_RESULT,
        description: "how many ipv6 peer test result processes have concluded",
    });
    metrics.push(MetricType::Counter {
        name: CONNECTIONS_OPENED,
        description: "how many connections have been opened",
    });
    metrics.push(MetricType::Counter {
        name: CONNECTIONS_CLOSED,
        description: "how many connections have been closed",
    });
    metrics.push(MetricType::Counter {
        name: NUM_CONNECTIONS,
        description: "how many connections have been opened",
    });

    // gauges
    metrics.push(MetricType::Gauge {
        name: NUM_ACTIVE_PEER_TESTS,
        description: "how many peer tests are active",
    });
    metrics.push(MetricType::Gauge {
        name: NUM_ACTIVE_CONNECTIONS,
        description: "how many active connections there are",
    });

    // histograms
    metrics.push(MetricType::Histogram {
        name: HANDSHAKE_DURATION,
        description: "how long it takes for the handshake to finish",
        buckets: vec![
            50f64, 100f64, 150f64, 200f64, 250f64, 300f64, 350f64, 400f64, 450f64, 500f64, 600f64,
            700f64, 800f64, 900f64, 1000f64, 3000f64, 5000f64, 10_000f64,
        ],
    });
    metrics.push(MetricType::Histogram {
        name: INBOUND_PKT_SIZES,
        description: "inbound packet sizes",
        buckets: vec![
            24f64, 100f64, 300f64, 500f64, 700f64, 900f64, 1100f64, 1200f64, 1300f64, 1400f64,
            1500f64,
        ],
    });
    metrics.push(MetricType::Histogram {
        name: ACK_RECEIVE_TIME,
        description: "ack times",
        buckets: vec![
            50f64, 90f64, 130f64, 170f64, 210f64, 250f64, 290f64, 330f64, 370f64, 500f64, 800f64,
            1000f64,
        ],
    });
    metrics.push(MetricType::Histogram {
        name: INBOUND_FRAGMENT_COUNT,
        description: "how many fragments outbound messages contain",
        buckets: vec![1f64, 2f64, 4f64, 6f64, 8f64, 10f64, 15f64, 20f64],
    });
    metrics.push(MetricType::Histogram {
        name: OUTBOUND_FRAGMENT_COUNT,
        description: "how many fragments outbound messages contain",
        buckets: vec![1f64, 2f64, 4f64, 6f64, 8f64, 10f64, 15f64, 20f64],
    });
    metrics.push(MetricType::Histogram {
        name: SESSION_DURATION,
        description: "ssu2 session durations",
        buckets: vec![
            10f64, 30f64, 60f64, 120f64, 240f64, 300f64, 360f64, 420f64, 480f64, 540f64, 600f64,
        ],
    });

    metrics
}
