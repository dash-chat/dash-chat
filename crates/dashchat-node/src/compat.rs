use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    Messaging,
}

pub type Capabilities = comcap::Capabilities<Capability>;

pub fn capabilities() -> Capabilities {
    Capabilities::zero().with_capability(Capability::Messaging, 1)
}
