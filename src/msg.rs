use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{
   Addr, Binary, Uint128
};
use secret_toolkit::{ 
    snip721:: { ViewerInfo, Trait },
    permit:: { Permit }
};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
      pub entropy: String,
      pub entropy_shill: String,
      pub quest_contract: ContractInfo,
      pub levels: Vec<Level>,
      pub level_cap: i32,
      pub shill_contract: ContractInfo
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Level {
    pub level: i32,
    pub xp_needed: i32
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Quest {
    pub quest_id: i32,
    pub title: String,
    pub description: String,
    pub duration_until_join_closed: u64,
    pub duration_in_staking: u64,
    pub num_of_nfts: i32,
    pub start_time: u64,
    pub create_date: u64,
    pub xp_reward: i32,
    pub shill_reward: Uint128,
    pub shill_trait_bonus_reward: Uint128,
    pub bonus_reward_traits: Vec<Trait>,
    pub wolves_on_the_hunt: i32
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QuestMsg {
    pub quest_id: i32
}
 
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Token {
    pub token_id: String,
    pub owner: Addr,
    pub sender: Addr,
    pub quest_id: i32,
    pub staked_date: Option<u64>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct HistoryToken {
    pub token_id: String,
    pub owner: Addr,
    pub sender: Addr,
    pub quest_id: i32,
    pub staked_date: Option<u64>,
    pub claimed_date: Option<u64>,
    pub reward_amount: Uint128,
    pub xp_reward: i32
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ContractInfo {
    /// contract's code hash string
    pub code_hash: String,
    /// contract's address
    pub address: Addr,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
// pub struct Requirement {
//     pub contract_address: String,
//     pub traits: Vec<Trait>

// }

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
// pub struct Trait {
//     pub trait_category: String,
//     pub trait_value: String
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg { 
    StartQuest{
        quest: Quest
    },
    BatchReceiveNft{
        from: Addr, 
        token_ids: Vec<String>,
        msg: Option<Binary>
    },
    SendNftBack{ 
        token_id: String,
        owner: Addr
    },
    ClaimNfts{ 
        token_ids: Vec<String>
    },
    SetViewingKey{
        key: String
    },
    SendShillBack{
        amount: Uint128,
        address: Addr
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg { 
    GetQuests {},
    GetState {
        viewer: ViewerInfo
    },
    GetUserStakedNfts{ 
        permit: Permit
    },
    GetNumStakedNftKeys{ 
        viewer: ViewerInfo
    },
    GetStakedNfts{
        viewer: ViewerInfo, 
        start_page: u32, 
        page_size: u32 
    }, 
    GetUserStakedNftHistory{
        permit: Permit, 
        start_page: u32, 
        page_size: u32 
    },
    GetNumUserStakedNftHistory{ 
        permit: Permit
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QuestResponse {
    pub quests: Vec<Quest>,
}
