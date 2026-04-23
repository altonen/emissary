// Copyright 2018-2019 Parity Technologies (UK) Ltd.
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

//! Kademlia k-bucket implementation.

use crate::{
    netdb::types::{FloodFill, Key},
    primitives::RouterId,
};

use alloc::vec::Vec;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::netdb::k-bucket";

/// Kademlia k-bucket.
pub struct KBucket {
    /// Floodfill routers of the bucket.
    floodfills: Vec<FloodFill>,
}

impl KBucket {
    /// Create new [`KBucket`].
    pub fn new() -> Self {
        Self {
            floodfills: Vec::with_capacity(20),
        }
    }

    /// Try to insert `key` into [`KBucket`].
    ///
    /// Returns `true` if `key` already exists or if it was successfully inserted into [`KBucket`].
    /// If the k-bucket is full, its searched for the lowest performing floodfill and if their score
    /// is below the insertion threshold (0), the floodfill is evicted and `key` is inserted in
    /// their place.
    ///
    /// Returns `false` if `key` could not be inserted into [`KBucket`].
    pub fn try_insert(&mut self, key: Key<RouterId>) -> bool {
        if self.floodfills.iter().any(|floodfill| floodfill.key == key) {
            return true;
        }

        if self.floodfills.len() < 20 {
            self.floodfills.push(FloodFill::new(key.preimage().clone()));
            return true;
        }

        if let Some(floodfill) = self.floodfills.iter_mut().min() {
            if floodfill.score < 0 {
                tracing::trace!(
                    target: LOG_TARGET,
                    old_floodfill = %floodfill.key.preimage(),
                    score = %floodfill.score,
                    new_floodfill = %key.preimage(),
                    "evicting floodfill",
                );

                floodfill.key = key;
                floodfill.score = 0;

                return true;
            }
        }

        false
    }

    /// Adjust score of a floodfill.
    pub fn adjust_score(&mut self, key: Key<RouterId>, adjustment: isize) {
        match self.floodfills.iter_mut().find(|floodfill| floodfill.key == key) {
            Some(floodfill) => {
                tracing::trace!(
                    target: LOG_TARGET,
                    score = %(floodfill.score + adjustment),
                    "router score adjusted",
                );
                floodfill.score += adjustment;
            }
            None => tracing::debug!(
                target: LOG_TARGET,
                router_id = %key.preimage(),
                "cannot adjust score, router doesn't exist in the k-bucket",
            ),
        }
    }

    /// Get iterator over the k-bucket, sorting the k-bucket entries in increasing order
    /// by distance.
    pub fn closest_iter<K: Clone>(&self, target: &Key<K>) -> impl Iterator<Item = RouterId> {
        let mut floodfills = self.floodfills.clone();

        floodfills.sort_by_key(|a| target.distance(&a.key));
        floodfills.into_iter().map(|router| router.key.preimage().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_iter() {
        let mut bucket = KBucket::new();

        // add some random nodes to the bucket
        let _ = (0..10)
            .map(|_| {
                let peer = RouterId::random();
                bucket.floodfills.push(FloodFill::new(RouterId::random()));

                peer
            })
            .collect::<Vec<_>>();

        let target = Key::from(RouterId::random());
        let mut iter = bucket.closest_iter(&target);
        let mut prev = None;

        while let Some(router_id) = iter.next() {
            if let Some(distance) = prev {
                assert!(distance < target.distance(&Key::from(router_id.clone())));
            }

            prev = Some(target.distance(&Key::from(router_id)));
        }
    }

    #[test]
    fn floodfill_with_low_score_evicted() {
        let mut bucket = KBucket::new();

        let floodfills = (0..20)
            .map(|_| {
                let peer = RouterId::random();
                bucket.floodfills.push(FloodFill::new(peer.clone()));

                peer
            })
            .collect::<Vec<_>>();

        // try to add new floodfill router to k-bucket
        let router_id = RouterId::random();
        let key = Key::from(router_id.clone());
        assert!(!bucket.try_insert(key.clone()));

        // decrease the score of one of the floodfills
        bucket.adjust_score(Key::from(floodfills[0].clone()), -10);

        // try to insert the router again and verify it succeeds
        assert!(bucket.try_insert(key));

        // ensure the first floodfill is no longer found and that all scores are equal
        assert!(!bucket
            .floodfills
            .iter()
            .any(|floodfill| floodfill.key == Key::from(floodfills[0].clone())));
        assert!(bucket.floodfills.iter().all(|floodfill| floodfill.score == 0));
    }
}
