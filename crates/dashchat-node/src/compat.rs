use derive_more::derive::{Deref, From};
use named_id::{AnyNameable, Rename, RenameNone};
use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compat<Bare, Tagged> {
    Unversioned(Bare),
    Versioned(Tagged),
}

impl<Bare: fmt::Debug, Tagged: fmt::Debug> Rename for Compat<Bare, Tagged> {
    fn nameables(&self) -> Vec<AnyNameable<'_>> {
        Vec::new()
    }
}

impl<Bare, Tagged> Serialize for Compat<Bare, Tagged>
where
    Bare: Serialize,
    Tagged: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Compat::Unversioned(bare) => bare.serialize(serializer),
            Compat::Versioned(tagged) => tagged.serialize(serializer),
        }
    }
}

impl<'de, Bare, Tagged> Deserialize<'de> for Compat<Bare, Tagged>
where
    Bare: Deserialize<'de>,
    Tagged: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<B, T> {
            Tagged(T),
            Bare(B),
        }
        match Helper::<Bare, Tagged>::deserialize(deserializer)? {
            Helper::Tagged(t) => Ok(Compat::Versioned(t)),
            Helper::Bare(b) => Ok(Compat::Unversioned(b)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    Messaging,
    SomethingElse,
}

#[derive(Clone, Debug, PartialEq, Eq, Deref, From, Serialize, Deserialize, RenameNone)]
pub struct Capabilities(BTreeMap<Capability, u16>);

impl Capabilities {
    pub fn with_capability(mut self, cap: Capability, version: u16) -> Self {
        self.0.insert(cap, version);
        self
    }

    pub fn infimum(&self, other: &Self) -> Self {
        Self(
            self.0
                .iter()
                .map(|(k, &v)| (k.clone(), v.min(other.0.get(k).copied().unwrap_or(0))))
                .collect(),
        )
    }

    pub fn current() -> Self {
        Self::zero().with_capability(Capability::Messaging, 1)
    }

    pub fn zero() -> Self {
        Self(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionConvertError {
    Lossy,
    UnknownVersion,
}

impl fmt::Display for VersionConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionConvertError::Lossy => write!(f, "lossy version conversion"),
            VersionConvertError::UnknownVersion => write!(f, "unknown version"),
        }
    }
}

impl std::error::Error for VersionConvertError {}

pub trait VersionConvert: Sized {
    const CAPABILITY: Capability;
    fn to_version(&self, target_version: u16) -> Result<Self, VersionConvertError>;
}

#[cfg(test)]
mod test_compat;

#[cfg(test)]
mod test_chat_message_compat;
