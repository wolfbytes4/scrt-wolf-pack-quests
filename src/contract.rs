use cosmwasm_std::{
    entry_point, to_binary, Binary, Env, Deps, DepsMut,
    MessageInfo, Response, StdError, StdResult, Addr
};
use crate::error::ContractError;
use crate::msg::{CountResponse, QuestResponse, ExecuteMsg, InstantiateMsg, QueryMsg, Quest, ContractInfo, QuestMsg, Token};
use crate::state::{config, config_read, State};
use crate::rand::{sha_256};
use secret_toolkit::{
    snip721::{
        transfer_nft_msg, private_metadata_query, register_receive_nft_msg,
        set_viewing_key_msg, set_whitelisted_approval_msg, AccessLevel, Metadata, ViewerInfo,
    },
    utils::{pad_handle_result, pad_query_result, HandleCallback},
}; 
use serde::{Deserialize};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg
) -> Result<Response, StdError> {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);
    // create initial state with count and contract owner
    let state = State {
        quests: vec![],
        locked_nfts: vec![],
        owner: info.sender.clone(),
        count: msg.count,
        viewing_key: Some(viewing_key),
        quest_contract: ContractInfo {
            code_hash: msg.quest_contract.code_hash,
            address: msg.quest_contract.address,
        }
    };
    // save the contract state
    config(deps.storage).save(&state)?;

    deps.api.debug(&format!("Contract was initialized by {}", info.sender));
 
    Ok(Response::new()
        .add_message(register_receive_nft_msg(
            _env.contract.code_hash,
            Some(true),
            None,
            256,//blocksize
            state.quest_contract.code_hash.clone(),
            state.quest_contract.address.clone().to_string(),
        )?)
        .add_message(set_viewing_key_msg(
            state.viewing_key.unwrap().to_string(),
            None,
            256,
            state.quest_contract.code_hash,
            state.quest_contract.address.to_string(),
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
        ExecuteMsg::Increment {} => try_increment(deps),
        ExecuteMsg::StartQuest { quest } => try_start_quest(deps, _env, info, quest),
        ExecuteMsg::Reset { count } => try_reset(deps, info, count),
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
    let collection_raw = sender;
    deps.api.debug(&format!("Batch received"));
    //return Err(ContractError::CustomError {val: msg.to_string()});  
   if let Some(bin) = msg { 
     let bytes = base64::decode(bin.to_base64()).unwrap(); // you should handle errors
     let qmsg: QuestMsg = serde_json::from_slice(&bytes).unwrap();
     
     config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        let quest = state.quests.iter().find(|&x| x.quest_id == qmsg.quest_id).unwrap();
        let currentTime = _env.block.time.seconds();
        if currentTime > quest.duration_until_join_closed + quest.create_date {
            return Err(ContractError::CustomError {val: "You can't join this quest".to_string()});
        }
        //check trait bonus on claim
        //check if enough wolfs sent
        if token_ids.len() != quest.requirements.len(){ 
            return Err(ContractError::CustomError {val: "You did not sent enough wolves for this quest".to_string()});
        }
        //enter wolves in array
        for id in token_ids.iter() {
            let locked_wolf = Token { 
                token_id: id.to_string(),
                owner: from.clone(),
                sender: sender.clone(),
                quest_id: qmsg.quest_id
            };
            state.locked_nfts.push(locked_wolf);
        }
        Ok(state)
     })?;
 
   }
    // let contract =
    //     load::<StoreContractInfo, _>(&deps.storage, COLLECTION_KEY)?.into_humanized(&deps.api)?;
 
    // let admins: Vec<CanonicalAddr> = load(&deps.storage, ADMINS_KEY)?;
    // let from_raw = deps.api.canonical_address(from)?;
    // // only allow an admin to add tokens to the gumball
    // if !admins.contains(&from_raw) {
    //     return Err(StdError::unauthorized());
    // }
    // 721 contracts should not be doing a Send if there are no tokens sent, but you never know
    // what people will code
    //if !token_ids.is_empty() { 
        // config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        //     for id in token_ids.iter() {
                
        //         let token_id = id.to_string();
        //         let viewer = Some(ViewerInfo {
        //             address: "VIEWER'S_ADDRESS".to_string(),
        //             viewing_key: "VIEWER'S_KEY".to_string(),
        //         });
        //         let include_expired = None;
        //         let block_size = 256;
        //         let callback_code_hash = "TOKEN_CONTRACT_CODE_HASH".to_string();
        //         let contract_addr = "TOKEN_CONTRACT_ADDRESS".to_string();

        //         let nft_dossier =
        //             nft_dossier_query(deps.querier, token_id, viewer, include_expired, block_size, callback_code_hash, contract_addr)?;
        //                 } 
        //     Ok(state)
        // });
   // }
    Ok(Response::default())
}

pub fn try_start_quest(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    quest: Quest
) -> Result<Response, ContractError> { 

    config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        if info.sender != state.owner{
            return Err(ContractError::Unauthorized {});
        }
        
        if state.quests.iter().any(|i| i.quest_id==quest.quest_id) {
            return Err(ContractError::CustomError {val: "The quest id already exist".to_string()});
        }

        let mut q = quest;
        q.create_date = _env.block.time.seconds();
        state.quests.push(q);
        Ok(state)
    })?;

    deps.api.debug("quest added");
    Ok(Response::default())
}

pub fn try_send_nft_back(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    token_id: String
) -> Result<Response, ContractError> { 
    let mut nft = Token{ owner: Addr::unchecked(""), quest_id: 0, sender: Addr::unchecked(""), token_id: "".to_string()};
    let mut contract: Option<String> = None;
    let mut hash: Option<String> = None;
    config(deps.storage).update(|mut state| -> Result<_,ContractError>{
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
        Ok(state) 
    })?; 
  
    Ok(Response::new()
        .add_message(transfer_nft_msg(
            nft.owner.to_string(),
            nft.token_id.to_string(),
            None,
            None,
            256, 
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
//check date and ownership
//update lvl and xp
//send shill as reward + bonus if traits met
//transfer back
    Ok(Response::default())
}

pub fn try_increment(
    deps: DepsMut,
) -> Result<Response, ContractError> {

    // 1. load state
    // 2. increment the counter by 1
    // 3. save state

    config(deps.storage).update(|mut state| -> Result<_,ContractError>{
        state.count += 1;
        Ok(state)
    })?;

    deps.api.debug("count incremented");
    Ok(Response::default())
}

pub fn try_reset(
    deps: DepsMut,
    info: MessageInfo,
    count: i32,
) -> Result<Response, ContractError> {

    // 1. load state
    // 2. if sender is not the contract owner, return error
    // 3. else, reset the counter to the value given
    config(deps.storage).update(|mut state| -> Result<_, ContractError>{
        if info.sender != state.owner{
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    deps.api.debug("count reset successfully");
    Ok(Response::default())
}

#[entry_point]
pub fn query(
    deps: Deps,
    _env: Env,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
        QueryMsg::GetQuests {} => to_binary(&query_quests(deps)?),
        QueryMsg::GetState {} => to_binary(&query_state(deps)?),
    }
}

fn query_count(
    deps: Deps,
) -> StdResult<CountResponse> {

    // 1. load state
    let state = config_read(deps.storage).load()?;
    deps.api.debug("count incremented successfully");
    // 2. return count response

    Ok(CountResponse { count: state.count })
}

fn query_quests(
    deps: Deps,
) -> StdResult<QuestResponse> {

    // 1. load state
    let state = config_read(deps.storage).load()?;
    deps.api.debug("count incremented successfully"); 

    Ok(QuestResponse { quests: state.quests })
}


fn query_state(
    deps: Deps,
) -> StdResult<State> {

    // 1. load state
    let state = config_read(deps.storage).load()?;
    deps.api.debug("count incremented successfully"); 

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};  
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17, entropy:"wolfpack".to_string(), quest_contract:{ContractInfo{address:Addr::unchecked("secret174kgn5rtw4kf6f938wm7kwh70h2v4vcfft5mqy"), code_hash:"45f450a4277570f8d1a81eb1185e17ce042a217227dfd836a613c7e54ac15447".to_string()} }};
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
