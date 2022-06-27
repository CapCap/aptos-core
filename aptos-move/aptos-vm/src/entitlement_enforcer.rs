use anyhow;
use move_deps::move_core_types::account_address::AccountAddress;
use move_deps::move_core_types::identifier::Identifier;
use move_deps::move_core_types::value::MoveValue;
use move_deps::move_resource_viewer::AnnotatedMoveStruct;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;

#[derive(Clone, Debug, PartialEq)]
pub enum MoveType {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector { items: Box<MoveType> },
    Struct(MoveStructTag),
    GenericTypeParam { index: u16 },
    Reference { mutable: bool, to: Box<MoveType> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoveStructTag {
    pub address: AccountAddress,
    pub module: Identifier,
    pub name: Identifier,
    pub generic_type_params: Vec<MoveType>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveStructValue(BTreeMap<Identifier, serde_json::Value>);

impl TryFrom<AnnotatedMoveStruct> for MoveStructValue {
    type Error = anyhow::Error;

    fn try_from(s: AnnotatedMoveStruct) -> anyhow::Result<Self> {
        let mut map = BTreeMap::new();
        for (id, val) in s.value {
            map.insert(id, MoveValue::try_from(val)?.json()?);
        }
        Ok(Self(map))
    }
}

pub enum ResourceChange {
    Delete(AccountAddress, MoveStructTag),
    Write(AccountAddress, MoveStructTag, MoveStructValue),
}
