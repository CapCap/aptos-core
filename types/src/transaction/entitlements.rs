use crate::state_store::state_key::StateKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct Entitlements {
    inner: Vec<Entitlement>,
}

impl Entitlements {
    pub fn new(entitlements: Vec<Entitlement>) -> Self {
        Self {
            inner: entitlements,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct Entitlement {
    pub state_key: StateKey,
    pub entitlement_clause: EntitlementClause,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct Between {
    pub lower: u64,
    pub upper: u64,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct MoreThan {
    pub minimum: u64,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct LessThan {
    pub minimum: u64,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct Exactly {
    pub amount: u64,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub enum EntitlementClause {
    Between(Between),
    MoreThan(MoreThan),
    LessThan(LessThan),
    Exactly(Exactly),
}
