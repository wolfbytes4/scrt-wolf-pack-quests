use std::any::type_name;
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use cosmwasm_std::{Addr, Storage, Binary, StdError, StdResult};
use cosmwasm_storage::{
    singleton, singleton_read, ReadonlySingleton, Singleton,
};
use secret_toolkit::{
    serialization::{Bincode2, Serde}
};
use crate::msg::{Quest, Token, ContractInfo, Level};
use crate::error::ContractError;

pub static CONFIG_KEY: &[u8] = b"config";
pub const LEVEL_KEY: &[u8] = b"level";
pub const ADMIN_KEY: &[u8] = b"admin";
 

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State { 
    pub quests: Vec<Quest>,
    pub locked_nfts: Vec<Token>,
    pub owner: Addr, 
    pub viewing_key: Option<String>,
    pub quest_contract: ContractInfo, 
    pub level_cap: i32,
    pub shill_viewing_key: Option<String>,
    pub shill_contract: ContractInfo
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}