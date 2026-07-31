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
    crypto::{chachapoly::ChaChaPoly, EphemeralPrivateKey, StaticPublicKey},
    error::QueryError,
    i2np::{
        database::lookup::{DatabaseLookupBuilder, LookupType, ReplyType},
        garlic::{DeliveryInstructions, GarlicMessageBuilder, GARLIC_MESSAGE_OVERHEAD},
        MessageBuilder, MessageType, I2NP_MESSAGE_EXPIRATION,
    },
    netdb::Dht,
    primitives::{Lease, LeaseSet2, MessageId, RouterId, TunnelId},
    profile::ProfileStorage,
    router::context::RouterContext,
    runtime::{Instant, Runtime},
};

use bytes::{BufMut, Bytes, BytesMut};
use futures_channel::oneshot;
use hashbrown::HashSet;
use rand::Rng;

use alloc::{vec, vec::Vec};
use core::{fmt, time::Duration};

/// Total query timeout.
const QUERY_TOTAL_TIMEOUT: Duration = Duration::from_secs(20);

/// Tunnel selector.
///
/// Distributes tunnel usage fairly across all tunnels.
pub struct TunnelSelector<T: Clone> {
    /// Iterator index.
    iterator: usize,

    /// Tunnels.
    tunnels: Vec<T>,
}

impl<T: Clone> TunnelSelector<T> {
    /// Create new [`TunnelSelector`].
    pub fn new() -> Self {
        Self {
            iterator: 0usize,
            tunnels: Vec::new(),
        }
    }

    /// Add `tunnel` into [`TunnelSelector`].
    pub fn add_tunnel(&mut self, tunnel: T) {
        self.tunnels.push(tunnel);
    }

    /// Get tunnel count.
    pub fn len(&self) -> usize {
        self.tunnels.len()
    }

    /// Remove tunnel from [`TunnelSelector`] using predicate.
    pub fn remove_tunnel(&mut self, predicate: impl Fn(&T) -> bool) {
        self.tunnels.retain(|tunnel| predicate(tunnel))
    }

    /// Get next tunnel from [`TunnelSelector`], if any exists.
    pub fn next_tunnel(&mut self) -> Option<T> {
        if self.tunnels.is_empty() {
            return None;
        }

        let index = {
            let index = self.iterator;
            self.iterator = self.iterator.wrapping_add(1usize);

            index
        };

        Some(self.tunnels[index % self.tunnels.len()].clone())
    }
}

/// Query, either [`LeaseSet2`] or [`RouterInfo`].
pub struct Query<R: Runtime, T> {
    /// Lookup key.
    pub key: Bytes,

    /// New pending routers discovered via `DatabaseSearchReply` messages.
    ///
    /// RI lookup is pending for these routers before a lease set query can be sent.
    pub pending: HashSet<RouterId>,

    /// Queried (or pending) floodfills.
    ///
    /// This is used to prevent quering the same floodfills twice during recursive queries.
    pub queried: HashSet<RouterId>,

    /// New queryable routers discovered via `DatabaseSearchReply` messages.
    pub queryable: HashSet<RouterId>,

    /// Selected floofill.
    pub selected: Option<RouterId>,

    /// When was the query started.
    pub started: R::Instant,

    /// Oneshot senders for sending the result to caller.
    pub subscribers: Vec<oneshot::Sender<Result<T, QueryError>>>,
}

impl<R: Runtime, T: Clone> Query<R, T> {
    /// Create new [`Query`].
    pub fn new(key: Bytes, tx: oneshot::Sender<Result<T, QueryError>>, selected: RouterId) -> Self {
        Self {
            key,
            pending: HashSet::new(),
            queried: HashSet::from_iter([selected.clone()]),
            queryable: HashSet::new(),
            selected: Some(selected),
            started: R::now(),
            subscribers: vec![tx],
        }
    }

    /// Handle `DatabaseSearchReply`.
    ///
    /// This function is called when a `DatabaseSearchReply` is received for an active query,
    /// meaning database lookup failed.
    ///
    /// The reply may contain routers that are closer to the search key so filter out all queried
    /// routers, mark all routers whose router infos we already have as queryable and mark rest of
    /// routers as pending and return their router IDs so their router infos can be downloaded.
    pub fn handle_search_reply(
        &mut self,
        routers: &[RouterId],
        profile_storage: &ProfileStorage<R>,
    ) -> Vec<RouterId> {
        // database search reply was received so there is selected router right now
        //
        // the next lookup will be sent (and router will be selected) when the timer expires
        self.selected = None;

        routers
            .iter()
            .filter_map(|router_id| {
                // router already queried
                if self.queried.contains(router_id) {
                    return None;
                }

                // router not yet queried but its router info is available
                if profile_storage.contains(router_id) {
                    self.queryable.insert(router_id.clone());
                    return None;
                }

                // non-queried router and router info not available, download it
                self.pending.insert(router_id.clone());

                Some(router_id.clone())
            })
            .collect()
    }

    /// Handle `DatabaseLookUp` timeout.
    pub fn handle_timeout(
        &mut self,
        dht: &Dht<R>,
        profile_storage: &ProfileStorage<R>,
    ) -> Result<RouterId, QueryError> {
        if self.started.elapsed() >= QUERY_TOTAL_TIMEOUT {
            return Err(QueryError::Timeout);
        }

        // move all queried routers to queryable
        self.pending.retain(|router_id| {
            if profile_storage.contains(router_id) {
                self.queryable.insert(router_id.clone());
                return false;
            }

            true
        });

        // attempt to select next floodfill
        //
        // if new floodfills were found with previous searches, attempt to select a floodfill from
        // them that's closest to the search key and if we haven't received any "floodfill replies",
        // attempt to select a floodfill from the dht
        match Dht::<R>::get_closest(&self.key, &self.queryable) {
            Some(floodfill) => {
                self.queryable.remove(&floodfill);
                Ok(floodfill)
            }
            None => dht
                .closest_with_ignore(&self.key, 1usize, &self.queried)
                .next()
                .ok_or(QueryError::NoFloodfills),
        }
    }

    /// Add new subscriber for the query.
    pub fn add_subscriber(&mut self, tx: oneshot::Sender<Result<T, QueryError>>) {
        self.subscribers.push(tx);
    }

    /// Complete query by sending the result to caller(s).
    pub fn complete(self, value: Result<T, QueryError>) {
        self.subscribers.into_iter().for_each(|tx| {
            let _ = tx.send(value.clone());
        });
    }
}

/// Query kind.
pub enum QueryKind<R: Runtime> {
    /// Lease set query.
    LeaseSet {
        /// Active query.
        query: Query<R, LeaseSet2>,
    },

    /// Router info.
    RouterInfo {
        /// Active query.
        query: Query<R, ()>,
    },

    /// Router exploration.
    Exploration,

    /// Router info lookup.
    Router,
}

impl<R: Runtime> fmt::Debug for QueryKind<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeaseSet { .. } => f.debug_struct("QueryKind::LeaseSet").finish_non_exhaustive(),
            Self::RouterInfo { .. } =>
                f.debug_struct("QueryKind::RouterInfo").finish_non_exhaustive(),
            Self::Exploration => f.debug_struct("QueryKind::Exploration").finish(),
            Self::Router => f.debug_struct("QueryKind::Router").finish(),
        }
    }
}

/// Message sender.
pub struct NetDbMessageBuilder<R: Runtime> {
    /// Active inbound tunhnels
    pub outbound_tunnels: TunnelSelector<TunnelId>,

    /// Active inbound tunnels.
    pub inbound_tunnels: TunnelSelector<Lease>,

    /// Router context.
    router_ctx: RouterContext<R>,
}

impl<R: Runtime> NetDbMessageBuilder<R> {
    pub fn new(router_ctx: RouterContext<R>) -> Self {
        Self {
            outbound_tunnels: TunnelSelector::new(),
            inbound_tunnels: TunnelSelector::new(),
            router_ctx,
        }
    }

    /// Create [`DatabaseLookup`] message for a lease set identified by `key` and garlic-encrypt it
    /// with `static_key`.
    ///
    /// The function fails with `QueryError::NoTunnel` if there is are no inbound or outbound
    /// tunnels.
    pub fn create_lease_set_query(
        &mut self,
        key: Bytes,
        static_key: StaticPublicKey,
    ) -> Result<(Vec<u8>, TunnelId), QueryError> {
        let outbound_tunnel = self.outbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;
        let Lease {
            router_id,
            tunnel_id,
            ..
        } = self.inbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;

        let message = DatabaseLookupBuilder::new(key.clone(), LookupType::LeaseSet)
            .with_reply_type(ReplyType::Tunnel {
                tunnel_id,
                router_id,
            })
            .build();

        let mut message = GarlicMessageBuilder::default()
            .with_date_time(R::time_since_epoch().as_secs() as u32)
            .with_garlic_clove(
                MessageType::DatabaseLookup,
                MessageId::from(R::rng().next_u32()),
                R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                DeliveryInstructions::Local,
                &message,
            )
            .build();

        let ephemeral_secret = EphemeralPrivateKey::random(R::rng());
        let ephemeral_public = ephemeral_secret.public();
        let (garlic_key, garlic_tag) =
            self.router_ctx.noise().derive_outbound_garlic_key(static_key, ephemeral_secret);

        // message length + poly13055 tg + ephemeral key + garlic message length
        let mut out = BytesMut::with_capacity(message.len() + GARLIC_MESSAGE_OVERHEAD);

        // encryption must succeed since the parameters are managed by us
        ChaChaPoly::new(&garlic_key)
            .encrypt_with_ad_new(&garlic_tag, &mut message)
            .expect("to succeed");

        out.put_u32(message.len() as u32 + 32);
        out.put_slice(&ephemeral_public.to_vec());
        out.put_slice(&message);

        Ok((
            MessageBuilder::standard()
                .with_expiration(R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION)
                .with_message_type(MessageType::Garlic)
                .with_message_id(R::rng().next_u32())
                .with_payload(&out)
                .build(),
            outbound_tunnel,
        ))
    }

    /// Create [`DatabaseLookup`] message for a router info identified by `key`.
    ///
    /// On success, returns a serialized [`DatabaseLookup`] message and ID of the selected outbound
    /// tunnel.
    ///
    /// The function fails with `QueryError::NoTunnel` if there is are no inbound or outbound
    /// tunnels.
    pub fn create_router_info_query(
        &mut self,
        key: Bytes,
    ) -> Result<(Vec<u8>, TunnelId), QueryError> {
        let outbound_tunnel = self.outbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;
        let Lease {
            router_id,
            tunnel_id,
            ..
        } = self.inbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;

        Ok((
            MessageBuilder::standard()
                .with_expiration(R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION)
                .with_message_type(MessageType::DatabaseLookup)
                .with_message_id(R::rng().next_u32())
                .with_payload(
                    &DatabaseLookupBuilder::new(key, LookupType::Router)
                        .with_reply_type(ReplyType::Tunnel {
                            tunnel_id,
                            router_id,
                        })
                        .build(),
                )
                .build(),
            outbound_tunnel,
        ))
    }

    /// Create [`DatabaseLookup`] message for router exploration.
    ///
    /// On success, returns a serialized [`DatabaseLookup`] message and ID of the selected outbound
    /// tunnel.
    ///
    /// The function fails with `QueryError::NoTunnel` if there is are no inbound or outbound
    /// tunnels.
    pub fn create_router_exploration(
        &mut self,
        key: Bytes,
    ) -> Result<(Vec<u8>, TunnelId), QueryError> {
        let outbound_tunnel = self.outbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;
        let Lease {
            router_id,
            tunnel_id,
            ..
        } = self.inbound_tunnels.next_tunnel().ok_or(QueryError::NoTunnel)?;

        Ok((
            MessageBuilder::standard()
                .with_expiration(R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION)
                .with_message_type(MessageType::DatabaseLookup)
                .with_message_id(R::rng().next_u32())
                .with_payload(
                    &DatabaseLookupBuilder::new(key, LookupType::Exploration)
                        .with_reply_type(ReplyType::Tunnel {
                            tunnel_id,
                            router_id,
                        })
                        .build(),
                )
                .build(),
            outbound_tunnel,
        ))
    }
}
