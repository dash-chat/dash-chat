//! Which of this process's iroh endpoints currently host a local hub, and so
//! accept blob pushes.
//!
//! Shared by handle rather than owned by any node: a hub holds its node's
//! endpoint and blob store across node rebuilds, so a record owned by a node
//! would fall out of sync with the hub it is supposed to describe. The host
//! creates one registry above both and hands a clone to each node build and
//! to each hub it spawns.

use std::sync::Arc;

use dashmap::DashSet;

#[derive(Clone, Debug, Default)]
pub struct LocalHubRegistry {
    /// A `DashSet` rather than a mutex: shard locks don't poison, so a panicking
    /// writer can't wedge the push-authorization loop, whose unanswered
    /// authorizations hang pushes instead of refusing them.
    endpoints: Arc<DashSet<iroh::EndpointId>>,
}

impl LocalHubRegistry {
    /// Mark `id`'s endpoint as hosting a local hub. Balanced by
    /// [`LocalHubRegistry::deregister`] when the hub stops or its server task
    /// exits.
    pub fn register(&self, id: iroh::EndpointId) {
        self.endpoints.insert(id);
    }

    /// The hub on `id`'s endpoint stopped.
    pub fn deregister(&self, id: iroh::EndpointId) {
        self.endpoints.remove(&id);
    }

    /// Whether `id`'s endpoint currently hosts a local hub.
    pub fn hosts(&self, id: iroh::EndpointId) -> bool {
        self.endpoints.contains(&id)
    }
}
