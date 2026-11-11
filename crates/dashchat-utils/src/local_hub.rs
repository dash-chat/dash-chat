//! Which of this process's iroh endpoints currently host a local hub, and so
//! accept blob pushes.
//!
//! Process-lifetime by necessity: a hub holds its node's endpoint and blob
//! store across node rebuilds, so any record owned by a node would fall out
//! of sync with the hub it is supposed to describe.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, OnceLock};

static HUB_ENDPOINTS: OnceLock<Mutex<HashSet<iroh::EndpointId>>> = OnceLock::new();

fn endpoints() -> MutexGuard<'static, HashSet<iroh::EndpointId>> {
    // Recover from poisoning rather than propagate it: a panic here would kill
    // the node's push-authorization loop, and unanswered authorizations hang
    // pushes instead of refusing them.
    HUB_ENDPOINTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

/// Mark `id`'s endpoint as hosting a local hub. Balanced by
/// [`deregister_local_hub`] when the hub stops or its server task exits.
pub fn register_local_hub(id: iroh::EndpointId) {
    endpoints().insert(id);
}

/// The hub on `id`'s endpoint stopped.
pub fn deregister_local_hub(id: iroh::EndpointId) {
    endpoints().remove(&id);
}

/// Whether `id`'s endpoint currently hosts a local hub.
pub fn hosts_local_hub(id: iroh::EndpointId) -> bool {
    endpoints().contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_recovers_from_a_poisoned_lock() {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::panic::catch_unwind(|| {
            let _guard = HUB_ENDPOINTS.get_or_init(Default::default).lock().unwrap();
            panic!("poison the registry");
        });
        std::panic::set_hook(hook);

        let id = iroh::SecretKey::generate().public();
        register_local_hub(id);
        assert!(hosts_local_hub(id));
        deregister_local_hub(id);
        assert!(!hosts_local_hub(id));
    }
}
