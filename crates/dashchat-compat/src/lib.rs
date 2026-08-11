//! DEPRECATED. This whole concept needs to be reworked!
//!
//! The Capability system is too aggressive.
//! There's no hard requirement that nodes with different capabilities need
//! to downgrade to the lowest common denominator.
//! For instance, if node A supports a media type and node B doesn't,
//! node A's wire messages serialization should gracefully handle both cases,
//! and B should be able to deserialize A's message, throwing away the unknown fields.

use std::fmt;

#[cfg(test)]
mod util;

pub type CapabilityVersion = u16;

pub trait Capability: Ord + Clone + Copy + std::fmt::Debug + Send + Sync + 'static {}
impl<C> Capability for C where C: Ord + Clone + Copy + std::fmt::Debug + Send + Sync + 'static {}

/// Generates a capabilities struct with typed fields and infimum negotiation.
///
/// The default value for each field is the version number given in the macro.
/// `infimum` takes the min of each paired field, treating a missing peer capability as 0.
/// `infimum_opt` is the same but treats `None` as "no constraint" (returns self).
#[macro_export]
macro_rules! capabilities {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident {
            $($field:ident: $version:expr),* $(,)?
        }
    ) => {
        $(#[$attr])*
        $vis struct $name {
            $(pub $field: $crate::CapabilityVersion,)*
        }

        #[allow(unused)]
        impl $name {
            pub fn zero() -> Self {
                Self {
                    $($field: 0,)*
                }
            }

            pub fn current() -> Self {
                Self {
                    $($field: $version,)*
                }
            }

            pub fn infimum(&self, other: &Self) -> Self {
                Self {
                    $($field: self.$field.min(other.$field),)*
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {

    capabilities! {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct TestCapSet {
            messaging: 3,
            gossip_protocol: 1,
        }
    }

    #[test]
    fn capabilities_macro_current() {
        let caps = TestCapSet::current();
        assert_eq!(caps.messaging, 3);
        assert_eq!(caps.gossip_protocol, 1);
    }

    #[test]
    fn capabilities_macro_infimum() {
        let a = TestCapSet {
            messaging: 3,
            gossip_protocol: 1,
        };
        let b = TestCapSet {
            messaging: 2,
            gossip_protocol: 1,
        };
        let inf = a.infimum(&b);
        assert_eq!(inf.messaging, 2);
        assert_eq!(inf.gossip_protocol, 1);
        // commutative
        assert_eq!(b.infimum(&a), inf);
    }

    #[test]
    fn capabilities_infimum() {
        let caps1 = TestCapSet {
            messaging: 1,
            gossip_protocol: 4,
        };
        let caps2 = TestCapSet {
            messaging: 2,
            gossip_protocol: 3,
        };
        let expected = TestCapSet {
            messaging: 1,
            gossip_protocol: 3,
        };
        assert_eq!(caps1.infimum(&caps2), expected);
        assert_eq!(caps2.infimum(&caps1), expected);
    }
}
