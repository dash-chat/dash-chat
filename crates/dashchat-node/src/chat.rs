mod compat_tests;
mod delete;
mod edit;
mod message;
pub use delete::*;
pub use edit::*;
pub use message::*;

use crate::Topic;

pub type ChatId = Topic<crate::topic::kind::Chat>;
pub type GroupChatId = ChatId;
pub type DirectChatId = ChatId;
pub type DeviceGroupId = Topic<crate::topic::kind::DeviceGroup>;
// pub type GroupChatId = Topic<crate::topic::kind::GroupChat>;
// pub type DirectChatId = Topic<crate::topic::kind::DirectChat>;
