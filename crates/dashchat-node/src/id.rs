use derive_more::{Deref, From};
use named_id::RenameAll;
use p2panda_auth::group::GroupMember;
use p2panda_core::PublicKey;
use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize};

/// The ID tied to a particular device.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
    RenameAll,
)]
pub struct DeviceId(PublicKey);

impl DeviceId {
    pub fn to_group_member(self) -> GroupMember<PublicKey> {
        GroupMember::Individual(self.0)
    }
}

/// The ID for an "agent" which may control multiple devices.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Deref,
    RenameAll,
)]
pub struct AgentId(ActorId);

impl AgentId {
    pub fn from_bytes(bytes: &[u8; 32]) -> anyhow::Result<Self> {
        Ok(Self(ActorId::from_bytes(bytes)?))
    }

    pub fn from_pubkey(pubkey: PublicKey) -> Self {
        Self(ActorId::from_bytes(pubkey.as_bytes()).unwrap())
    }

    pub fn to_group_member(self) -> GroupMember<PublicKey> {
        GroupMember::Group(PublicKey::from_bytes(self.0.as_bytes()).unwrap())
    }
}

/// This type is a workaround to a current limitation in p2panda-auth.
/// In dashchat, AgentId represents a device group, which in turn is a member of group chats.
/// Currently it's not possible for a Group to have Manage access in another group.
/// So the workaround is that every time an AgentId joins a groups,
/// the controller of that Agent also joins with a representative DeviceId which can have Manage access.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameAll)]
pub struct GroupRep {
    pub agent_id: AgentId,
    pub representative_device_id: DeviceId,
}

impl GroupRep {
    pub fn to_group_members(&self) -> Vec<GroupMember<PublicKey>> {
        vec![
            self.agent_id.to_group_member(),
            self.representative_device_id.to_group_member(),
        ]
    }
}
