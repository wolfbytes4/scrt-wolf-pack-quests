use cosmwasm_std::{
    entry_point, to_binary, Env, Deps, DepsMut,
    MessageInfo, Response, StdError, StdResult, Addr, CanonicalAddr,
    Binary, Uint128, CosmosMsg
};
use crate::error::ContractError;
use crate::msg::{QuestResponse, ExecuteMsg, InstantiateMsg, QueryMsg, Quest, ContractInfo, QuestMsg, Token, HistoryToken };
use crate::state::{ State, ADMIN_VIEWING_KEY_ITEM, VIEWING_KEY_STORE,
    CONFIG_ITEM, LEVEL_ITEM, ADMIN_ITEM, STAKED_NFTS_STORE, STAKED_NFTS_HISTORY_STORE, MY_ADDRESS_ITEM, PREFIX_REVOKED_PERMITS};
use crate::rand::{sha_256};
use secret_toolkit::{
    snip721::{
        batch_transfer_nft_msg, transfer_nft_msg, nft_dossier_query, register_receive_nft_msg,
        set_viewing_key_msg, set_metadata_msg, ViewerInfo, NftDossier, Transfer, Metadata
    },
    permit::{validate, Permit, RevokedPermits},
    snip20::{ transfer_msg }
};  
pub const BLOCK_SIZE: usize = 256;


#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, StdError> {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);

    // create initial state
    let state = State {
        quests: vec![],
        locked_nfts: vec![],
        owner: info.sender.clone(), 
        viewing_key: Some(viewing_key),
        quest_contract: ContractInfo {
            code_hash:  msg.quest_contract.code_hash,
            address: msg.quest_contract.address,
        },
        shill_contract: msg.shill_contract,
        shill_viewing_key: Some(msg.entropy_shill), 
        level_cap: msg.level_cap
    };
   
    //Save Contract state
    CONFIG_ITEM.save(deps.storage, &state)?;
    LEVEL_ITEM.save(deps.storage, &msg.levels)?;
    ADMIN_ITEM.save(deps.storage, &deps.api.addr_canonicalize(&info.sender.to_string())?)?;
    MY_ADDRESS_ITEM.save(deps.storage,  &deps.api.addr_canonicalize(&_env.contract.address.to_string())?)?;

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
        ExecuteMsg::SendNftBack { token_id, owner } => {
            try_send_nft_back(deps, _env, &info.sender, token_id, owner)
        },
        ExecuteMsg::ClaimNfts{ token_ids } => {
            try_claim_nfts(deps, _env, &info.sender, token_ids)
        },
        ExecuteMsg::SetViewingKey { key } => try_set_viewing_key(
            deps,
            _env, 
            &info.sender,
            key
        ), 
        ExecuteMsg::SendShillBack { amount, address } => {
            try_send_shill_back(deps, _env, &info.sender, amount, address)
        },
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
     
   if let Some(bin) = msg { 
     let bytes = base64::decode(bin.to_base64()).unwrap();
     let qmsg: QuestMsg = serde_json::from_slice(&bytes).unwrap();

     let mut staked_nfts: Vec<Token> = STAKED_NFTS_STORE.get(deps.storage, &deps.api.addr_canonicalize(&from.to_string())?).unwrap_or_else(Vec::new);
     let mut state = CONFIG_ITEM.load(deps.storage)?;
     
        let mut quest = state.quests.iter_mut().find(|x| x.quest_id == qmsg.quest_id).unwrap();
        let current_time = _env.block.time.seconds();
        //check if the quest is still on going
        if current_time < quest.start_time || current_time > quest.duration_until_join_closed + quest.start_time {
            return Err(ContractError::CustomError {val: "You can't join this quest".to_string()});
        }

        //check if enough wolfs sent for the quest
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
            
            staked_nfts.push(locked_wolf);
            quest.wolves_on_the_hunt = quest.wolves_on_the_hunt + 1;
        } 

        // save info about nft in the storage and update number of wolves staked to the quest
        STAKED_NFTS_STORE.insert(deps.storage, &deps.api.addr_canonicalize(&from.to_string())?, &staked_nfts)?;
        CONFIG_ITEM.save(deps.storage, &state)?;
 
   }
   else{
    return Err(ContractError::CustomError {val: "Invalid message received".to_string()});
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
    
        if info.sender != state.owner{
            return Err(ContractError::Unauthorized {});
        }
        
        if state.quests.iter().any(|i| i.quest_id==quest.quest_id) {
            return Err(ContractError::CustomError {val: "The quest id already exist".to_string()});
        }

        let mut q = quest;
        q.create_date = _env.block.time.seconds();
        q.wolves_on_the_hunt = 0;
        state.quests.push(q);
        CONFIG_ITEM.save(deps.storage, &state)?;

    deps.api.debug("quest added");
    Ok(Response::default())
}

pub fn try_send_nft_back(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    token_id: String,
    owner: Addr
) -> Result<Response, ContractError> { 
    let mut nft = Token{ owner: Addr::unchecked(""), quest_id: 0, sender: Addr::unchecked(""), token_id: "".to_string(), staked_date: None};
    let mut contract: Option<String> = None;
    let mut hash: Option<String> = None;

    let state = CONFIG_ITEM.load(deps.storage)?;
    let mut staked_nfts: Vec<Token> = STAKED_NFTS_STORE.get(deps.storage,&deps.api.addr_canonicalize(&owner.to_string())?).unwrap_or_else(Vec::new);
    if staked_nfts.len() == 0
    {
        return Err(ContractError::CustomError {val: "This address does not have anything staked".to_string()});
    }

        if sender.clone() != state.owner {
            return Err(ContractError::CustomError {val: "You don't have the permissions to execute this command".to_string()});
        }  
        if let Some(pos) = staked_nfts.iter().position(|x| x.token_id == token_id) {
            nft = staked_nfts.swap_remove(pos);
            hash = Some(state.quest_contract.code_hash.to_string());
            contract = Some(state.quest_contract.address.to_string());
        }
        else{
            return Err(ContractError::CustomError {val: "Token doesn't exist".to_string()});
        }
         
        STAKED_NFTS_STORE.insert(deps.storage, &deps.api.addr_canonicalize(&owner.to_string())?, &staked_nfts)?;
  
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
    let mut staked_nfts: Vec<Token> = STAKED_NFTS_STORE.get(deps.storage, &deps.api.addr_canonicalize(&sender.to_string())?).unwrap_or_else(Vec::new);
    let state = CONFIG_ITEM.load(deps.storage)?; 
    let levels = LEVEL_ITEM.load(deps.storage)?;
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let mut response_attrs = vec![];
    
    // Get viewing key for NFTs
    let viewer = Some(ViewerInfo {
        address: _env.contract.address.to_string(),
        viewing_key: state.viewing_key.as_ref().unwrap().to_string(),
    });

    let mut amount_to_send = Uint128::from(0u32);
    
    //check for bonus and add to amount of shill to be sent
    // Iter through nfts being claimed
    for token_id in token_ids.iter() { 
        let mut has_bonus_trait: bool = false;

        if let Some(pos) = staked_nfts.iter().position(|x| &x.token_id == token_id && &x.owner == sender) {
            // Remove token from locked nfts and update it's metadata
            let nft = staked_nfts.swap_remove(pos); 
            
            let meta: NftDossier =  nft_dossier_query(
                deps.querier,
                token_id.to_string(),
                viewer.clone(),
                None,
                BLOCK_SIZE,
                state.quest_contract.code_hash.clone(),
                state.quest_contract.address.to_string(),
            )?;
     
            let quest = state.quests.iter().find(|&x| x.quest_id == nft.quest_id).unwrap();

            // Check date if allowed to claim
            let current_time = _env.block.time.seconds();
            if current_time < nft.staked_date.unwrap() + quest.duration_in_staking
            {
                return Err(ContractError::CustomError {val: "You're trying to claim before the staking period is over".to_string()});
            }

            amount_to_send += quest.shill_reward;
            //TODO check for trait bonus here

            //add staked nft to history 
            let staked_history_store = STAKED_NFTS_HISTORY_STORE.add_suffix(sender.to_string().as_bytes());
            let history_token: HistoryToken = { HistoryToken {
                token_id: nft.token_id,
                owner: nft.owner,
                sender: nft.sender,
                quest_id: nft.quest_id,
                staked_date: nft.staked_date,
                claimed_date: Some(current_time),
                reward_amount: amount_to_send,
                xp_reward: quest.xp_reward
            }};
            
            staked_history_store.push(deps.storage, &history_token)?;

            let new_ext = 
                if let Some(Metadata { extension, .. }) = meta.public_metadata {
                    if let Some(mut ext) = extension { 
                        let current_xp_trait = ext.attributes.as_ref().unwrap().iter().find(|&x| x.trait_type == Some("XP".to_string())).unwrap();
                        let current_lvl_trait = ext.attributes.as_ref().unwrap().iter().find(|&x| x.trait_type == Some("LVL".to_string())).unwrap();
                        let current_xp = current_xp_trait.value.parse::<i32>().unwrap() + quest.xp_reward;
                        let current_lvl = current_lvl_trait.value.parse::<i32>().unwrap();
                        for attr in ext.attributes.as_mut().unwrap().iter_mut() {

                            if attr.trait_type == Some("XP".to_string()) {
                                attr.value = current_xp.to_string();
                            }  

                            if attr.trait_type == Some("LVL".to_string()) {
                                let shouldbe_lvl = if attr.value.parse::<i32>().unwrap() < state.level_cap {
                                        levels.iter().find(|&x| x.xp_needed > current_xp).unwrap().level - 1
                                    } 
                                    else { 
                                        attr.value.parse::<i32>().unwrap() 
                                    }; 
                                attr.value = shouldbe_lvl.to_string();

                                if shouldbe_lvl > current_lvl {
                                    response_attrs.push(("lvl_increase_".to_string() + &token_id, shouldbe_lvl.to_string()));
                                }
                            } 
                            
                            if has_bonus_trait == false && quest.bonus_reward_traits.iter().any(|i| i.trait_type==attr.trait_type && i.value == attr.value) {
                                has_bonus_trait = true;
                                amount_to_send += quest.shill_trait_bonus_reward;
                            }
                        }
                        ext 
                    }
                    else {
                        return Err(ContractError::CustomError {val: "unable to set metadata with uri".to_string()});
                    }
                } 
                else {
                    return Err(ContractError::CustomError {val: "unable to get metadata from nft contract".to_string()});
                };
           

            response_msgs.push(
                set_metadata_msg(
                    token_id.to_string(),
                    Some(Metadata {
                        token_uri: None,
                        extension: Some(new_ext),
                    }),
                    None,
                    None,
                    BLOCK_SIZE,
                    state.quest_contract.code_hash.clone(),
                    state.quest_contract.address.to_string()
                )?
            ); 
             
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
         
    STAKED_NFTS_STORE.insert(deps.storage, &deps.api.addr_canonicalize(&sender.to_string())?, &staked_nfts)?;
    response_attrs.push(("shill_amount".to_string(), amount_to_send.to_string()));
 
    Ok(Response::new().add_messages(response_msgs).add_attributes(response_attrs))
}

 

pub fn try_set_viewing_key(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    key: String
) -> Result<Response, ContractError> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    let prng_seed: Vec<u8> = sha_256(base64::encode(key).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);

    let vk: ViewerInfo = { ViewerInfo {
        address: sender.to_string(),
        viewing_key: viewing_key,
    } };

    if sender.clone() == state.owner {
        ADMIN_VIEWING_KEY_ITEM.save(deps.storage, &vk)?;
    }  
    else{
        VIEWING_KEY_STORE.insert(deps.storage, &deps.api.addr_canonicalize(&sender.to_string())?, &vk)?;
    }
 
    Ok(Response::default())
}

pub fn try_send_shill_back(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    amount: Uint128,
    address: Addr
) -> Result<Response, ContractError> {  
    let state = CONFIG_ITEM.load(deps.storage)?;
    if sender.clone() != state.owner {
        return Err(ContractError::Unauthorized {});
    }
   
    Ok(Response::new().add_message(
        transfer_msg(
            address.to_string(),
            amount,
            None,
            None,
            256,
            state.shill_contract.code_hash.to_string(),
            state.shill_contract.address.to_string()
        )?)
    ) 
}

#[entry_point]
pub fn query(
    deps: Deps,
    _env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg { 
        QueryMsg::GetQuests {} => to_binary(&query_quests(deps)?),
        QueryMsg::GetState {viewer} => to_binary(&query_state(deps, viewer)?),
        QueryMsg::GetUserStakedNfts {permit} => to_binary(&query_user_staked_nfts(deps, permit)?),
        QueryMsg::GetNumUserStakedNftHistory { permit } => to_binary(&query_num_user_staked_nft_history(deps, permit)?),
        QueryMsg::GetUserStakedNftHistory {permit, start_page, page_size} => to_binary(&query_user_staked_nft_history(deps, permit, start_page, page_size)?),
        QueryMsg::GetNumStakedNftKeys { viewer } => to_binary(&query_num_staked_keys(deps, viewer)?),
        QueryMsg::GetStakedNfts { viewer, start_page, page_size } => to_binary(&query_staked_nfts(deps, viewer, start_page, page_size)?)
       
        
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
    viewer: ViewerInfo
) -> StdResult<State> {
    check_admin_key(deps, viewer)?;

    let state = CONFIG_ITEM.load(deps.storage)?;  

    Ok(state)
}
 
fn query_user_staked_nfts(
    deps: Deps, 
    permit: Permit
) -> StdResult<Vec<Token>> { 
    let (user_raw, my_addr) = get_querier(deps, permit)?;

    let staked_nfts = STAKED_NFTS_STORE.get(deps.storage, &user_raw).unwrap();
 
    Ok(staked_nfts)
}

fn query_num_staked_keys(
    deps: Deps, 
    viewer: ViewerInfo
) -> StdResult<u32> {
    check_admin_key(deps, viewer)?;
    let num_staked_keys = STAKED_NFTS_STORE.get_len(deps.storage).unwrap();

    Ok(num_staked_keys)
}

fn query_staked_nfts(
    deps: Deps, 
    viewer: ViewerInfo,
    start_page: u32, 
    page_size: u32
) -> StdResult<Vec<(CanonicalAddr,Vec<Token>)>> {
    check_admin_key(deps, viewer)?; 
    let staked_nfts = STAKED_NFTS_STORE.paging(deps.storage, start_page, page_size)?;
    Ok(staked_nfts)
}

fn query_user_staked_nft_history(
    deps: Deps, 
    permit: Permit,
    start_page: u32, 
    page_size: u32
) -> StdResult<Vec<HistoryToken>> {
    let (user_raw, my_addr) = get_querier(deps, permit)?;
    
    let staked_history_store = STAKED_NFTS_HISTORY_STORE.add_suffix(&user_raw);
    //let staked_history_store = STAKED_NFTS_HISTORY_STORE.add_suffix(viewer.address.to_string().as_bytes());
    let history = staked_history_store.paging(deps.storage, start_page, page_size)?;
    Ok(history)
}

fn query_num_user_staked_nft_history(
    deps: Deps, 
    permit: Permit
) -> StdResult<u32> { 
    let (user_raw, my_addr) = get_querier(deps, permit)?;
    let staked_history_store = STAKED_NFTS_HISTORY_STORE.add_suffix(&user_raw);
    let num = staked_history_store.get_len(deps.storage)?;
    Ok(num)
} 

fn check_admin_key(deps: Deps, viewer: ViewerInfo) -> StdResult<()> {
    let admin_viewing_key = ADMIN_VIEWING_KEY_ITEM.load(deps.storage)?;  
    let prng_seed: Vec<u8> = sha_256(base64::encode(viewer.viewing_key).as_bytes()).to_vec();
    let vk = base64::encode(&prng_seed);

    if vk != admin_viewing_key.viewing_key || viewer.address != admin_viewing_key.address{
        return Err(StdError::generic_err(
            "Wrong viewing key for this address or viewing key not set",
        )); 
    }

    return Ok(());
}

fn get_querier(
    deps: Deps,
    permit: Permit,
) -> StdResult<(CanonicalAddr, Option<CanonicalAddr>)> {
    if let pmt = permit {
        let me_raw: CanonicalAddr = MY_ADDRESS_ITEM.load(deps.storage)?;
        let my_address = deps.api.addr_humanize(&me_raw)?;
        let querier = deps.api.addr_canonicalize(&validate(
            deps,
            PREFIX_REVOKED_PERMITS,
            &pmt,
            my_address.to_string(),
            None
        )?)?;
        if !pmt.check_permission(&secret_toolkit::permit::TokenPermissions::Owner) {
            return Err(StdError::generic_err(format!(
                "Owner permission is required for Stashh minter queries, got permissions {:?}",
                pmt.params.permissions
            )));
        }
        return Ok((querier, Some(me_raw)));
    }
    return Err(StdError::generic_err(
        "Unauthorized",
    ));  
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


