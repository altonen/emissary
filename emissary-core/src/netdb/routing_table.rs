// Copyright 2018 Parity Technologies (UK) Ltd.
// Copyright 2023 litep2p developers
//
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

//! Kademlia routing table implementation.

use crate::{
    netdb::{
        bucket::KBucket,
        types::{Distance, Key},
    },
    primitives::RouterId,
};

use hashbrown::HashSet;

use alloc::vec::Vec;
use core::ops::Deref;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::netdb::routing-table";

/// Number of k-buckets.
const NUM_BUCKETS: usize = 256;

pub struct RoutingTable {
    /// Local key.
    local_key: Key<RouterId>,

    /// K-buckets.
    buckets: Vec<KBucket>,
}

/// A (type-safe) index into a `KBucketsTable`, i.e. a non-negative integer in the
/// interval `[0, NUM_BUCKETS)`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BucketIndex(usize);

impl Deref for BucketIndex {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl BucketIndex {
    /// Creates a new `BucketIndex` for a `Distance`.
    ///
    /// The given distance is interpreted as the distance from a `local_key` of
    /// a `KBucketsTable`. If the distance is zero, `None` is returned, in
    /// recognition of the fact that the only key with distance `0` to a
    /// `local_key` is the `local_key` itself, which does not belong in any
    /// bucket.
    fn new(d: &Distance) -> Option<BucketIndex> {
        d.ilog2().map(|i| BucketIndex(i as usize))
    }
}

impl RoutingTable {
    /// Create new [`RoutingTable`].
    pub fn new(local_key: Key<RouterId>) -> Self {
        RoutingTable {
            local_key,
            buckets: (0..NUM_BUCKETS).map(|_| KBucket::new()).collect(),
        }
    }

    /// Get index into a k-bucket using `key`.
    ///
    /// Returns `None` if `key` is the local router's ID.
    fn bucket_index(&self, key: &Key<RouterId>) -> Option<BucketIndex> {
        match BucketIndex::new(&self.local_key.distance(&key)) {
            Some(index) => Some(index),
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    "tried to add local router to routing table",
                );
                None
            }
        }
    }

    /// Add router to [`RoutingTable`].
    pub fn add_router(&mut self, router_id: RouterId) {
        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            "add router",
        );
        let key = Key::from(router_id.clone());

        if let Some(index) = self.bucket_index(&key) {
            if !self.buckets[*index].try_insert(key) {
                tracing::trace!(
                    target: LOG_TARGET,
                    %router_id,
                    "failed to add floodfill to routing table",
                );
            }
        }
    }

    /// Adjust the score of a floodfill.
    pub fn adjust_score(&mut self, router_id: &RouterId, adjustment: isize) {
        let key = Key::from(router_id.clone());

        if let Some(index) = BucketIndex::new(&self.local_key.distance(&key)) {
            self.buckets[*index].adjust_score(key, adjustment);
        }
    }

    /// Get `limit` many floodfills closest to `target` from the k-buckets.
    pub fn closest<'a, K: Clone + 'a>(
        &'a mut self,
        target: Key<K>,
        limit: usize,
    ) -> impl Iterator<Item = RouterId> + 'a {
        ClosestBucketsIter::new(self.local_key.distance(&target))
            .flat_map(move |index| self.buckets[*index].closest_iter(&target))
            .take(limit)
    }

    /// Get `limit` many floodfills closest to `target` from the k-buckets, ignoring routers
    /// specified in `ignore`.
    pub fn closest_with_ignore<'a, 'b: 'a, K: Clone + 'a>(
        &'a self,
        target: Key<K>,
        limit: usize,
        ignore: &'b HashSet<RouterId>,
    ) -> impl Iterator<Item = RouterId> + 'a {
        ClosestBucketsIter::new(self.local_key.distance(&target))
            .flat_map(move |index| self.buckets[*index].closest_iter(&target))
            .filter(|router_id| !ignore.contains(router_id))
            .take(limit)
    }
}

/// An iterator over the bucket indices, in the order determined by the `Distance` of a target from
/// the `local_key`, such that the entries in the buckets are incrementally further away from the
/// target, starting with the bucket covering the target.
/// The original implementation is taken from `rust-libp2p`, see [issue#1117][1] for the explanation
/// of the algorithm used.
///
///  [1]: https://github.com/libp2p/rust-libp2p/pull/1117#issuecomment-494694635
struct ClosestBucketsIter {
    /// The distance to the `local_key`.
    distance: Distance,

    /// The current state of the iterator.
    state: ClosestBucketsIterState,
}

/// Operating states of a `ClosestBucketsIter`.
enum ClosestBucketsIterState {
    /// The starting state of the iterator yields the first bucket index and
    /// then transitions to `ZoomIn`.
    Start(BucketIndex),

    /// The iterator "zooms in" to to yield the next bucket cotaining nodes that
    /// are incrementally closer to the local node but further from the `target`.
    /// These buckets are identified by a `1` in the corresponding bit position
    /// of the distance bit string. When bucket `0` is reached, the iterator
    /// transitions to `ZoomOut`.
    ZoomIn(BucketIndex),

    /// Once bucket `0` has been reached, the iterator starts "zooming out"
    /// to buckets containing nodes that are incrementally further away from
    /// both the local key and the target. These are identified by a `0` in
    /// the corresponding bit position of the distance bit string. When bucket
    /// `255` is reached, the iterator transitions to state `Done`.
    ZoomOut(BucketIndex),

    /// The iterator is in this state once it has visited all buckets.
    Done,
}

impl ClosestBucketsIter {
    fn new(distance: Distance) -> Self {
        let state = match BucketIndex::new(&distance) {
            Some(i) => ClosestBucketsIterState::Start(i),
            None => ClosestBucketsIterState::Start(BucketIndex(0)),
        };
        Self { distance, state }
    }

    fn next_in(&self, i: BucketIndex) -> Option<BucketIndex> {
        (0..*i).rev().find_map(|i| self.distance.0.bit(i).then_some(BucketIndex(i)))
    }

    fn next_out(&self, i: BucketIndex) -> Option<BucketIndex> {
        (*i + 1..NUM_BUCKETS).find_map(|i| (!self.distance.0.bit(i)).then_some(BucketIndex(i)))
    }
}

impl Iterator for ClosestBucketsIter {
    type Item = BucketIndex;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            ClosestBucketsIterState::Start(i) => {
                self.state = ClosestBucketsIterState::ZoomIn(i);
                Some(i)
            }
            ClosestBucketsIterState::ZoomIn(i) =>
                if let Some(i) = self.next_in(i) {
                    self.state = ClosestBucketsIterState::ZoomIn(i);
                    Some(i)
                } else {
                    let i = BucketIndex(0);
                    self.state = ClosestBucketsIterState::ZoomOut(i);
                    Some(i)
                },
            ClosestBucketsIterState::ZoomOut(i) =>
                if let Some(i) = self.next_out(i) {
                    self.state = ClosestBucketsIterState::ZoomOut(i);
                    Some(i)
                } else {
                    self.state = ClosestBucketsIterState::Done;
                    None
                },
            ClosestBucketsIterState::Done => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netdb::types::U256;

    #[test]
    fn closest_routers() {
        let own_router_id = RouterId::random();
        let own_key = Key::from(own_router_id);
        let mut table = RoutingTable::new(own_key.clone());

        for _ in 0..60 {
            let router_id = RouterId::random();
            table.add_router(router_id);
        }

        let target = Key::from(RouterId::random());
        let closest = table.closest(target.clone(), 60usize);
        let mut prev = None;

        for router_id in closest {
            if let Some(value) = prev {
                assert!(value < target.distance(&Key::from(router_id.clone())));
            }

            prev = Some(target.distance(&Key::from(router_id)));
        }
    }

    #[test]
    fn cannot_add_own_router_id() {
        let own_router_id = RouterId::random();
        let own_key = Key::from(own_router_id);
        let table = RoutingTable::new(own_key.clone());

        assert!(table.bucket_index(&own_key).is_none());
    }

    #[test]
    fn closest_buckets_iterator_set_lsb() {
        // Test zooming-in & zooming-out of the iterator using a toy example with set LSB.
        let d = Distance(U256::from(0b10011011));
        let mut iter = ClosestBucketsIter::new(d);
        // Note that bucket 0 is visited twice. This is, technically, a bug, but to not
        // complicate the implementation and keep it consistent with `libp2p` it's kept as is.
        // There are virtually no practical consequences of this, because to have bucket 0
        // populated we have to encounter two sha256 hash values differing only in one least
        // significant bit.
        let expected_buckets = vec![7, 4, 3, 1, 0, 0, 2, 5, 6]
            .into_iter()
            .chain(8..=255)
            .map(|i| BucketIndex(i));
        for expected in expected_buckets {
            let got = iter.next().unwrap();
            assert_eq!(got, expected);
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn closest_buckets_iterator_unset_lsb() {
        // Test zooming-in & zooming-out of the iterator using a toy example with unset LSB.
        let d = Distance(U256::from(0b01011010));
        let mut iter = ClosestBucketsIter::new(d);
        let expected_buckets =
            vec![6, 4, 3, 1, 0, 2, 5, 7].into_iter().chain(8..=255).map(|i| BucketIndex(i));
        for expected in expected_buckets {
            let got = iter.next().unwrap();
            assert_eq!(got, expected);
        }
        assert!(iter.next().is_none());
    }
}
