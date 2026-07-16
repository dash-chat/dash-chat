mod edit;
mod message;
mod validation;
pub use message::*;
pub use validation::*;

use crate::Topic;

pub type ChatId = Topic<crate::topic::kind::Chat>;
pub type GroupChatId = ChatId;
pub type DirectChatId = ChatId;
pub type DeviceGroupId = Topic<crate::topic::kind::DeviceGroup>;
// pub type GroupChatId = Topic<crate::topic::kind::GroupChat>;
// pub type DirectChatId = Topic<crate::topic::kind::DirectChat>;
