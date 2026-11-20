use p2panda::operation::Header;
use p2panda_auth::group::{GroupAction, GroupMember};
use p2panda_spaces::{ActorId, SpacesArgs};

use crate::DeviceId;

pub type MembershipAction = GroupAction<ActorId, ()>;

/// The group-auth action a space control operation carries, if it is one.
pub fn membership_action(header: &Header) -> Option<MembershipAction> {
    match header.extensions.spaces_args()? {
        SpacesArgs::Group { group_action, .. } => Some(group_action),
        _ => None,
    }
}

/// Whether the action admits `device` to the group: a Create listing it, or an Add of it.
pub fn action_adds_member(action: &MembershipAction, device: DeviceId) -> bool {
    let is_device = |member: &GroupMember<ActorId>| matches!(member, GroupMember::Individual(pk) if DeviceId::from(*pk) == device);
    match action {
        GroupAction::Create { initial_members } => {
            initial_members.iter().any(|(member, _)| is_device(member))
        }
        GroupAction::Add { member, .. } => is_device(member),
        _ => false,
    }
}
