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

//! Bounded observability primitives for I2PControl.
//!
//! - `LogRing`: bounded, redacted, independently clearable log buffer.
//! - `MetricsSnapshot`: cloneable cumulative metrics source.
//! - `RollingWindow`: fixed-bucket rolling traffic accumulator.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// --- Log Ring ---

/// A bounded, redacted, independently clearable log ring buffer.
///
/// Captures sanitized formatted events into an in-memory ring with fixed
/// maximum entries and total bytes. Evicts oldest entries deterministically.
/// Clear affects only this ring; existing terminal/file sinks remain unchanged.
pub struct LogRing {
    inner: Mutex<LogRingInner>,
    generation: AtomicU64,
    max_entries: usize,
    max_bytes: usize,
}

struct LogRingInner {
    entries: VecDeque<LogEntry>,
    total_bytes: usize,
}

/// A single log entry in the ring.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogRing {
    /// Create a new log ring with the given bounds.
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(LogRingInner {
                entries: VecDeque::new(),
                total_bytes: 0,
            }),
            generation: AtomicU64::new(0),
            max_entries,
            max_bytes,
        }
    }

    /// Push a log entry into the ring. Evicts oldest entries if bounds exceeded.
    ///
    /// This method is non-blocking and does not perform I/O.
    pub fn push(&self, entry: LogEntry) {
        let entry_size = entry.level.len() + entry.target.len() + entry.message.len() + 64;
        let mut inner = self.inner.lock().unwrap();

        // Evict oldest entries until we fit
        while inner.entries.len() >= self.max_entries
            || (inner.total_bytes + entry_size > self.max_bytes && !inner.entries.is_empty())
        {
            if let Some(old) = inner.entries.pop_front() {
                let old_size = old.level.len() + old.target.len() + old.message.len() + 64;
                inner.total_bytes = inner.total_bytes.saturating_sub(old_size);
            }
        }

        inner.total_bytes += entry_size;
        inner.entries.push_back(entry);
    }

    /// Take an immutable snapshot of the current log entries.
    ///
    /// Returns entries in chronological order and the current generation.
    /// Concurrent readers receive a coherent before-or-after generation.
    pub fn snapshot(&self) -> (Vec<LogEntry>, u64) {
        let inner = self.inner.lock().unwrap();
        let generation = self.generation.load(Ordering::Acquire);
        (inner.entries.iter().cloned().collect(), generation)
    }

    /// Clear the ring and increment the generation.
    ///
    /// Only affects this ring. Existing terminal/file formatting and
    /// filter reload remain unchanged.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.entries.clear();
        inner.total_bytes = 0;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Current number of entries.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.entries.len()
    }

    /// Whether the ring is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.entries.is_empty()
    }
}

impl Default for LogRing {
    fn default() -> Self {
        Self::new(1000, 512 * 1024)
    }
}

// --- Metrics Snapshot ---

/// Cloneable passive cumulative metrics snapshot source.
///
/// Replaces dependence on the single `EventSubscriber` with a multi-consumer
/// observable metrics source. Counters are monotonic except process restart.
/// Snapshot reads are non-destructive.
pub struct MetricsSnapshot {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    total_transport_received: AtomicU64,
    total_transport_sent: AtomicU64,
    total_transit_received: AtomicU64,
    total_transit_sent: AtomicU64,
    connected_routers: AtomicU64,
    participating_tunnels: AtomicU64,
    tunnel_build_successes: AtomicU64,
    tunnel_build_failures: AtomicU64,
    uptime_start: Instant,
}

/// A point-in-time snapshot of cumulative metrics.
#[derive(Debug, Clone)]
pub struct MetricsData {
    pub total_transport_received: u64,
    pub total_transport_sent: u64,
    pub total_transit_received: u64,
    pub total_transit_sent: u64,
    pub connected_routers: usize,
    pub participating_tunnels: usize,
    pub tunnel_build_successes: u64,
    pub tunnel_build_failures: u64,
    pub uptime_ms: u64,
}

impl MetricsSnapshot {
    /// Create a new metrics snapshot source.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                total_transport_received: AtomicU64::new(0),
                total_transport_sent: AtomicU64::new(0),
                total_transit_received: AtomicU64::new(0),
                total_transit_sent: AtomicU64::new(0),
                connected_routers: AtomicU64::new(0),
                participating_tunnels: AtomicU64::new(0),
                tunnel_build_successes: AtomicU64::new(0),
                tunnel_build_failures: AtomicU64::new(0),
                uptime_start: Instant::now(),
            }),
        }
    }

    /// Record transport bytes received.
    pub fn record_transport_received(&self, bytes: u64) {
        self.inner.total_transport_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record transport bytes sent.
    pub fn record_transport_sent(&self, bytes: u64) {
        self.inner.total_transport_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record transit bytes received.
    pub fn record_transit_received(&self, bytes: u64) {
        self.inner.total_transit_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record transit bytes sent.
    pub fn record_transit_sent(&self, bytes: u64) {
        self.inner.total_transit_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Set connected routers count.
    pub fn set_connected_routers(&self, count: usize) {
        self.inner.connected_routers.store(count as u64, Ordering::Relaxed);
    }

    /// Set participating tunnels count.
    pub fn set_participating_tunnels(&self, count: usize) {
        self.inner.participating_tunnels.store(count as u64, Ordering::Relaxed);
    }

    /// Record a tunnel build success.
    pub fn record_tunnel_build_success(&self) {
        self.inner.tunnel_build_successes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a tunnel build failure.
    pub fn record_tunnel_build_failure(&self) {
        self.inner.tunnel_build_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Take a non-destructive snapshot of all cumulative metrics.
    pub fn snapshot(&self) -> MetricsData {
        let inner = &self.inner;
        MetricsData {
            total_transport_received: inner.total_transport_received.load(Ordering::Relaxed),
            total_transport_sent: inner.total_transport_sent.load(Ordering::Relaxed),
            total_transit_received: inner.total_transit_received.load(Ordering::Relaxed),
            total_transit_sent: inner.total_transit_sent.load(Ordering::Relaxed),
            connected_routers: inner.connected_routers.load(Ordering::Relaxed) as usize,
            participating_tunnels: inner.participating_tunnels.load(Ordering::Relaxed) as usize,
            tunnel_build_successes: inner.tunnel_build_successes.load(Ordering::Relaxed),
            tunnel_build_failures: inner.tunnel_build_failures.load(Ordering::Relaxed),
            uptime_ms: inner.uptime_start.elapsed().as_millis() as u64,
        }
    }
}

impl Clone for MetricsSnapshot {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

// --- Rolling Window ---

/// Fixed-bucket rolling traffic accumulator for Proposal 170.
///
/// Covers a fixed interval (e.g. 15 seconds) with deterministic boundary
/// inclusion. Read is O(buckets), not O(events). Uses monotonic clock.
pub struct RollingWindow {
    inner: Mutex<RollingInner>,
    bucket_duration_ms: u64,
    num_buckets: usize,
}

struct RollingInner {
    buckets: VecDeque<Bucket>,
    #[allow(dead_code)]
    last_update: Instant,
}

#[derive(Debug, Clone, Default)]
struct Bucket {
    start_ms: u64,
    inbound: u64,
    outbound: u64,
}

/// Rolling window data point.
#[derive(Debug, Clone, Default)]
pub struct RollingData {
    pub inbound_1s: u64,
    pub outbound_1s: u64,
    pub inbound_15s: u64,
    pub outbound_15s: u64,
}

impl RollingWindow {
    /// Create a new rolling window.
    ///
    /// `bucket_duration_ms` is the duration of each bucket.
    /// `num_buckets` is how many buckets cover the rolling interval.
    pub fn new(bucket_duration_ms: u64, num_buckets: usize) -> Self {
        Self {
            inner: Mutex::new(RollingInner {
                buckets: VecDeque::with_capacity(num_buckets),
                last_update: Instant::now(),
            }),
            bucket_duration_ms,
            num_buckets,
        }
    }

    /// Record traffic bytes for the current time period.
    pub fn record(&self, inbound: u64, outbound: u64) {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let now_ms = now.elapsed().as_millis() as u64;

        // Evict expired buckets
        self.evict_expired(&mut inner, now_ms);

        // Update or create current bucket
        let bucket_start = (now_ms / self.bucket_duration_ms) * self.bucket_duration_ms;
        match inner.buckets.back_mut() {
            Some(bucket) if bucket.start_ms == bucket_start => {
                bucket.inbound += inbound;
                bucket.outbound += outbound;
            }
            _ => {
                inner.buckets.push_back(Bucket {
                    start_ms: bucket_start,
                    inbound,
                    outbound,
                });
            }
        }
    }

    /// Read the rolling window data.
    pub fn read(&self) -> RollingData {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        let now_ms = now.elapsed().as_millis() as u64;

        self.evict_expired(&mut inner, now_ms);

        let mut data = RollingData::default();

        for bucket in &inner.buckets {
            let age_ms = now_ms.saturating_sub(bucket.start_ms);

            // 1-second window
            if age_ms <= 1000 {
                data.inbound_1s += bucket.inbound;
                data.outbound_1s += bucket.outbound;
            }

            // 15-second window
            if age_ms <= 15000 {
                data.inbound_15s += bucket.inbound;
                data.outbound_15s += bucket.outbound;
            }
        }

        data
    }

    fn evict_expired(&self, inner: &mut RollingInner, now_ms: u64) {
        let max_age = self.bucket_duration_ms * self.num_buckets as u64;
        while let Some(front) = inner.buckets.front() {
            if now_ms.saturating_sub(front.start_ms) > max_age {
                inner.buckets.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Default for RollingWindow {
    fn default() -> Self {
        // 1-second buckets, 15 buckets = 15-second window
        Self::new(1000, 15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- LogRing tests ---

    #[test]
    fn log_ring_push_and_snapshot() {
        let ring = LogRing::new(10, 1024 * 1024);
        ring.push(LogEntry {
            timestamp_ms: 1000,
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "hello".to_string(),
        });

        let (entries, gen) = ring.snapshot();
        assert_eq!(entries.len(), 1);
        assert_eq!(gen, 0);
        assert_eq!(entries[0].message, "hello");
    }

    #[test]
    fn log_ring_eviction() {
        let ring = LogRing::new(3, 1024 * 1024);
        for i in 0..5 {
            ring.push(LogEntry {
                timestamp_ms: i * 1000,
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg-{i}"),
            });
        }

        let (entries, _) = ring.snapshot();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "msg-2");
        assert_eq!(entries[2].message, "msg-4");
    }

    #[test]
    fn log_ring_clear() {
        let ring = LogRing::new(10, 1024 * 1024);
        ring.push(LogEntry {
            timestamp_ms: 1000,
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "hello".to_string(),
        });

        ring.clear();

        let (entries, gen) = ring.snapshot();
        assert!(entries.is_empty());
        assert_eq!(gen, 1);
    }

    #[test]
    fn log_ring_byte_eviction() {
        let ring = LogRing::new(100, 200);
        for i in 0..10 {
            ring.push(LogEntry {
                timestamp_ms: i * 1000,
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: "x".repeat(50),
            });
        }

        let (entries, _) = ring.snapshot();
        assert!(entries.len() < 10);
    }

    #[test]
    fn log_ring_concurrent_read_clear() {
        let ring = Arc::new(LogRing::new(100, 1024 * 1024));
        for i in 0..10 {
            ring.push(LogEntry {
                timestamp_ms: i * 1000,
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: format!("msg-{i}"),
            });
        }

        let ring2 = Arc::clone(&ring);
        let handle = std::thread::spawn(move || {
            let (_, gen) = ring2.snapshot();
            gen
        });

        ring.clear();
        let gen = handle.join().unwrap();
        // Generation is either 0 (before clear) or 1 (after clear)
        assert!(gen <= 1);
    }

    // --- MetricsSnapshot tests ---

    #[test]
    fn metrics_snapshot_defaults() {
        let metrics = MetricsSnapshot::new();
        let data = metrics.snapshot();
        assert_eq!(data.total_transport_received, 0);
        assert_eq!(data.total_transport_sent, 0);
        assert_eq!(data.connected_routers, 0);
        assert_eq!(data.tunnel_build_successes, 0);
    }

    #[test]
    fn metrics_snapshot_cumulative() {
        let metrics = MetricsSnapshot::new();
        metrics.record_transport_received(100);
        metrics.record_transport_sent(200);
        metrics.record_transit_received(50);
        metrics.record_transit_sent(75);
        metrics.set_connected_routers(10);
        metrics.set_participating_tunnels(5);
        metrics.record_tunnel_build_success();
        metrics.record_tunnel_build_success();
        metrics.record_tunnel_build_failure();

        let data = metrics.snapshot();
        assert_eq!(data.total_transport_received, 100);
        assert_eq!(data.total_transport_sent, 200);
        assert_eq!(data.total_transit_received, 50);
        assert_eq!(data.total_transit_sent, 75);
        assert_eq!(data.connected_routers, 10);
        assert_eq!(data.participating_tunnels, 5);
        assert_eq!(data.tunnel_build_successes, 2);
        assert_eq!(data.tunnel_build_failures, 1);
    }

    #[test]
    fn metrics_snapshot_non_destructive() {
        let metrics = MetricsSnapshot::new();
        metrics.record_transport_received(100);

        let data1 = metrics.snapshot();
        let data2 = metrics.snapshot();
        assert_eq!(
            data1.total_transport_received,
            data2.total_transport_received
        );
    }

    #[test]
    fn metrics_snapshot_clone_shares_state() {
        let m1 = MetricsSnapshot::new();
        let m2 = m1.clone();
        m1.record_transport_received(42);
        assert_eq!(m2.snapshot().total_transport_received, 42);
    }

    // --- RollingWindow tests ---

    #[test]
    fn rolling_window_empty() {
        let window = RollingWindow::new(1000, 15);
        let data = window.read();
        assert_eq!(data.inbound_1s, 0);
        assert_eq!(data.outbound_1s, 0);
        assert_eq!(data.inbound_15s, 0);
        assert_eq!(data.outbound_15s, 0);
    }

    #[test]
    fn rolling_window_record() {
        let window = RollingWindow::new(1000, 15);
        window.record(100, 200);
        let data = window.read();
        assert_eq!(data.inbound_1s, 100);
        assert_eq!(data.outbound_1s, 200);
        assert_eq!(data.inbound_15s, 100);
        assert_eq!(data.outbound_15s, 200);
    }

    #[test]
    fn rolling_window_default_1s_buckets() {
        let window = RollingWindow::default();
        // Default should be 1-second buckets, 15 buckets = 15s window
        window.record(50, 75);
        let data = window.read();
        assert_eq!(data.inbound_1s, 50);
        assert_eq!(data.outbound_1s, 75);
    }
}
