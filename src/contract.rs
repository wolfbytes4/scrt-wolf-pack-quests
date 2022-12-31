use cosmwasm_std::{
    entry_point, to_binary, Env, Deps, DepsMut,
    MessageInfo, Response, StdError, StdResult, Addr,
    Binary, Uint128, CosmosMsg
};
use crate::error::ContractError;
use crate::msg::{QuestResponse, ExecuteMsg, InstantiateMsg, QueryMsg, Quest, ContractInfo, QuestMsg, Token, Level};
use crate::state::{config, config_read, State, CONFIG_KEY, LEVEL_KEY, ADMIN_KEY};
use crate::rand::{sha_256};
use secret_toolkit::{
    snip721::{
        batch_transfer_nft_msg, transfer_nft_msg, nft_dossier_query, register_receive_nft_msg,
        set_viewing_key_msg, set_metadata_msg, ViewerInfo, NftDossier, Transfer
    },
    snip20::{ balance_query, transfer_msg, Balance },
    storage:: { Item }
}; 
pub const BLOCK_SIZE: usize = 256;
pub static CONFIG_ITEM: Item<State> = Item::new(CONFIG_KEY);
pub static LEVEL_ITEM: Item<Vec<Level>> = Item::new(LEVEL_KEY);
pub static ADMIN_ITEM: Item<Addr> = Item::new(ADMIN_KEY);

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, StdError> {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);
    let prng_seed_shill: Vec<u8> = sha_256(base64::encode(msg.entropy_shill).as_bytes()).to_vec();
    let shill_viewing_key = base64::encode(&prng_seed_shill);
    // create initial state
    let state = State {
        quests: vec![],
        locked_nfts: vec![],
        owner: info.sender.clone(), 
        viewing_key: Some(viewing_key),
        quest_contract: ContractInfo {
            code_hash: msg.quest_contract.code_hash,
            address: msg.quest_contract.address,
        },
        shill_contract: msg.shill_contract,
        shill_viewing_key: Some(shill_viewing_key), 
        level_cap: msg.level_cap
    };
    // save the contract state
    // config(deps.storage).save(&state)?; 
    CONFIG_ITEM.save(deps.storage, &state)?;
    LEVEL_ITEM.save(deps.storage, &msg.levels)?;
    ADMIN_ITEM.save(deps.storage, &info.sender)?;

    deps.api.debug(&format!("Contract was initialized by {}", info.sender));
    
    Ok(Response::new()
        .add_message(register_receive_nft_msg(
            _env.contract.code_hash,
            Some(true),
            None,
            BLOCK_SIZE,
            state.quest_contract.code_hash.clone(),
            state.quest_contract.address.clone().to_string(),
        )?)
        .add_message(set_viewing_key_msg(
            state.viewing_key.unwrap().to_string(),
            None,
            BLOCK_SIZE,
            state.quest_contract.code_hash,
            state.quest_contract.address.to_string(),
        )?)
        .add_message(set_viewing_key_msg(
            state.shill_viewing_key.unwrap().to_string(),
            None,
            BLOCK_SIZE,
            state.shill_contract.code_hash,
            state.shill_contract.address.to_string(),
        )?)
    )

}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg
) -> Result<Response, ContractError> {
    match msg { 
        ExecuteMsg::StartQuest { quest } => try_start_quest(deps, _env, info, quest), 
        ExecuteMsg::BatchReceiveNft { from, token_ids, msg } => {
            try_batch_receive(deps, _env, &info.sender, &from, token_ids, msg)
        },
        ExecuteMsg::SendNftBack { token_id } => {
            try_send_nft_back(deps, _env, &info.sender, token_id)
        },
        ExecuteMsg::ClaimNfts{ token_ids } => {
            try_claim_nfts(deps, _env, &info.sender, token_ids)
        }
    }
} 

fn try_batch_receive(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    from: &Addr,
    token_ids: Vec<String>,
    msg: Option<Binary>,
) -> Result<Response, ContractError> { 
    deps.api.debug(&format!("Batch received"));
    //return Err(ContractError::CustomError {val: msg.to_string()});  
   if let Some(bin) = msg { 
     let bytes = base64::decode(bin.to_base64()).unwrap(); // you should handle errors
     let qmsg: QuestMsg = serde_json::from_slice(&bytes).unwrap();

     let mut state = CONFIG_ITEM.load(deps.storage)?;
     //config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        let quest = state.quests.iter().find(|&x| x.quest_id == qmsg.quest_id).unwrap();
        let current_time = _env.block.time.seconds();
        if current_time < quest.start_time || current_time > quest.duration_until_join_closed + quest.start_time {
            return Err(ContractError::CustomError {val: "You can't join this quest".to_string()});
        }
        //check if enough wolfs sent
        if (token_ids.len() as i32) != quest.num_of_nfts{ 
            return Err(ContractError::CustomError {val: "You did not send the right amount of wolves for this quest".to_string()});
        }

        //enter wolves in array
        for id in token_ids.iter() {
            let locked_wolf = Token { 
                token_id: id.to_string(),
                owner: from.clone(),
                sender: sender.clone(),
                quest_id: qmsg.quest_id,
                staked_date: Some(current_time)
            };
            state.locked_nfts.push(locked_wolf);
        } 
        CONFIG_ITEM.save(deps.storage, &state)?;
     //})?;
 
   }

    Ok(Response::default())
}

pub fn try_start_quest(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    quest: Quest
) -> Result<Response, ContractError> { 

    let mut state = CONFIG_ITEM.load(deps.storage)?;
    //config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        if info.sender != state.owner{
            return Err(ContractError::Unauthorized {});
        }
        
        if state.quests.iter().any(|i| i.quest_id==quest.quest_id) {
            return Err(ContractError::CustomError {val: "The quest id already exist".to_string()});
        }

        let mut q = quest;
        q.create_date = _env.block.time.seconds();
        state.quests.push(q);
        CONFIG_ITEM.save(deps.storage, &state)?;
    //})?;

    deps.api.debug("quest added");
    Ok(Response::default())
}

pub fn try_send_nft_back(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    token_id: String
) -> Result<Response, ContractError> { 
    let mut nft = Token{ owner: Addr::unchecked(""), quest_id: 0, sender: Addr::unchecked(""), token_id: "".to_string(), staked_date: None};
    let mut contract: Option<String> = None;
    let mut hash: Option<String> = None;

    let mut state = CONFIG_ITEM.load(deps.storage)?;
    //config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        if sender.clone() != state.owner {
            return Err(ContractError::CustomError {val: "You don't have the permissions to execute this command".to_string()});
        }  
        if let Some(pos) = state.locked_nfts.iter().position(|x| x.token_id == token_id) {
            nft = state.locked_nfts.swap_remove(pos);
            hash = Some(state.quest_contract.code_hash.to_string());
            contract = Some(state.quest_contract.address.to_string());
        }
        else{
            return Err(ContractError::CustomError {val: "Token doesn't exist".to_string()});
        }
        
        CONFIG_ITEM.save(deps.storage, &state)?;
    //})?; 
  
    Ok(Response::new()
        .add_message(transfer_nft_msg(
            nft.owner.to_string(),
            nft.token_id.to_string(),
            None,
            None,
            BLOCK_SIZE,
            hash.unwrap().to_string(),
            contract.unwrap().to_string()
        )?)
    )
}

pub fn try_claim_nfts(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    token_ids: Vec<String>
) -> Result<Response, ContractError> { 

    let mut state = CONFIG_ITEM.load(deps.storage)?; 
    let levels = LEVEL_ITEM.load(deps.storage)?;
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
//config(deps.storage).update(|mut state| -> Result<_,ContractError>{
    // Get viewing key for NFTs
    let viewer = Some(ViewerInfo {
        address: _env.contract.address.to_string(),
        viewing_key: state.viewing_key.as_ref().unwrap().to_string(),
    });

    let mut amount_to_send = Uint128::from(0u32);
    //check for bonus and add to amount of shill to be sent
    // Iter through nfts being claimed
    for token_id in token_ids.iter() { 
        if let Some(pos) = state.locked_nfts.iter().position(|x| &x.token_id == token_id && &x.owner == sender) {
            // Remove token from locked nfts and update it's metadata
            let nft = state.locked_nfts.swap_remove(pos); 
            
            let mut meta: NftDossier =  nft_dossier_query(
                deps.querier,
                token_id.to_string(),
                viewer.clone(),
                None,
                BLOCK_SIZE,
                state.quest_contract.code_hash.clone(),
                state.quest_contract.address.to_string(),
            )?;
     
            let pub_attributes = meta.public_metadata.as_ref().unwrap().extension.as_ref().unwrap().attributes.as_ref().unwrap();
            let current_xp_trait = pub_attributes.iter().find(|&x| x.trait_type == Some("xp".to_string())).unwrap();
            let current_lvl_trait = pub_attributes.iter().find(|&x| x.trait_type == Some("lvl".to_string())).unwrap();
            let quest = state.quests.iter().find(|&x| x.quest_id == nft.quest_id).unwrap();

            // Check date if allowed to claim
            let current_time = _env.block.time.seconds();
            if current_time < nft.staked_date.unwrap() + quest.duration_in_staking
            {
                return Err(ContractError::CustomError {val: "You're trying to claim before the staking period is over".to_string()});
            }

            let mut current_xp : i32 = current_xp_trait.value.parse().unwrap();
            let mut current_lvl: i32 = current_lvl_trait.value.parse().unwrap();
            
            let new_xp = current_xp + quest.xp_reward;
            let shouldbe_lvl = if current_lvl < state.level_cap {
                levels.iter().find(|&x| x.xp_needed > current_xp).unwrap().level - 1
                } 
                else { 
                    current_lvl 
                };
            
            current_xp = new_xp;
            current_lvl = shouldbe_lvl;
            
            amount_to_send += quest.shill_reward;
            //TODO check for trait bonus here

            //update pub metadata with new xp and level
            set_metadata_msg(
                token_id.to_string(),
                meta.public_metadata,
                None,
                None,
                BLOCK_SIZE,
                state.quest_contract.code_hash.clone(),
                state.quest_contract.address.to_string()
            )?; 
             
        }
        else{
            return Err(ContractError::CustomError {val: "Token doesn't exist or you are not the owner".to_string()});
        }
        
    }

    //transfer back
    let mut transfers: Vec<Transfer> = Vec::new();
    transfers.push(
        Transfer{
            recipient: sender.to_string(),
            token_ids: token_ids,
            memo: None
        }
    );

    let cosmos_batch_msg = batch_transfer_nft_msg(
        transfers,
        None,
        BLOCK_SIZE,
        state.quest_contract.code_hash.clone(),
        state.quest_contract.address.to_string(),
    )?;
    response_msgs.push(cosmos_batch_msg); 

    //check shill reward here
    if amount_to_send > Uint128::from(0u32) { 
        let amount = amount_to_send;
        let padding = None;
        let block_size = 256;
        let callback_code_hash = state.shill_contract.code_hash.to_string();
        let contract_addr = state.shill_contract.address.to_string();
    
        let cosmos_msg = transfer_msg(
            sender.to_string(),
            amount,
            None,
            padding,
            block_size,
            callback_code_hash,
            contract_addr,
        )?;
    
        response_msgs.push(cosmos_msg);  
    }
        

    
    CONFIG_ITEM.save(deps.storage, &state)?;
    
//})?;


   // Ok(Response::default())
      
    Ok(Response::new().add_messages(response_msgs))
}

 

 

#[entry_point]
pub fn query(
    deps: Deps,
    _env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg { 
        QueryMsg::GetQuests {} => to_binary(&query_quests(deps)?),
        QueryMsg::GetState {admin} => to_binary(&query_state(deps, admin)?),
        QueryMsg::GetShillBalance {admin} => to_binary(&query_shill_balance(deps, admin)?)
    }
}
 
fn query_quests(
    deps: Deps,
) -> StdResult<QuestResponse> {
 
    let state = CONFIG_ITEM.load(deps.storage)?;
    Ok(QuestResponse { quests: state.quests })
}


fn query_state(
    deps: Deps,
    admin: Addr
) -> StdResult<State> {
    let admin_key = ADMIN_ITEM.load(deps.storage)?;  
    
    if admin_key != admin{
        return Err(StdError::generic_err("You can't run this query"));
    }
    
    let state = CONFIG_ITEM.load(deps.storage)?;  

    Ok(state)
}

fn query_shill_balance(
    deps: Deps,
    admin: Addr
) -> StdResult<Balance> {

    let state = CONFIG_ITEM.load(deps.storage)?; 
    let admin_key = ADMIN_ITEM.load(deps.storage)?;  
    if admin_key != admin{
        return Err(StdError::generic_err("You can't run this query"));
    } 

    let address = state.quest_contract.address.to_string();
    let key = state.shill_viewing_key.unwrap();
    let block_size = 256;
    let callback_code_hash = state.shill_contract.code_hash.to_string();
    let contract_addr = state.shill_contract.address.to_string();

    let balance =
        balance_query(deps.querier, state.quest_contract.address.to_string(), key, block_size, callback_code_hash, contract_addr)?;

    Ok(balance)
}


#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};  
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { entropy:"wolfpack".to_string(), quest_contract:{ContractInfo{address:Addr::unchecked("secret174kgn5rtw4kf6f938wm7kwh70h2v4vcfft5mqy"), code_hash:"45f450a4277570f8d1a81eb1185e17ce042a217227dfd836a613c7e54ac15447".to_string()} }};
        let info = mock_info("creator", &coins(1000, "earth"));
       
        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        println!("{} dayknnkjnjknkjn kjnknkjnk nkjnkjs", 31);
 
        assert_eq!(2, res.messages.len());

        // it worked, let's query the state
        
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();
    
        let msg = InstantiateMsg { count: 17, entropy:"wolfpack".to_string(), quest_contract:{ContractInfo{ address:Addr::unchecked("secret174kgn5rtw4kf6f938wm7kwh70h2v4vcfft5mqy"), code_hash:"45f450a4277570f8d1a81eb1185e17ce042a217227dfd836a613c7e54ac15447".to_string()} }};
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    
        // anyone can increment
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    
        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }
    
    #[test]
    fn reset() {
        let mut deps = mock_dependencies();
    
        let msg = InstantiateMsg { count: 17, entropy:"wolfpack".to_string(), quest_contract:{ContractInfo{address:Addr::unchecked("secret174kgn5rtw4kf6f938wm7kwh70h2v4vcfft5mqy"), code_hash:"45f450a4277570f8d1a81eb1185e17ce042a217227dfd836a613c7e54ac15447".to_string()} }};
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    
        // not anyone can reset
        let unauth_env = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_env, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }
    
        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();
    
        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }

    // #[test]
    // fn try_batch_receive() {
    //     let mut deps = mock_dependencies();
    
    //     let msg = BatchReceiveNft { };
    //     let info = mock_info("creator", &coins(2, "token"));
    //     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    
    //     // anyone can increment
    //     let info = mock_info("anyone", &coins(2, "token"));
    //     let msg = ExecuteMsg::BatchReceiveNft {from:"afaf", msg: "eyJxdWVzdF9pZCI6MX0=".to_string(), token_ids: ["4"]};
    //     let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    
    //     // // should increase counter by 1
    //     // let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
    //     // let value: CountResponse = from_binary(&res).unwrap();
    //     // assert_eq!(18, value.count);
    // }
    
}


