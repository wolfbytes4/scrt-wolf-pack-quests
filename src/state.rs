use schemars::JsonSchema;
use serde::{ Deserialize, Serialize};

use cosmwasm_std::{CanonicalAddr, Addr, Storage};
use cosmwasm_storage::{
    singleton, singleton_read, ReadonlySingleton, Singleton,
};
use secret_toolkit::{ 
    storage:: { Item, Keymap, AppendStore },
    snip721:: { ViewerInfo }
};
use crate::msg::{Quest, Token, HistoryToken, ContractInfo, Level};

pub static CONFIG_KEY: &[u8] = b"config";
pub const LEVEL_KEY: &[u8] = b"level";
pub const ADMIN_KEY: &[u8] = b"admin";
pub const MY_ADDRESS_KEY: &[u8] = b"my_address";
pub const ADMIN_VIEWING_KEY: &[u8] = b"admin_viewing_key";
pub const VIEWING_KEY: &[u8] = b"viewing_key";
pub const STAKED_NFTS_KEY: &[u8] = b"staked";
pub const STAKED_NFTS_HISTORY_KEY: &[u8] = b"staked_history";
pub const PREFIX_REVOKED_PERMITS: &str = "revoke";

pub static CONFIG_ITEM: Item<State> = Item::new(CONFIG_KEY);
pub static LEVEL_ITEM: Item<Vec<Level>> = Item::new(LEVEL_KEY);
pub static ADMIN_ITEM: Item<CanonicalAddr> = Item::new(ADMIN_KEY);
pub static ADMIN_VIEWING_KEY_ITEM: Item<ViewerInfo> = Item::new(ADMIN_VIEWING_KEY);
pub static MY_ADDRESS_ITEM: Item<CanonicalAddr> = Item::new(MY_ADDRESS_KEY);
pub static VIEWING_KEY_STORE: Keymap<CanonicalAddr, ViewerInfo> = Keymap::new(VIEWING_KEY);
pub static STAKED_NFTS_STORE: Keymap<CanonicalAddr, Vec<Token>> = Keymap::new(STAKED_NFTS_KEY);
pub static STAKED_NFTS_HISTORY_STORE: AppendStore<HistoryToken> = AppendStore::new(STAKED_NFTS_HISTORY_KEY);

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