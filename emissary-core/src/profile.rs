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
    crypto::{base64_decode, base64_encode},
    primitives::{RouterId, RouterInfo},
    runtime::{Instant, Runtime, Storage},
};

use bytes::Bytes;
use hashbrown::{HashMap, HashSet};

#[cfg(feature = "std")]
use parking_lot::{RwLock, RwLockReadGuard};
#[cfg(feature = "no_std")]
use spin::rwlock::{RwLock, RwLockReadGuard};

use alloc::{string::String, sync::Arc, vec::Vec};
use core::{marker::PhantomData, time::Duration};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::profile";

/// Last decline threshold.
///
/// TODO: explain
const LAST_DECLINE_THRESHOLD: Duration = Duration::from_secs(180);

/// How long the router is considered unreachable after last dial failure.
const UNREACHABILITY_THRESHOLD: Duration = Duration::from_secs(180);

/// How often [`ProfileManager`] sorts profiles.
const PROFILE_STORAGE_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(60);

/// Profile storage backup interval.
///
/// How often is a backup taken of [`ProfileStorage`].
const PROFILE_STORAGE_BACKUP_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How many routers does the high capacity bucket hold.
const NUM_HIGH_CAPACITY_ROUTERS: usize = 200usize;

/// How many routers does the standard bucket hold.
const NUM_STANDARD_ROUTERS: usize = 400usize;

/// How many routers does the untracked bucket hold.
const NUM_UNTRACKED_ROUTERS: usize = 2000usize;

/// Router bucket.
pub enum Bucket {
    /// Any bucket.
    Any,

    /// Fast bucket.
    Fast,

    /// Standard bucket.
    Standard,

    /// Untracked bucket.
    Untracked,
}

/// Router profile.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Is there an active connection to the router.
    pub is_connected: bool,

    /// Last activity, duration since UNIX epoch.
    pub last_activity: Duration,

    /// Last time a tunnel was declined.
    ///
    /// `None` if there is no information.
    pub last_declined: Option<Duration>,

    /// Last time a dial failed.
    ///
    /// `None` if there is no information.
    pub last_dial_failure: Option<Duration>,

    /// Number of accepted tunnels.
    pub num_accepted: usize,

    /// Number of successful connections.
    pub num_connection: usize,

    /// Number of dial failures.
    pub num_dial_failures: usize,

    /// How many [`DatabaseSearchReply`]s have been received.
    pub num_lookup_failures: usize,

    /// How many [`DatabaseLookup`]s have gone unaswered.
    pub num_lookup_no_responses: usize,

    /// How many [`DatabaseStore`]s have been received.
    pub num_lookup_successes: usize,

    /// Number of rejected tunnels.
    pub num_rejected: usize,

    /// Number of times the router has been selecte for a tunnel.
    pub num_selected: usize,

    /// Number of test failures for tunnels where the router was a selected hop.
    pub num_test_failures: usize,

    /// Number of test successes for tunnels where the router was a selected hop.
    pub num_test_successes: usize,

    /// Number of tunnel build request timeouts where this router was a selected hop.
    pub num_unaswered: usize,
}

impl Profile {
    /// Create new [`Profile`].
    fn new() -> Self {
        Self {
            last_activity: Duration::from_secs(0),
            last_declined: None,
            is_connected: false,
            last_dial_failure: None,
            num_accepted: 0usize,
            num_connection: 0usize,
            num_dial_failures: 0usize,
            num_lookup_failures: 0usize,
            num_lookup_no_responses: 0usize,
            num_lookup_successes: 0usize,
            num_rejected: 0usize,
            num_selected: 0usize,
            num_test_failures: 0usize,
            num_test_successes: 0usize,
            num_unaswered: 0usize,
        }
    }

    /// Create new [`Profile`] with `last_activity` set to now.
    fn new_with_activity<R: Runtime>() -> Self {
        Self {
            last_activity: R::time_since_epoch(),
            is_connected: false,
            last_declined: None,
            last_dial_failure: None,
            num_accepted: 0usize,
            num_connection: 0usize,
            num_dial_failures: 0usize,
            num_lookup_failures: 0usize,
            num_lookup_no_responses: 0usize,
            num_lookup_successes: 0usize,
            num_rejected: 0usize,
            num_selected: 0usize,
            num_test_failures: 0usize,
            num_test_successes: 0usize,
            num_unaswered: 0usize,
        }
    }

    /// Is the router considered always inactive.
    ///
    /// The router is considered always inactive if it's at least 30 minutes old and has had no
    /// recorded activity.
    fn is_always_inactive(&self) -> bool {
        if self.last_activity < Duration::from_secs(30 * 60) {
            return false;
        }

        self.last_declined.is_none()
            && self.last_dial_failure.is_none()
            && self.num_accepted == 0usize
            && self.num_connection == 0usize
            && self.num_dial_failures == 0usize
            && self.num_lookup_failures == 0usize
            && self.num_lookup_no_responses == 0usize
            && self.num_lookup_successes == 0usize
            && self.num_rejected == 0usize
            && self.num_selected == 0usize
            && self.num_test_failures == 0usize
            && self.num_test_successes == 0usize
            && self.num_unaswered == 0usize
    }

    /// Is the router considered always unreachable.
    ///
    /// If the router has had more than 5 dial failures with no successes, the router is considered
    /// always unreachable.
    fn is_always_unreachable(&self) -> bool {
        self.num_dial_failures > 5 && self.num_connection == 0
    }

    /// Is the router considered "recently inactive".
    ///
    /// If the router hasn't had any activity in the last 2 hours, it's considered recently
    /// inactive.
    ///
    /// This is different from `is_always_inactive()` as the router may have had activity at some
    /// point in time but hasn't any activity in the last 2 hours and is not actively connected.
    fn is_recently_inactive(&self) -> bool {
        self.last_activity > Duration::from_secs(2 * 60 * 60) && !self.is_connected
    }

    /// Has the router recently declined a tunnel.
    ///
    /// Decline is either an actual declination or a failure to respond to a request.
    fn has_recently_declined<R: Runtime>(&self) -> bool {
        self.last_declined.map_or_else(
            || false,
            |last_declined| {
                R::time_since_epoch()
                    .checked_sub(last_declined)
                    .is_some_and(|elapsed| elapsed < LAST_DECLINE_THRESHOLD)
            },
        )
    }

    /// Does the router have low participation rate.
    fn has_low_participation_rate(&self) -> bool {
        4 * self.num_accepted < self.num_rejected
    }

    /// Calculate participation rate for the router.
    fn participation_rate(&self) -> Option<f64> {
        if self.num_accepted + self.num_rejected + self.num_unaswered == 0 {
            return None;
        }

        Some(
            self.num_accepted as f64
                / ((self.num_accepted + self.num_rejected + self.num_unaswered) as f64),
        )
    }

    /// Calculate weighted participation rate for the router.
    fn weighted_participation_rate(&self, avg: f64) -> f64 {
        (self.num_accepted as f64 + 10f64 * avg)
            / ((self.num_accepted + self.num_rejected + self.num_unaswered + 10) as f64)
    }

    /// Is the router considered unreachable.
    fn is_unreachable<R: Runtime>(&self) -> bool {
        self.last_dial_failure.map_or_else(
            || false,
            |last_dial_failure| {
                R::time_since_epoch()
                    .checked_sub(last_dial_failure)
                    .is_some_and(|elapsed| elapsed < UNREACHABILITY_THRESHOLD)
            },
        )
    }

    /// Is the router always declining tunnels.
    fn is_always_declining(&self) -> bool {
        self.num_accepted == 0 && self.num_rejected >= 5
    }

    /// Is the router considered failing.
    pub fn is_failing<R: Runtime>(&self) -> bool {
        self.has_recently_declined::<R>()
            || self.is_unreachable::<R>()
            || self.is_always_declining()
            || self.has_low_participation_rate()
    }

    /// Calculate floodfill score from the profile.
    pub fn floodfill_score(&self) -> isize {
        self.num_lookup_failures as isize
            + (self.num_lookup_no_responses as isize * -5isize)
            + (self.num_lookup_successes as isize * 10isize)
    }
}

/// Router info/profile reader.
pub struct Reader<'a> {
    /// Read access to router infos.
    router_infos: RwLockReadGuard<'a, HashMap<RouterId, RouterInfo>>,

    /// Read access to serialized router infos.
    raw_router_infos: RwLockReadGuard<'a, HashMap<RouterId, Vec<u8>>>,

    /// Read access to profiles.
    profiles: RwLockReadGuard<'a, HashMap<RouterId, Profile>>,
}

impl Reader<'_> {
    /// Get reference to router's router info.
    pub fn router_info(&self, router_id: &RouterId) -> Option<&RouterInfo> {
        self.router_infos.get(router_id)
    }

    /// Get a copy of serialized router info.
    pub fn raw_router_info(&self, router_id: &RouterId) -> Option<Vec<u8>> {
        self.raw_router_infos.get(router_id).cloned()
    }

    /// Get reference to router's profile.
    pub fn profile(&self, router_id: &RouterId) -> Option<&Profile> {
        self.profiles.get(router_id)
    }
}

/// Profile storage.
#[derive(Clone)]
pub struct ProfileStorage<R: Runtime> {
    /// Discovered routers.
    discovered_routers: Arc<RwLock<HashMap<RouterId, Vec<u8>>>>,

    /// Fast routers.
    fast: Arc<RwLock<HashSet<RouterId>>>,

    /// Router profiles.
    profiles: Arc<RwLock<HashMap<RouterId, Profile>>>,

    /// Raw router infos.
    //
    // TODO: store as `Bytes`
    raw_router_infos: Arc<RwLock<HashMap<RouterId, Vec<u8>>>>,

    /// Router infos.
    routers: Arc<RwLock<HashMap<RouterId, RouterInfo>>>,

    /// Standard routers.
    standard: Arc<RwLock<HashSet<RouterId>>>,

    /// Untracked routers.
    untracked: Arc<RwLock<HashSet<RouterId>>>,

    /// Object providing storage access, if provided.
    storage: Option<Arc<dyn Storage>>,

    /// Marker for `Runtime`.
    _runtime: PhantomData<R>,
}

impl<R: Runtime> ProfileStorage<R> {
    /// Create new [`ProfileStorage`].
    pub fn new(
        routers: &[Vec<u8>],
        profiles: &[(String, Profile)],
        storage: Option<Arc<dyn Storage>>,
    ) -> Self {
        tracing::info!(
            target: LOG_TARGET,
            num_routers = ?routers.len(),
            num_profiles = ?profiles.len(),
            "initializing profile storage",
        );

        // TODO: not good
        let (mut routers, mut raw_router_infos): (HashMap<_, _>, HashMap<_, _>) = routers
            .iter()
            .filter_map(|router| {
                RouterInfo::parse::<R>(router)
                    .map(|parsed| {
                        let router_id = parsed.identity.id();

                        ((router_id.clone(), parsed), (router_id, router.clone()))
                    })
                    .ok()
            })
            .unzip();

        let mut profiles = profiles
            .iter()
            .filter_map(|(router_id, profile)| {
                let router_id =
                    RouterId::from(base64_decode(router_id).expect("valid base64 name"));

                routers.contains_key(&router_id).then_some((router_id, *profile))
            })
            .collect::<HashMap<_, _>>();

        // empty profiles for all routers whose profiles were not found
        routers.keys().for_each(|router_id| {
            if !profiles.contains_key(router_id) {
                profiles.insert(router_id.clone(), Profile::new());
            }
        });

        // split router infos into fast and standard buckets and filter out unusable routers
        let (fast, standard): (Vec<_>, Vec<_>) = routers
            .iter()
            .filter_map(|(router_id, router_info)| {
                if !router_info.is_reachable() || !router_info.capabilities.is_usable() {
                    return None;
                }

                match router_info.capabilities.is_fast() {
                    true => Some((Some(router_id.clone()), None)),
                    false => Some((None, Some(router_id.clone()))),
                }
            })
            .unzip();

        // split routers into fast and untracked buckets
        let (fast, untracked) = {
            let (total, routers, untracked) = fast.iter().flatten().fold(
                (0f64, HashSet::<RouterId>::new(), HashSet::<RouterId>::new()),
                |(mut total, mut fast, mut untracked), router_id| {
                    match profiles.get(router_id).expect("to exist").participation_rate() {
                        Some(rate) => {
                            total += rate;
                            fast.insert(router_id.clone());
                        }
                        None => {
                            untracked.insert(router_id.clone());
                        }
                    }

                    (total, fast, untracked)
                },
            );

            if routers.is_empty() {
                (HashSet::new(), untracked)
            } else {
                let avg = total / routers.len() as f64;
                let mut routers = routers
                    .into_iter()
                    .map(|router_id| {
                        // profile must exist since the router's participation rate was calculated
                        let rate = profiles
                            .get(&router_id)
                            .expect("to exist")
                            .weighted_participation_rate(avg);

                        (router_id, rate)
                    })
                    .collect::<Vec<_>>();

                routers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                let fast = routers
                    .iter()
                    .take(NUM_HIGH_CAPACITY_ROUTERS)
                    .map(|(router_id, _)| router_id.clone())
                    .collect::<HashSet<_>>();
                let untracked = routers
                    .into_iter()
                    .filter_map(|(router_id, _)| (!fast.contains(&router_id)).then_some(router_id))
                    .chain(untracked)
                    .collect();

                (fast, untracked)
            }
        };

        let standard = standard.into_iter().flatten().chain(untracked).collect::<HashSet<_>>();

        // split routers into standard and untracked buckets
        let (standard, untracked) = {
            let (total, routers, untracked) = standard.iter().fold(
                (0f64, HashSet::<RouterId>::new(), HashSet::<RouterId>::new()),
                |(mut total, mut routers, mut untracked), router_id| {
                    match profiles.get(router_id).expect("to exist").participation_rate() {
                        Some(rate) => {
                            total += rate;
                            routers.insert(router_id.clone());
                        }
                        None => {
                            untracked.insert(router_id.clone());
                        }
                    }

                    (total, routers, untracked)
                },
            );

            if routers.is_empty() {
                (HashSet::new(), untracked)
            } else {
                let avg = total / routers.len() as f64;
                let mut routers = routers
                    .into_iter()
                    .map(|router_id| {
                        // profile must exist since the router's participation rate was calculated
                        let rate = profiles
                            .get(&router_id)
                            .expect("to exist")
                            .weighted_participation_rate(avg);

                        (router_id, rate)
                    })
                    .collect::<Vec<_>>();

                routers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                let standard = routers
                    .iter()
                    .take(NUM_STANDARD_ROUTERS)
                    .map(|(router_id, _)| router_id.clone())
                    .collect::<HashSet<_>>();
                let untracked = routers
                    .into_iter()
                    .filter_map(|(router_id, _)| {
                        (!standard.contains(&router_id)).then_some(router_id)
                    })
                    .chain(untracked)
                    .collect();

                (standard, untracked)
            }
        };

        // cull the untracked bucket down to 2000 routers
        let untracked = {
            let untracked = untracked.into_iter().collect::<Vec<_>>();
            let mut total = 0f64;
            let tracked = untracked
                .iter()
                .cloned()
                .filter_map(|router_id| {
                    profiles.get(&router_id).expect("to exist").participation_rate().map(|rate| {
                        total += rate;
                        router_id
                    })
                })
                .collect::<Vec<_>>();

            if tracked.is_empty() {
                untracked.into_iter().take(NUM_UNTRACKED_ROUTERS).collect::<HashSet<_>>()
            } else {
                let avg = total / tracked.len() as f64;
                let mut tracked = tracked
                    .into_iter()
                    .map(|router_id| {
                        // profile must exist since the router's participation rate was calculated
                        let rate = profiles
                            .get(&router_id)
                            .expect("to exist")
                            .weighted_participation_rate(avg);

                        (router_id, rate)
                    })
                    .collect::<Vec<_>>();

                tracked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                tracked
                    .iter()
                    .take(NUM_UNTRACKED_ROUTERS)
                    .map(|(router_id, _)| router_id.clone())
                    .collect::<HashSet<_>>()
            }
        };

        // remove all routers that were not included in any of the buckets
        {
            let router_ids = routers
                .iter()
                .flat_map(|(router_id, _)| {
                    (!fast.contains(router_id)
                        && !standard.contains(router_id)
                        && !untracked.contains(router_id))
                    .then(|| router_id.clone())
                })
                .collect::<Vec<_>>();

            let router_ids = router_ids
                .into_iter()
                .map(|router_id| {
                    routers.remove(&router_id);
                    raw_router_infos.remove(&router_id);
                    profiles.remove(&router_id);

                    base64_encode(router_id.to_vec())
                })
                .collect::<Vec<_>>();

            if let Some(ref storage) = storage {
                storage.remove_from_disk(router_ids);
            }
        }

        tracing::info!(
            target: LOG_TARGET,
            num_fast = fast.len(),
            num_standard = standard.len(),
            num_untracked = untracked.len(),
            "profile storage initialized"
        );

        let profile_storage = Self {
            discovered_routers: Default::default(),
            fast: Arc::new(RwLock::new(fast)),
            profiles: Arc::new(RwLock::new(profiles)),
            raw_router_infos: Arc::new(RwLock::new(raw_router_infos)),
            routers: Arc::new(RwLock::new(routers)),
            standard: Arc::new(RwLock::new(standard)),
            storage,
            untracked: Arc::new(RwLock::new(untracked)),
            _runtime: Default::default(),
        };

        R::spawn(ProfileManager::<R>::new(profile_storage.clone()).run());

        profile_storage
    }

    /// Insert `router` into [`ProfileStorage`].
    pub fn add_router(&self, router_info: RouterInfo) -> bool {
        let router_id = router_info.identity.id();

        {
            let mut fast = self.fast.write();
            let mut standard = self.standard.write();

            if router_info.capabilities.is_fast() {
                fast.insert(router_id.clone());
                standard.remove(&router_id);
            } else {
                standard.insert(router_id.clone());
                fast.remove(&router_id);
            }
        }

        if self.routers.write().insert(router_id.clone(), router_info).is_none() {
            self.profiles.write().insert(router_id, Profile::new_with_activity::<R>());
        }

        true
    }

    /// Register [`RouterInfo`] discovered via `NetDb` queries or direct `DatabaseStore` messages.
    pub fn discover_router(&self, router_info: RouterInfo, serialized: Bytes) -> bool {
        let router_id = router_info.identity.id();

        // if the router was accepted to profile storage, store the serialized router info
        // which is used to make a backup of the router
        if self.add_router(router_info) {
            let serialized = serialized.to_vec();

            self.raw_router_infos.write().insert(router_id.clone(), serialized.clone());
            self.discovered_routers.write().insert(router_id, serialized);

            return true;
        }

        false
    }

    /// Get the number of routers currently stored in [`ProfileStorage`].
    pub fn num_routers(&self) -> usize {
        self.routers.read().len()
    }

    // TODO: remove
    // TODO: why?
    pub fn get(&self, router: &RouterId) -> Option<RouterInfo> {
        self.routers.read().get(router).cloned()
    }

    /// Get raw router info of `router_id`.
    pub fn get_raw(&self, router_id: &RouterId) -> Option<Vec<u8>> {
        self.raw_router_infos.read().get(router_id).cloned()
    }

    /// Check if [`ProfileStorage`] contains `router_id`.
    pub fn contains(&self, router_id: &RouterId) -> bool {
        self.routers.read().contains_key(router_id)
    }

    /// Get `RouterId`s of those routers that pass `filter`.
    pub fn get_router_ids(
        &self,
        bucket: Bucket,
        filter: impl Fn(&RouterId, &RouterInfo, &Profile) -> bool,
    ) -> Vec<RouterId> {
        let routers = self.routers.read();
        let profiles = self.profiles.read();

        match bucket {
            Bucket::Any => {
                let fast = self.fast.read();
                let standard = self.standard.read();
                let untracked = self.untracked.read();

                fast.iter()
                    .chain(standard.iter())
                    .chain(untracked.iter())
                    .filter_map(|router_id| {
                        // profile & router info must exist since they're managed by us
                        let profile = profiles.get(router_id).expect("to exist");
                        let router_info = routers.get(router_id).expect("to exist");

                        filter(router_id, router_info, profile).then_some(router_id.clone())
                    })
                    .collect()
            }
            Bucket::Untracked => {
                let untracked = self.untracked.read();

                untracked
                    .iter()
                    .filter_map(|router_id| {
                        // profile & router info must exist since they're managed by us
                        let profile = profiles.get(router_id).expect("to exist");
                        let router_info = routers.get(router_id).expect("to exist");

                        filter(router_id, router_info, profile).then_some(router_id.clone())
                    })
                    .collect()
            }
            Bucket::Fast => {
                let fast = self.fast.read();

                fast.iter()
                    .filter_map(|router_id| {
                        // profile & router info must exist since they're managed by us
                        let profile = profiles.get(router_id).expect("to exist");
                        let router_info = routers.get(router_id).expect("to exist");

                        filter(router_id, router_info, profile).then_some(router_id.clone())
                    })
                    .collect()
            }
            Bucket::Standard => {
                let standard = self.standard.read();

                standard
                    .iter()
                    .filter_map(|router_id| {
                        // profile & router info must exist since they're managed by us
                        let profile = profiles.get(router_id).expect("to exist");
                        let router_info = routers.get(router_id).expect("to exist");

                        filter(router_id, router_info, profile).then_some(router_id.clone())
                    })
                    .collect()
            }
        }
    }

    /// Get [`Reader`].
    pub fn reader(&self) -> Reader<'_> {
        Reader {
            router_infos: self.routers.read(),
            raw_router_infos: self.raw_router_infos.read(),
            profiles: self.profiles.read(),
        }
    }

    /// Returns `true` if router identified by `RouterId` is a floodfill router.
    ///
    /// Returns `false` if it's not or if the router is not found in [`ProfileManager`].
    pub fn is_floodfill(&self, router_id: &RouterId) -> bool {
        self.routers
            .read()
            .get(router_id)
            .is_some_and(|router_info| router_info.is_floodfill())
    }

    /// Record that `router_id` was selected for a tunnel.
    pub fn selected_for_tunnel(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_selected += 1;
        profile.last_activity = R::time_since_epoch();
    }

    /// Record that `router_id`'s participation for a tunnel could not be determined.
    ///
    /// This happens when a build record fails to decrypt, causing the entire build response to be
    /// unparseable and hops following the malformed hop cannot be decrypted and parsed.
    pub fn unselected_for_tunnel(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_selected = profile.num_selected.saturating_sub(1);
    }

    /// Record that `router_id` accepted a tunnel build request.
    pub fn tunnel_accepted(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_accepted += 1;
        profile.last_activity = R::time_since_epoch();
        profile.last_declined = None;
    }

    /// Record that `router_id` rejected a tunnel build request.
    pub fn tunnel_rejected(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_rejected += 1;
        profile.last_activity = R::time_since_epoch();
        profile.last_declined = Some(R::time_since_epoch());
    }

    /// Record that `router_id` failed to answer a tunnel build request.
    pub fn tunnel_not_answered(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_unaswered += 1;
        profile.last_activity = R::time_since_epoch();
        profile.last_declined = Some(R::time_since_epoch());
    }

    /// Record test success for a tunnel that `router_id` was a participant of.
    pub fn tunnel_test_succeeded(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_test_successes += 1;
        profile.last_activity = R::time_since_epoch();
    }

    /// Record test failure for a tunnel that `router_id` was a participant of.
    pub fn tunnel_test_failed(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        // profile must exist since it's controlled by us
        let profile = inner.get_mut(router_id).expect("to exist");

        profile.num_test_failures += 1;
        profile.last_activity = R::time_since_epoch();
    }

    /// Record dial success for `router_id`.
    ///
    /// Profile might not exist if this is an inbound connection.
    pub fn dial_succeeded(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        match inner.get_mut(router_id) {
            Some(profile) => {
                profile.num_connection += 1;
                profile.is_connected = true;
                profile.last_activity = R::time_since_epoch();
            }
            None => {
                let mut profile = Profile::new();
                profile.is_connected = true;
                profile.num_connection += 1;
                profile.last_activity = R::time_since_epoch();

                inner.insert(router_id.clone(), profile);
            }
        }
    }

    /// Connection to the router has been closed.
    pub fn connection_closed(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        if let Some(profile) = inner.get_mut(router_id) {
            profile.is_connected = false
        }
    }

    /// Record dial failure for `router_id`.
    ///
    /// Profile might not exist if this is an inbound connection.
    pub fn dial_failed(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        match inner.get_mut(router_id) {
            Some(profile) => {
                profile.num_dial_failures += 1;
                profile.last_activity = R::time_since_epoch();
                profile.last_dial_failure = Some(profile.last_activity);
            }
            None => {
                let mut profile = Profile::new();
                profile.num_dial_failures += 1;
                profile.last_activity = R::time_since_epoch();
                profile.last_dial_failure = Some(profile.last_activity);

                inner.insert(router_id.clone(), profile);
            }
        }
    }

    /// Record a non-respone to a lease set/router info query.
    pub fn database_lookup_no_response(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        if let Some(profile) = inner.get_mut(router_id) {
            profile.num_lookup_no_responses += 1;
        }
    }

    /// Record non-respones to a lease set/router info query.
    pub fn database_lookup_success(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        if let Some(profile) = inner.get_mut(router_id) {
            profile.num_lookup_successes += 1;
        }
    }

    /// Record non-respones to a lease set/router
    pub fn database_lookup_failure(&self, router_id: &RouterId) {
        let mut inner = self.profiles.write();

        if let Some(profile) = inner.get_mut(router_id) {
            profile.num_lookup_failures += 1;
        }
    }

    /// Create new [`ProfileStorage`] from random `routers`.
    ///
    /// Only used in tests.
    #[cfg(test)]
    pub fn from_random(routers: Vec<RouterInfo>) -> Self {
        let routers = routers
            .into_iter()
            .map(|router| (router.identity.id(), router))
            .collect::<HashMap<_, _>>();

        let profiles =
            routers.keys().map(|router_id| (router_id.clone(), Profile::new())).collect();

        // split router infos into fast and standard buckets and filter out unusable routers
        let (fast, standard): (Vec<_>, Vec<_>) = routers
            .iter()
            .filter_map(|(router_id, router_info)| {
                if !router_info.is_reachable() || !router_info.capabilities.is_usable() {
                    return None;
                }

                match router_info.capabilities.is_fast() {
                    true => Some((Some(router_id.clone()), None)),
                    false => Some((None, Some(router_id.clone()))),
                }
            })
            .unzip();

        Self {
            discovered_routers: Default::default(),
            fast: Arc::new(RwLock::new(fast.into_iter().flatten().collect())),
            profiles: Arc::new(RwLock::new(profiles)),
            raw_router_infos: Default::default(),
            routers: Arc::new(RwLock::new(routers)),
            standard: Arc::new(RwLock::new(standard.into_iter().flatten().collect())),
            storage: None,
            untracked: Default::default(),
            _runtime: Default::default(),
        }
    }
}

/// Profile manager.
struct ProfileManager<R: Runtime> {
    /// Profile storage.
    profile_storage: ProfileStorage<R>,
}

impl<R: Runtime> ProfileManager<R> {
    /// Create new [`ProfileManager`].
    fn new(profile_storage: ProfileStorage<R>) -> Self {
        Self { profile_storage }
    }

    /// Run the event loop of profile manager.
    async fn run(self) {
        let mut last_backup = R::now();

        loop {
            R::delay(PROFILE_STORAGE_MAINTENANCE_INTERVAL).await;

            let profiles = self.profile_storage.profiles.read();

            let (total, routers, no_profile_routers) = {
                let fast = self.profile_storage.fast.read();
                let standard = self.profile_storage.standard.read();
                let untracked = self.profile_storage.untracked.read();

                fast.iter().chain(standard.iter()).chain(untracked.iter()).fold(
                    (0f64, HashSet::<RouterId>::new(), HashSet::<RouterId>::new()),
                    |(mut total, mut routers, mut untracked), router_id| {
                        match profiles.get(router_id).expect("to exist").participation_rate() {
                            Some(rate) => {
                                total += rate;
                                routers.insert(router_id.clone());
                            }
                            None => {
                                untracked.insert(router_id.clone());
                            }
                        }
                        (total, routers, untracked)
                    },
                )
            };

            // if there are no statistics yet, leave the groups unmodified
            if routers.is_empty() {
                continue;
            }

            // calculate weighted capacity for each router
            let avg = total / routers.len() as f64;
            let mut routers = routers
                .into_iter()
                .map(|router_id| {
                    // profile must exist since the router's participation rate was calculated
                    let rate = profiles
                        .get(&router_id)
                        .expect("to exist")
                        .weighted_participation_rate(avg);

                    (router_id, rate)
                })
                .collect::<Vec<_>>();

            // sort by capacity in descending order
            routers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // split routers into fast, standard and untracked buckets
            let router_infos = self.profile_storage.routers.read();

            let mut fast = HashSet::<RouterId>::new();
            let mut standard = HashSet::<RouterId>::new();
            let mut untracked = HashSet::<RouterId>::new();
            let mut removed = HashSet::<RouterId>::new();

            for (router_id, _) in routers {
                let Some(router_info) = router_infos.get(&router_id) else {
                    removed.insert(router_id);
                    continue;
                };

                if !router_info.is_reachable() {
                    removed.insert(router_id);
                    continue;
                }

                if router_info.capabilities.is_fast() && fast.len() < NUM_HIGH_CAPACITY_ROUTERS {
                    fast.insert(router_id);
                    continue;
                }

                if standard.len() < NUM_STANDARD_ROUTERS {
                    standard.insert(router_id);
                    continue;
                }

                untracked.insert(router_id);
            }

            untracked.extend(no_profile_routers);

            // remove unusable routers
            if let Some(ref storage) = self.profile_storage.storage {
                storage.remove_from_disk(
                    removed
                        .into_iter()
                        .map(|router_id| base64_encode(router_id.to_vec()))
                        .collect::<Vec<_>>(),
                );
            }

            // replace old groups with new groups
            {
                *self.profile_storage.fast.write() = fast;
                *self.profile_storage.standard.write() = standard;
                *self.profile_storage.untracked.write() = untracked;
            }
            drop(profiles);
            drop(router_infos);

            if last_backup.elapsed() <= PROFILE_STORAGE_BACKUP_INTERVAL {
                continue;
            }

            // cull the set of untracked routers to `NUM_UNTRACKED_ROUTERS`
            let router_ids = {
                let inner = self.profile_storage.untracked.read();
                let profiles = self.profile_storage.profiles.read();

                if inner.len() <= NUM_UNTRACKED_ROUTERS {
                    Vec::new()
                } else {
                    let mut always_inactive = Vec::new();
                    let mut always_unreachable = Vec::new();
                    let mut recently_inactive = Vec::new();

                    // first remove always inactive routers
                    inner.iter().for_each(|router_id| {
                        if profiles
                            .get(router_id)
                            .is_some_and(|profile| profile.is_always_inactive())
                        {
                            always_inactive.push(router_id.clone());
                        }
                    });

                    // if necessary, remove always unreachable routers
                    if inner.len() - always_inactive.len() > NUM_UNTRACKED_ROUTERS {
                        inner.iter().for_each(|router_id| {
                            if profiles
                                .get(router_id)
                                .is_some_and(|profile| profile.is_always_unreachable())
                            {
                                always_unreachable.push(router_id.clone());
                            }
                        })
                    }

                    if inner.len() - always_inactive.len() - always_unreachable.len()
                        > NUM_UNTRACKED_ROUTERS
                    {
                        inner.iter().for_each(|router_id| {
                            if profiles
                                .get(router_id)
                                .is_some_and(|profile| profile.is_recently_inactive())
                            {
                                recently_inactive.push(router_id.clone());
                            }
                        })
                    }

                    always_inactive
                        .into_iter()
                        .chain(always_unreachable)
                        .chain(recently_inactive)
                        .collect()
                }
            };

            if let Some(ref storage) = self.profile_storage.storage {
                if !router_ids.is_empty() {
                    // purge all collections from the removed routers
                    let router_ids = {
                        let mut removed = Vec::new();
                        let mut fast = self.profile_storage.fast.write();
                        let mut standard = self.profile_storage.standard.write();
                        let mut untracked = self.profile_storage.untracked.write();
                        let mut profiles = self.profile_storage.profiles.write();
                        let mut raw_router_infos = self.profile_storage.raw_router_infos.write();
                        let mut routers = self.profile_storage.routers.write();
                        let mut discovered = self.profile_storage.discovered_routers.write();

                        for router_id in router_ids {
                            if untracked.len() <= NUM_UNTRACKED_ROUTERS {
                                break;
                            }

                            let Some(profile) = profiles.get(&router_id) else {
                                continue;
                            };

                            let should_remove = !profile.is_connected
                                && (profile.is_always_inactive()
                                    || profile.is_always_unreachable()
                                    || profile.is_recently_inactive());

                            if !should_remove || !untracked.contains(&router_id) {
                                continue;
                            }

                            fast.remove(&router_id);
                            standard.remove(&router_id);
                            untracked.remove(&router_id);
                            profiles.remove(&router_id);
                            raw_router_infos.remove(&router_id);
                            routers.remove(&router_id);
                            discovered.remove(&router_id);
                            removed.push(base64_encode(router_id.to_vec()));
                        }

                        removed
                    };

                    storage.remove_from_disk(router_ids);
                }

                tracing::info!(
                    target: LOG_TARGET,
                    num_fast = %self.profile_storage.fast.read().len(),
                    num_standard = %self.profile_storage.standard.read().len(),
                    num_untracked = %self.profile_storage.untracked.read().len(),
                    "profile storage updated",
                );

                let profiles = self.profile_storage.profiles.read().clone();
                let mut inner = self.profile_storage.discovered_routers.write();

                let routers = profiles
                    .into_iter()
                    .map(|(router_id, profile)| {
                        (
                            base64_encode(router_id.to_vec()),
                            inner.remove(&router_id),
                            profile,
                        )
                    })
                    .collect::<Vec<_>>();

                if !routers.is_empty() {
                    tracing::info!(
                        target: LOG_TARGET,
                        num_routers = ?routers.len(),
                        "taking backup of profile storage",
                    );

                    storage.save_to_disk(routers);
                }
            }

            last_backup = R::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crypto::base64_encode, primitives::RouterInfoBuilder, runtime::mock::MockRuntime};

    #[tokio::test]
    async fn initialize_with_infos_without_profiles() {
        let (_, infos): (Vec<_>, Vec<_>) = (0..5)
            .map(|_| {
                let (info, _, sgn_key) = RouterInfoBuilder::default().build();
                let router_id = info.identity.id();

                (router_id, info.serialize(&sgn_key))
            })
            .unzip();

        let profiles = ProfileStorage::<MockRuntime>::new(&infos, &Vec::new(), None);

        assert_eq!(profiles.routers.read().len(), 5);
        assert_eq!(profiles.profiles.read().len(), 5);
        assert!(profiles
            .routers
            .read()
            .keys()
            .all(|key| profiles.profiles.read().contains_key(key)));
        assert!(profiles.profiles.read().values().all(|profile| profile == &Profile::new()));
    }

    #[tokio::test]
    async fn initialize_with_infos_and_profiles() {
        let (router_ids, infos): (Vec<_>, Vec<_>) = (0..5)
            .map(|_| {
                let (info, _, sgn_key) = RouterInfoBuilder::default().build();
                let router_id = info.identity.id();

                (router_id, info.serialize(&sgn_key))
            })
            .unzip();

        let profiles = (0..3)
            .map(|i| {
                let router_id = base64_encode(router_ids[i].to_vec());

                (
                    router_id,
                    Profile {
                        is_connected: false,
                        last_activity: Duration::from_secs((i as u64 + 1) * 10000),
                        last_declined: None,
                        last_dial_failure: None,
                        num_accepted: i + 1,
                        num_connection: i + 1,
                        num_dial_failures: i + 1,
                        num_lookup_failures: i + 1,
                        num_lookup_no_responses: i + 1,
                        num_lookup_successes: i + 1,
                        num_rejected: i + 1,
                        num_selected: i + 1,
                        num_test_failures: i + 1,
                        num_test_successes: i + 1,
                        num_unaswered: i + 1,
                    },
                )
            })
            .collect::<Vec<_>>();

        let profiles = ProfileStorage::<MockRuntime>::new(&infos, &profiles, None);

        assert_eq!(profiles.routers.read().len(), 5);
        assert_eq!(profiles.profiles.read().len(), 5);
        assert!(profiles
            .routers
            .read()
            .keys()
            .all(|key| profiles.profiles.read().contains_key(key)));

        for i in 0..3 {
            assert_ne!(
                profiles.profiles.read().get(&router_ids[i]).unwrap(),
                &Profile::new()
            );
        }

        for i in 3..5 {
            assert_eq!(
                profiles.profiles.read().get(&router_ids[i]).unwrap(),
                &Profile::new()
            );
        }
    }

    #[tokio::test]
    async fn profile_without_router_info() {
        let profiles = (0..3)
            .map(|i| {
                let router_id = base64_encode(RouterId::random().to_vec());

                (
                    router_id,
                    Profile {
                        is_connected: false,
                        last_activity: Duration::from_secs((i as u64 + 1) * 10000),
                        last_declined: None,
                        last_dial_failure: None,
                        num_accepted: i + 1,
                        num_connection: i + 1,
                        num_dial_failures: i + 1,
                        num_lookup_failures: i + 1,
                        num_lookup_no_responses: i + 1,
                        num_lookup_successes: i + 1,
                        num_rejected: i + 1,
                        num_selected: i + 1,
                        num_test_failures: i + 1,
                        num_test_successes: i + 1,
                        num_unaswered: i + 1,
                    },
                )
            })
            .collect::<Vec<_>>();

        let profiles = ProfileStorage::<MockRuntime>::new(&Vec::new(), &profiles, None);

        assert!(profiles.routers.read().is_empty());
        assert!(profiles.profiles.read().is_empty());
    }

    #[tokio::test]
    async fn create_profile_if_it_doesnt_exist() {
        let profiles = ProfileStorage::<MockRuntime>::new(&Vec::new(), &Vec::new(), None);
        let router_id = RouterId::random();

        assert!(profiles.routers.read().is_empty());
        assert!(profiles.profiles.read().is_empty());

        profiles.dial_succeeded(&router_id);

        let reader = profiles.reader();
        assert_eq!(
            reader.profiles.get(&router_id).unwrap().num_connection,
            1usize
        );
        assert!(reader.profiles.get(&router_id).unwrap().is_connected);
    }
}
