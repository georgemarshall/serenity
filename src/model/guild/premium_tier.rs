use serde_repr::{Serialize_repr, Deserialize_repr};

/// The guild's premium tier, depends on the amount of users boosting the guild currently
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum PremiumTier {
    /// No tier, considered None
    None = 0,
    Tier1 = 1,
    Tier2 = 2,
    Tier3 = 3,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Default for PremiumTier {
    fn default() -> Self {
        PremiumTier::None
    }
}
