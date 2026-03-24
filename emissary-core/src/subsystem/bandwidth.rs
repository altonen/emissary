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

use crate::{config::BandwidthConfig, primitives::Bandwidth, runtime::Runtime, subsystem::Source};

use futures::FutureExt;

use alloc::sync::Arc;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU8, Ordering},
    task::{Context, Poll},
    time::Duration,
};

/// How often is router's bandwidth usage calculated.
const BANDWIDTH_MEASUREMENT_INTERVAL: Duration = Duration::from_secs(1);

/// Number of slots for the 5-second meter.
const SHORT_WINDOW_LEN: usize = 5usize;

/// Number of slots for the 5-minute meter.
const MEDIUM_WINDOW_LEN: usize = 300usize;

/// High congestion threshold.
const HIGH_CONGESTION_THRESHOLD: f64 = 0.9f64;

/// Medium congestion threshold.
const MEDIUM_CONGESTION_THRESHOLD: f64 = 0.7f64;

/// Congestion level based on bandwidth usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CongestionLevel {
    /// Low congestion: bandwidth usage is below 70%.
    #[default]
    Low = 0,

    /// Medium congestion: bandwidth usage is between 70% and 90%.
    Medium = 1,

    /// High congestion: bandwidth usage is above 90%.
    High = 2,
}

impl CongestionLevel {
    /// Convert from u8 value.
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            _ => Self::Low,
        }
    }
}

/// Current congestion level of the router.
///
/// Either short or medium-term congestion, depending on which meter this object is bound to.
#[derive(Debug, Default, Clone)]
pub struct Congestion {
    /// Curren congestion.
    congestion: Arc<AtomicU8>,

    /// Bandwidth class of the router.
    bandwidth: Bandwidth,
}

impl Congestion {
    /// Create new `Congestion` with bandwidth class.
    pub fn new(bandwidth: Bandwidth) -> Self {
        Self {
            bandwidth,
            ..Default::default()
        }
    }

    /// Store a new congestion level.
    pub fn store(&self, congestion: CongestionLevel) {
        self.congestion.store(congestion as u8, Ordering::Relaxed);
    }

    /// Load current congestion level.
    pub fn load(&self) -> CongestionLevel {
        CongestionLevel::from_u8(self.congestion.load(Ordering::Relaxed))
    }

    /// Get bandwidth class of the router.
    pub fn bandwidth(&self) -> Bandwidth {
        self.bandwidth
    }
}

/// Bandwidth slot.
#[derive(Debug, Default, Clone, Copy)]
struct BandwidthSlot {
    /// Total inbound bytes in this slot.
    inbound: usize,

    /// Total outbound bytes in this slot.
    outbound: usize,

    /// Total transit inbound bytes in this slot.
    transit_inbound: usize,

    /// Total transit outbound bytes in this slot.
    transit_outbound: usize,
}

impl BandwidthSlot {
    /// Reset the slot to zero.
    fn reset(&mut self) {
        self.inbound = 0;
        self.outbound = 0;
        self.transit_inbound = 0;
        self.transit_outbound = 0;
    }

    /// Get total bandwidth.
    fn total(&self) -> usize {
        self.inbound + self.outbound
    }
}

/// Bandwidth meter.
#[derive(Debug)]
struct BandwidthMeter<const N: usize> {
    /// Ring buffer of bandwidth slots.
    slots: [BandwidthSlot; N],

    /// Current slot index.
    current_slot: usize,

    /// Number of slots that have been filled.
    ///
    /// Used to provide more accurate readings during start-up period.
    filled_slots: usize,
}

impl<const N: usize> BandwidthMeter<N> {
    /// Create a new `BandwidthMeter`.
    fn new() -> Self {
        Self {
            slots: [BandwidthSlot::default(); N],
            current_slot: 0,
            filled_slots: 1,
        }
    }

    /// Record inbound bandwidth.
    fn record_inbound(&mut self, bytes: usize, is_transit: bool) {
        self.slots[self.current_slot].inbound += bytes;

        if is_transit {
            self.slots[self.current_slot].transit_inbound += bytes;
        }
    }

    /// Record outbound bandwidth.
    fn record_outbound(&mut self, bytes: usize, is_transit: bool) {
        self.slots[self.current_slot].outbound += bytes;

        if is_transit {
            self.slots[self.current_slot].transit_outbound += bytes;
        }
    }

    /// Advance to the next time slot.
    fn advance(&mut self) {
        self.current_slot = (self.current_slot + 1) % N;
        self.slots[self.current_slot].reset();

        if self.filled_slots < N {
            self.filled_slots += 1;
        }
    }

    /// Calculate the average bandwidth per second.
    fn average_bandwidth_per_second(&self) -> usize {
        self.slots.iter().take(self.filled_slots).map(|s| s.total()).sum::<usize>()
            / self.filled_slots
    }
}

/// Bandwidth tracker.
pub struct BandwidthTracker<R: Runtime> {
    /// Maximum amount of traffic per second, in bytes.
    bandwidth: usize,

    /// Timer for collecting bandwidth measurements.
    bandwidth_timer: R::Timer,

    /// Current short-term congestion level.
    congestion_medium: Congestion,

    /// Current short-term congestion level.
    congestion_short: Congestion,

    /// Current second's inbound bandwidth (for real-time decisions).
    current_inbound: usize,

    /// Current second's outbound bandwidth (for real-time decisions).
    current_outbound: usize,

    /// Current second's transit inbound bandwidth.
    current_transit_inbound: usize,

    /// Current second's transit outbound bandwidth.
    current_transit_outbound: usize,

    /// Maximum amount of transit traffic per second, in bytes.
    max_transit: usize,

    /// 5-minute bandwidth meter.
    meter_medium: BandwidthMeter<MEDIUM_WINDOW_LEN>,

    /// 5-second bandwidth meter.
    meter_short: BandwidthMeter<SHORT_WINDOW_LEN>,
}

impl<R: Runtime> BandwidthTracker<R> {
    /// Create new `BandwidthTracker`.
    ///
    /// Returns three objects:
    /// * `BandwidthTracker` given to `SubsystemManager`
    /// * `Congestion` for short-term congestion detection, given to `TransitTunnelManager`
    /// * `Congestion` for medium-term congestion detection, given to `TransportManager`
    ///
    /// `TransitTunnelManager` uses the `Congestion` object to detect congestion. If congestion is
    /// high, all tunnnel build requests are rejected and if congestion is medium, the inbound
    /// tunnel build request is rejected with a 50/50 chance.
    ///
    /// `TransportManager` uses the `Congestion` object to publish correct congestion caps and
    /// bandwidth class.
    pub fn new(config: BandwidthConfig) -> (Self, Congestion, Congestion) {
        let max_transit = (config.bandwidth as f64 * config.share_ratio) as usize;
        let congestion_short = Congestion::new(Bandwidth::from(max_transit));
        let congestion_medium = Congestion::new(Bandwidth::from(max_transit));

        (
            Self {
                bandwidth: config.bandwidth,
                bandwidth_timer: R::timer(BANDWIDTH_MEASUREMENT_INTERVAL),
                congestion_medium: congestion_medium.clone(),
                congestion_short: congestion_short.clone(),
                current_inbound: 0,
                current_outbound: 0,
                current_transit_inbound: 0,
                current_transit_outbound: 0,
                max_transit,
                meter_medium: BandwidthMeter::new(),
                meter_short: BandwidthMeter::new(),
            },
            congestion_short,
            congestion_medium,
        )
    }

    /// Get the current bandwidth usage.
    fn current_bandwidth(&self) -> usize {
        self.current_inbound + self.current_outbound
    }

    /// Get the current transit bandwidth usage.
    fn current_transit_bandwidth(&self) -> usize {
        self.current_transit_inbound + self.current_transit_outbound
    }

    /// Check if we should drop this message based on bandwidth limits.
    ///
    /// Transit traffic is dropped first if bandwidth is exceeded.
    fn should_drop(&self, size: usize, source: Source) -> bool {
        let projected = self.current_bandwidth() + size;

        if projected <= self.bandwidth {
            if source.is_transit() {
                let projected = self.current_transit_bandwidth() + size;
                return projected > self.max_transit;
            }
            return false;
        }

        // always drop transit traffic when over the limit
        if source.is_transit() {
            return true;
        }

        // start dropping local traffic if we're 10% over the limit to give some headroom
        projected > self.bandwidth + (self.bandwidth / 10)
    }

    /// Updated outbound bandwidth.
    ///
    /// Returns `true` if the message should be dropped because bandwidth limits have been exceeded.
    pub fn update_outbound(&mut self, size: usize, source: Source) -> bool {
        if self.should_drop(size, source) {
            return true;
        }

        self.current_outbound += size;
        self.meter_short.record_outbound(size, source.is_transit());
        self.meter_medium.record_outbound(size, source.is_transit());

        if source.is_transit() {
            self.current_transit_outbound += size;
        }

        false
    }

    /// Update inbound bandwidth.
    ///
    /// Returns `true` if the message should be dropped because bandwidth limits have been exceeded.
    pub fn update_inbound(&mut self, bandwidth: usize, source: Source) -> bool {
        if self.should_drop(bandwidth, source) {
            return true;
        }

        self.current_inbound += bandwidth;
        self.meter_short.record_inbound(bandwidth, source.is_transit());
        self.meter_medium.record_inbound(bandwidth, source.is_transit());

        if source.is_transit() {
            self.current_transit_inbound += bandwidth;
        }

        false
    }

    /// Calculate congestion and update congestion states.
    fn calculate_congestion(&mut self) {
        // calculate average short and medium-term bandwidths
        let avg_short = self.meter_short.average_bandwidth_per_second();
        let avg_medium = self.meter_medium.average_bandwidth_per_second();

        let get_congestion_level = |avg: usize| -> CongestionLevel {
            let high_threshold = (self.bandwidth as f64 * HIGH_CONGESTION_THRESHOLD) as usize;
            let medium_threshold = (self.bandwidth as f64 * MEDIUM_CONGESTION_THRESHOLD) as usize;

            if avg > high_threshold {
                CongestionLevel::High
            } else if avg > medium_threshold {
                CongestionLevel::Medium
            } else {
                CongestionLevel::Low
            }
        };

        // calculate congestion levels
        self.congestion_short.store(get_congestion_level(avg_short));
        self.congestion_medium.store(get_congestion_level(avg_medium));

        // reset current measurements and advance meters
        self.current_inbound = 0;
        self.current_outbound = 0;
        self.current_transit_inbound = 0;
        self.current_transit_outbound = 0;

        self.meter_short.advance();
        self.meter_medium.advance();
    }
}

impl<R: Runtime> Future for BandwidthTracker<R> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.bandwidth_timer.poll_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(()) => {
                    self.calculate_congestion();
                    self.bandwidth_timer = R::timer(BANDWIDTH_MEASUREMENT_INTERVAL);
                }
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mock::MockRuntime;

    #[tokio::test]
    async fn no_bandwidth_shared() {
        let (tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 100 * 1024,
            share_ratio: 0.0,
        });

        assert_eq!(tracker.bandwidth, 100 * 1024);
        assert_eq!(tracker.max_transit, 0);
    }

    #[tokio::test]
    async fn all_bandwidth_shared() {
        let (tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 100 * 1024,
            share_ratio: 1.0,
        });

        assert_eq!(tracker.bandwidth, 100 * 1024);
        assert_eq!(tracker.max_transit, 100 * 1024);
    }

    #[tokio::test]
    async fn transit_dropped_when_over_transit_limit() {
        let (mut tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 0.5,
        });

        assert!(!tracker.update_inbound(400, Source::Transit));
        assert!(!tracker.update_inbound(100, Source::Transit));

        // transit dropped as it's over the transit limit (500 bytes)
        //
        // local traffic has capacity left
        assert!(tracker.update_inbound(100, Source::Transit));
        assert!(!tracker.update_inbound(100, Source::Client));
    }

    #[tokio::test]
    async fn transit_dropped_first_when_over_total_limit() {
        let (mut tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 1.0,
        });

        assert!(!tracker.update_inbound(900, Source::Client));

        // tarnsit traffic is dropped as it's over the limit by 100 bytes
        //
        // local traffit has 10% of headroom
        assert!(tracker.update_inbound(200, Source::Transit));
        assert!(!tracker.update_inbound(200, Source::Client));
    }

    #[tokio::test]
    async fn bandwidth_recorded_in_meters() {
        let (mut tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 10000,
            share_ratio: 0.8,
        });

        tracker.update_inbound(100, Source::Client);
        tracker.update_outbound(1000, Source::Transit);

        // both meters should have the same value
        assert_eq!(tracker.meter_short.average_bandwidth_per_second(), 1100);
        assert_eq!(tracker.meter_medium.average_bandwidth_per_second(), 1100);
    }

    #[tokio::test]
    async fn congestion_low_when_under_70_percent() {
        let (mut tracker, congestion, _) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 0.8,
        });

        tracker.update_inbound(600, Source::Client);
        tracker.calculate_congestion();

        assert_eq!(congestion.load(), CongestionLevel::Low);
    }

    #[tokio::test]
    async fn congestion_medium_when_between_70_and_90_percent() {
        let (mut tracker, congestion, _) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 0.8,
        });

        tracker.update_inbound(800, Source::Client);
        tracker.calculate_congestion();

        assert_eq!(congestion.load(), CongestionLevel::Medium);
    }

    #[tokio::test]
    async fn congestion_high_when_over_90_percent() {
        let (mut tracker, congestion, _) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 0.8,
        });

        tracker.update_inbound(950, Source::Client);
        tracker.calculate_congestion();

        assert_eq!(congestion.load(), CongestionLevel::High);
    }

    #[tokio::test]
    async fn meter_average_calculation() {
        let mut meter = BandwidthMeter::<5>::new();

        meter.record_inbound(100, false);
        meter.advance();

        meter.record_inbound(200, false);

        assert_eq!(meter.average_bandwidth_per_second(), 150);
    }

    #[tokio::test]
    async fn meter_transit_tracking() {
        let (mut tracker, ..) = BandwidthTracker::<MockRuntime>::new(BandwidthConfig {
            bandwidth: 1000,
            share_ratio: 0.8,
        });

        tracker.update_inbound(100, Source::Transit);
        tracker.update_inbound(50, Source::Exploratory);
        tracker.update_outbound(75, Source::Transit);
        tracker.update_outbound(25, Source::NetDb);

        assert_eq!(tracker.current_bandwidth(), 250);
        assert_eq!(tracker.current_transit_bandwidth(), 175);
    }
}
