use crate::ContractError::{
    DivideByZeroError, Insufficient, Invalidate, Locked, Paused, Unauthorized,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::Order::Ascending;
use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, Fraction,
    MessageInfo, Order, Response, StdResult, Timestamp, Uint128, Uint256, WasmMsg,
};
use cw2::set_contract_version;
use cw_storage_plus::U32Key;
use std::convert::{TryFrom, TryInto};
use std::ops::Mul;
use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::AssetInfo::{NativeToken, Token};
use crate::msg::SwapOperation::{AstroSwap, NativeSwap};
use crate::msg::{
    ActivatableResponse, BalanceResponse, BidsResponse, ClaimableResponse, ConfigResponse,
    Cw20BalanceResponse, ExecuteMsg, ExternalMsg, ExternalQueryMsg, InfoResponse, InstantiateMsg,
    PermissionResponse, PriceResponse, QueryMsg, TimestampResponse, TotalCapResponse,
    UnlockableResponse,
};
use crate::state::{
    Permission, State, TokenRecord, BALANCES, CLAIM_LIST, LAST_DEPOSIT, PERMISSIONS, STATE,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-deposit-withdraw";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: msg.owner.clone(),
        total_supply: Uint128::zero(),
        locked_b_luna: Uint128::zero(),
        swap_wallet: msg.swap_wallet.clone(),
        paused: false,
        anchor_liquidation_queue: msg
            .anchor_liquidation_queue
            .unwrap_or_else(|| Addr::unchecked("terra1e25zllgag7j9xsun3me4stnye2pcg66234je3u")),
        collateral_token: msg
            .collateral_token
            .unwrap_or_else(|| Addr::unchecked("terra1kc87mu460fwkqte29rquh4hc20m54fxwtsx7gp")),
        price_oracle: msg
            .price_oracle
            .unwrap_or_else(|| Addr::unchecked("terra1cgg6yef7qcdm070qftghfulaxmllgmvk77nc7t")),
        astroport_router: msg
            .astroport_router
            .unwrap_or_else(|| Addr::unchecked("terra16t7dpwwgx9n3lq6l6te3753lsjqwhxwpday9zx")),
        lock_period: msg.lock_period.unwrap_or(14 * 24 * 60 * 60),
        withdraw_lock: msg.withdraw_lock.unwrap_or(60 * 60),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    PERMISSIONS.save(
        deps.storage,
        deps.api
            .addr_canonicalize(&msg.owner.to_string())?
            .as_slice(),
        &Permission { submit_bid: true },
    )?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", state.owner)
        .add_attribute("swap_wallet", state.swap_wallet))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Deposit UST to vault
        ExecuteMsg::Deposit {} => deposit(deps, env, info),
        // Withdraw UST from vault
        ExecuteMsg::WithdrawUst { share } => withdraw_ust(deps, env, info, share),
        // Withdraw bLuna from Vault
        ExecuteMsg::WithdrawBLuna { share } => withdraw_b_luna(deps, env, info, share),
        // Activate all bids
        ExecuteMsg::ActivateBid {} => activate_bid(deps, env, info),
        // Submit bid with amount and premium slot from service
        // Only owner can execute
        ExecuteMsg::SubmitBid {
            amount,
            premium_slot,
        } => submit_bid(deps, env, info, amount, premium_slot),
        // Withdraw all liquidated bLuna from Anchor
        ExecuteMsg::ClaimLiquidation {} => claim_liquidation(deps, env, info),
        ExecuteMsg::Unlock {} => unlock(deps, env, info),
        ExecuteMsg::Swap {} => swap(deps, env, info),
        ExecuteMsg::SetPermission {
            address,
            new_permission,
        } => set_permission(deps, info, address, new_permission),
        ExecuteMsg::UpdateConfig {
            owner,
            paused,
            swap_wallet,
            lock_period,
            withdraw_lock,
        } => update_config(
            deps,
            info,
            owner,
            paused,
            swap_wallet,
            lock_period,
            withdraw_lock,
        ),
    }
}

fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Only one coin
    if info.funds.len() != 1 {
        return Err(Invalidate {});
    }
    let mut share: Uint128 = info.funds[0].amount;
    // Only UST and non-zero amount
    if info.funds[0].denom != "uusd" || share.is_zero() {
        return Err(Invalidate {});
    }
    let mut state = STATE.load(deps.storage)?;
    if state.paused {
        return Err(Paused {});
    }
    let msg_sender = info.sender.to_string().to_lowercase();
    LAST_DEPOSIT.save(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
        &env.block.time,
    )?;
    // UST in vault
    let mut usd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount
        - share;
    // bLuna in vault
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        state.collateral_token.to_string(),
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    // Iterate all valid bids
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            // Waiting UST for liquidation
            usd_balance += Uint128::try_from(item.amount)?;
            // Pending bLuna in Anchor
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    // Fetch bLuna price from oracle
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        state.price_oracle.to_string(),
        &ExternalQueryMsg::Price {
            base: state.collateral_token.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    if !state.total_supply.is_zero() {
        if total_cap.is_zero() {
            return Err(DivideByZeroError {});
        }
        share = share.checked_mul(state.total_supply)? / total_cap;
    }
    state.total_supply += share;
    STATE.save(deps.storage, &state)?;
    BALANCES.update(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default() + share) },
    )?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "deposit"),
        attr("from", info.sender),
        attr("amount", info.funds[0].amount),
        attr("share", share),
    ]))
}

fn submit_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    premium_slot: u8,
) -> Result<Response, ContractError> {
    let permission = PERMISSIONS
        .may_load(
            deps.storage,
            deps.api
                .addr_canonicalize(info.sender.to_string().to_lowercase().as_str())?
                .as_slice(),
        )?
        .unwrap_or(Permission { submit_bid: false });
    if !permission.submit_bid {
        return Err(Unauthorized {});
    }
    let usd_balance = deps
        .querier
        .query_balance(env.contract.address, "uusd")?
        .amount;
    if !amount.is_zero() && usd_balance >= amount {
        let state = STATE.load(deps.storage)?;
        Ok(Response::new()
            .add_attributes(vec![
                attr("action", "submit_bid"),
                attr("from", info.sender),
                attr("amount", amount),
                attr("premium_slot", premium_slot.to_string()),
            ])
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.anchor_liquidation_queue.to_string(),
                funds: vec![Coin::new(amount.u128(), "uusd")],
                msg: to_binary(&ExternalMsg::SubmitBid {
                    collateral_token: state.collateral_token.to_string(),
                    premium_slot,
                })?,
            })))
    } else {
        Err(Insufficient {})
    }
}

fn activate_bid(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    let mut bids_idx = Vec::new();
    let state = STATE.load(deps.storage)?;
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            if let Some(wait_end) = item.wait_end {
                if wait_end < env.block.time.seconds() {
                    bids_idx.push(item.idx);
                } else {
                    break;
                }
            }
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    Ok(Response::new()
        .add_attributes(vec![attr("action", "activate"), attr("from", info.sender)])
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.anchor_liquidation_queue.to_string(),
            funds: vec![],
            msg: to_binary(&ExternalMsg::ActivateBids {
                collateral_token: state.collateral_token.to_string(),
                bids_idx: Some(bids_idx),
            })?,
        })))
}

fn withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    share: Uint128,
) -> Result<Response, ContractError> {
    if share.is_zero() {
        return Err(Invalidate {});
    }
    let msg_sender = info.sender.to_string().to_lowercase();
    let last_timestamp = LAST_DEPOSIT.may_load(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
    )?;
    let mut state = STATE.load(deps.storage)?;
    if let Some(timestamp) = last_timestamp {
        if timestamp.plus_seconds(state.withdraw_lock) >= env.block.time {
            return Err(Locked {});
        }
    }
    BALANCES.update(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(share)?) },
    )?;
    let uusd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let mut usd_balance = uusd_balance;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        state.collateral_token.to_string(),
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            usd_balance += Uint128::try_from(item.amount)?;
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        state.price_oracle.to_string(),
        &ExternalQueryMsg::Price {
            base: state.collateral_token.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    // Calculate total cap
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;

    // Calculate exact amount from share and total cap
    let withdraw_cap = total_cap * share / state.total_supply;
    if withdraw_cap.is_zero() {
        return Err(Insufficient {});
    }
    // Withdraw if UST in vault is enough
    if uusd_balance >= withdraw_cap {
        state.total_supply -= share;
        STATE.save(deps.storage, &state)?;
        Ok(Response::new()
            .add_message(CosmosMsg::Bank(BankMsg::Send {
                to_address: msg_sender,
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: withdraw_cap,
                }],
            }))
            .add_attributes(vec![
                attr("action", "withdraw"),
                attr("to", info.sender),
                attr("share", share),
                attr("amount", withdraw_cap),
            ]))
    } else {
        // Retract bids for insufficient UST in vault
        let mut messages = vec![];
        usd_balance = withdraw_cap - uusd_balance;
        start_after = Some(Uint128::zero());
        loop {
            let res: BidsResponse = deps.querier.query_wasm_smart(
                state.anchor_liquidation_queue.to_string(),
                &ExternalQueryMsg::BidsByUser {
                    collateral_token: state.collateral_token.to_string(),
                    bidder: env.contract.address.to_string(),
                    start_after,
                    limit: Some(31),
                },
            )?;
            for item in &res.bids {
                if !item.amount.is_zero() {
                    if item.amount < usd_balance.into() {
                        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: state.anchor_liquidation_queue.to_string(),
                            msg: to_binary(&ExternalMsg::RetractBid {
                                bid_idx: item.idx,
                                amount: None,
                            })?,
                            funds: vec![],
                        }));
                        usd_balance -= Uint128::try_from(item.amount)?;
                    } else {
                        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: state.anchor_liquidation_queue.to_string(),
                            msg: to_binary(&ExternalMsg::RetractBid {
                                bid_idx: item.idx,
                                amount: Some(usd_balance.into()),
                            })?,
                            funds: vec![],
                        }));
                        usd_balance = Uint128::zero();
                        break;
                    }
                }
            }
            if usd_balance.is_zero() || res.bids.len() < 31 {
                break;
            }
            start_after = Some(res.bids.last().unwrap().idx);
        }
        if withdraw_cap > usd_balance {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: msg_sender.clone(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: withdraw_cap - usd_balance,
                }],
            }));
        }
        let mut unlocked_b_luna = Uint128::zero();
        if !usd_balance.is_zero() {
            let mut b_luna_withdraw = b_luna_balance * share * usd_balance
                / withdraw_cap
                / (state.total_supply - share * (withdraw_cap - usd_balance) / withdraw_cap);
            if !b_luna_withdraw.is_zero() {
                // swap on wallet
                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: state.collateral_token.to_string(),
                    msg: to_binary(&ExternalMsg::Send {
                        contract: state.swap_wallet.to_string(),
                        amount: b_luna_withdraw,
                        msg: to_binary(&deps.api.addr_validate(&msg_sender)?)?,
                    })?,
                    funds: vec![],
                }));
                // unlock
                if b_luna_balance_response.balance - state.locked_b_luna < b_luna_withdraw {
                    b_luna_withdraw -= b_luna_balance_response.balance - state.locked_b_luna;
                    let keys = CLAIM_LIST.keys(deps.storage, None, None, Order::Ascending);
                    let mut remove_keys = Vec::new();
                    let mut last_key = None;
                    let mut new_claim = TokenRecord {
                        amount: Uint128::zero(),
                        timestamp: Timestamp::default(),
                    };
                    for key in keys {
                        let claim = CLAIM_LIST.load(deps.storage, U32Key::from(key.clone()))?;
                        if b_luna_withdraw >= claim.amount {
                            b_luna_withdraw -= claim.amount;
                            unlocked_b_luna += claim.amount;
                            remove_keys.push(U32Key::from(key));
                        } else {
                            unlocked_b_luna += b_luna_withdraw;
                            new_claim = claim.clone();
                            new_claim.amount -= b_luna_withdraw;
                            b_luna_withdraw = Uint128::zero();
                            last_key = Some(key);
                        }
                        if b_luna_withdraw.is_zero() {
                            break;
                        }
                    }
                    for key in remove_keys {
                        CLAIM_LIST.remove(deps.storage, key);
                    }
                    if let Some(key) = last_key {
                        CLAIM_LIST.save(deps.storage, U32Key::from(key), &new_claim)?;
                    }
                    if !unlocked_b_luna.is_zero() {
                        state.locked_b_luna -= unlocked_b_luna;
                    }
                }
            }
        }
        state.total_supply -= share;
        STATE.save(deps.storage, &state)?;
        let mut attrs = vec![
            attr("action", "withdraw"),
            attr("to", info.sender),
            attr("share", share),
            attr("amount", withdraw_cap),
        ];
        if !unlocked_b_luna.is_zero() {
            attrs.push(attr("unlocked", unlocked_b_luna.to_string()));
        }
        Ok(Response::new().add_messages(messages).add_attributes(attrs))
    }
}

fn withdraw_b_luna(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    share: Uint128,
) -> Result<Response, ContractError> {
    if share.is_zero() {
        return Err(Invalidate {});
    }
    let msg_sender = info.sender.to_string().to_lowercase();
    let last_timestamp = LAST_DEPOSIT.may_load(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
    )?;
    let mut state = STATE.load(deps.storage)?;
    if let Some(timestamp) = last_timestamp {
        if timestamp.plus_seconds(state.withdraw_lock) >= env.block.time {
            return Err(Locked {});
        }
    }
    BALANCES.update(
        deps.storage,
        deps.api.addr_canonicalize(&msg_sender)?.as_slice(),
        |balance| -> StdResult<_> { Ok(balance.unwrap_or_default().checked_sub(share)?) },
    )?;
    let uusd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let mut usd_balance = uusd_balance;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        state.collateral_token.to_string(),
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;

    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            usd_balance += Uint128::try_from(item.amount)?;
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        state.price_oracle.to_string(),
        &ExternalQueryMsg::Price {
            base: state.collateral_token.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    // Calculate total cap
    let total_cap =
        b_luna_balance + Uint128::try_from(Uint256::from(usd_balance).mul(price.inv().unwrap()))?;
    // Calculate exact amount from share and total cap
    let withdraw_cap = total_cap * share / state.total_supply;

    state.total_supply -= share;
    STATE.save(deps.storage, &state)?;
    // Withdraw if bLuna in vault is enough
    if b_luna_balance_response.balance - state.locked_b_luna >= withdraw_cap {
        Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.collateral_token.to_string(),
                msg: to_binary(&ExternalMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: withdraw_cap,
                })?,
                funds: vec![],
            }))
            .add_attributes(vec![
                attr("action", "withdraw"),
                attr("to", info.sender),
                attr("share", share),
                attr("amount", withdraw_cap),
            ]))
    } else {
        Err(Locked {})
    }
}

fn claim_liquidation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut b_luna_balance = Uint128::zero();
    let mut start_after = Some(Uint128::zero());
    let mut state = STATE.load(deps.storage)?;
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    if b_luna_balance.is_zero() {
        return Err(Insufficient {});
    }
    let last_key = CLAIM_LIST.keys(deps.storage, None, None, Ascending).last();
    let new_key = if let Some(value) = last_key {
        u32::from_be_bytes(value.as_slice().try_into().unwrap()) + 1
    } else {
        0
    };
    CLAIM_LIST.save(
        deps.storage,
        U32Key::from(new_key),
        &TokenRecord {
            amount: b_luna_balance,
            timestamp: env.block.time,
        },
    )?;

    state.locked_b_luna += b_luna_balance;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.anchor_liquidation_queue.to_string(),
            msg: to_binary(&ExternalMsg::ClaimLiquidations {
                collateral_token: state.collateral_token.to_string(),
                bids_idx: None,
            })?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "liquidate"),
            attr("from", &info.sender),
            attr("amount", b_luna_balance.to_string()),
        ]))
}

fn unlock(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let keys = CLAIM_LIST.keys(deps.storage, None, None, Order::Ascending);
    let mut remove_keys = Vec::new();
    let mut unlocked_b_luna = Uint128::zero();
    let state = STATE.load(deps.storage)?;
    for key in keys {
        let claim = CLAIM_LIST.load(deps.storage, U32Key::from(key.clone()))?;
        if claim.timestamp.plus_seconds(state.lock_period) <= env.block.time {
            unlocked_b_luna += claim.amount;
            remove_keys.push(U32Key::from(key));
        } else {
            break;
        }
    }
    for key in remove_keys {
        CLAIM_LIST.remove(deps.storage, key);
    }
    if unlocked_b_luna.is_zero() {
        return Err(Insufficient {});
    }
    let mut state = STATE.load(deps.storage)?;
    state.locked_b_luna -= unlocked_b_luna;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "unlock"),
        attr("from", info.sender),
        attr("amount", unlocked_b_luna.to_string()),
    ]))
}

fn set_permission(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
    new_permission: Permission,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    if state.owner.to_string().to_lowercase() != info.sender.to_string().to_lowercase() {
        return Err(Unauthorized {});
    }
    let permission = PERMISSIONS
        .may_load(
            deps.storage,
            deps.api
                .addr_canonicalize(&address.to_string().to_lowercase())?
                .as_slice(),
        )?
        .unwrap_or(Permission { submit_bid: false });
    if permission == new_permission {
        return Err(Invalidate {});
    }
    if permission == (Permission { submit_bid: false }) {
        PERMISSIONS.remove(
            deps.storage,
            deps.api
                .addr_canonicalize(&address.to_string().to_lowercase())?
                .as_slice(),
        );
    } else {
        PERMISSIONS.save(
            deps.storage,
            deps.api
                .addr_canonicalize(&address.to_string().to_lowercase())?
                .as_slice(),
            &permission,
        )?;
    }
    Ok(Response::new().add_attributes(vec![
        attr("action", "set_permission"),
        attr("from", info.sender),
        attr("to", address.to_string().to_lowercase()),
        attr("submit_bid", new_permission.submit_bid.to_string()),
    ]))
}

fn swap(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        state.collateral_token.to_string(),
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let swap_amount = b_luna_balance_response.balance - state.locked_b_luna;
    if swap_amount.is_zero() {
        return Err(Insufficient {});
    }
    let msg = ExternalMsg::Send {
        contract: state.astroport_router.to_string(),
        amount: swap_amount,
        msg: to_binary(&ExternalMsg::ExecuteSwapOperations {
            operations: vec![
                AstroSwap {
                    offer_asset_info: Token {
                        contract_addr: state.collateral_token.clone(),
                    },
                    ask_asset_info: NativeToken {
                        denom: "uluna".to_string(),
                    },
                },
                NativeSwap {
                    offer_denom: "uluna".to_string(),
                    ask_denom: "uusd".to_string(),
                },
            ],
            minimum_receive: None,
            to: None,
            max_spread: Some(Decimal::from_str("0.5")?),
        })?,
    };
    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.collateral_token.to_string(),
            msg: to_binary(&msg)?,
            funds: vec![],
        }))
        .add_attributes(vec![
            attr("action", "swap"),
            attr("from", info.sender),
            attr("amount", swap_amount.to_string()),
        ]))
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<Addr>,
    paused: Option<bool>,
    swap_wallet: Option<Addr>,
    lock_period: Option<u64>,
    withdraw_lock: Option<u64>,
) -> Result<Response, ContractError> {
    let mut state = STATE.load(deps.storage)?;
    if state.owner.to_string().to_lowercase() != info.sender.to_string().to_lowercase() {
        return Err(Unauthorized {});
    }
    let mut attributes = vec![attr("action", "update_config"), attr("from", info.sender)];
    if let Some(owner) = owner {
        if owner.to_string().to_lowercase() != state.owner {
            PERMISSIONS.remove(
                deps.storage,
                deps.api
                    .addr_canonicalize(state.owner.to_string().as_str())?
                    .as_slice(),
            );
            PERMISSIONS.save(
                deps.storage,
                deps.api
                    .addr_canonicalize(owner.to_string().to_lowercase().as_str())?
                    .as_slice(),
                &Permission { submit_bid: true },
            )?;
            state.owner = deps
                .api
                .addr_validate(owner.to_string().to_lowercase().as_str())?;
            attributes.push(attr("owner", state.owner.to_string()));
        }
    }
    if let Some(paused) = paused {
        if paused != state.paused {
            state.paused = paused;
            attributes.push(attr("paused", paused.to_string()));
        }
    }
    if let Some(swap_wallet) = swap_wallet {
        if swap_wallet.to_string().to_lowercase() != state.swap_wallet {
            state.swap_wallet = deps
                .api
                .addr_validate(swap_wallet.to_string().to_lowercase().as_str())?;
            attributes.push(attr("swap_wallet", state.swap_wallet.to_string()));
        }
    }
    if let Some(lock_period) = lock_period {
        if lock_period != state.lock_period {
            state.lock_period = lock_period;
            attributes.push(attr("lock_period", lock_period.to_string()));
        }
    }
    if let Some(withdraw_lock) = withdraw_lock {
        if withdraw_lock != state.withdraw_lock {
            state.withdraw_lock = withdraw_lock;
            attributes.push(attr("withdraw_lock", withdraw_lock.to_string()));
        }
    }
    if attributes.len() <= 2 {
        return Err(Invalidate {});
    }
    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attributes(attributes))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetInfo {} => to_binary(&query_info(deps)?),
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        // Get share from address
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        // Get total cap in vault and anchor
        QueryMsg::TotalCap {} => to_binary(&query_total_cap(deps, env)?),
        // Return true if activate is needed
        QueryMsg::Activatable {} => to_binary(&query_activatable(deps, env)?),
        // Return true if liquidate is needed
        QueryMsg::Claimable {} => to_binary(&query_claimable(deps, env)?),
        QueryMsg::Permission { address } => to_binary(&query_permission(deps, address)?),
        QueryMsg::Unlockable {} => to_binary(&query_unlockable(deps, env)?),
        QueryMsg::LastDepositTimestamp { address } => {
            to_binary(&query_last_deposit_timestamp(deps, address)?)
        }
    }
}

fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(InfoResponse {
        total_supply: state.total_supply,
        locked_b_luna: state.locked_b_luna,
    })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: state.owner.to_string(),
        paused: state.paused,
        swap_wallet: state.swap_wallet.to_string(),
        anchor_liquidation_queue: state.anchor_liquidation_queue.to_string(),
        collateral_token: state.collateral_token.to_string(),
        price_oracle: state.price_oracle.to_string(),
        astroport_router: state.astroport_router.to_string(),
        lock_period: state.lock_period,
        withdraw_lock: state.withdraw_lock,
    })
}

fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, address.as_slice())?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
}

fn query_total_cap(deps: Deps, env: Env) -> StdResult<TotalCapResponse> {
    let mut usd_balance = deps
        .querier
        .query_balance(&env.contract.address, "uusd")?
        .amount;
    let state = STATE.load(deps.storage)?;
    let b_luna_balance_response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        state.collateral_token.to_string(),
        &ExternalQueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;
    let mut b_luna_balance = b_luna_balance_response.balance;
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            usd_balance += Uint128::try_from(item.amount)?;
            b_luna_balance += Uint128::try_from(item.pending_liquidated_collateral)?;
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    let price_response: PriceResponse = deps.querier.query_wasm_smart(
        state.price_oracle.to_string(),
        &ExternalQueryMsg::Price {
            base: state.collateral_token.to_string(),
            quote: "uusd".to_string(),
        },
    )?;
    let price = price_response.rate;
    let total_cap = Uint128::try_from(Uint256::from(b_luna_balance).mul(price))? + usd_balance;
    Ok(TotalCapResponse { total_cap })
}

fn query_activatable(deps: Deps, env: Env) -> StdResult<ActivatableResponse> {
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    let state = STATE.load(deps.storage)?;
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            if item.wait_end.is_some() && item.wait_end.unwrap() < env.block.time.seconds() {
                return Ok(ActivatableResponse { activatable: true });
            }
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    Ok(ActivatableResponse { activatable: false })
}

fn query_claimable(deps: Deps, env: Env) -> StdResult<ClaimableResponse> {
    let mut start_after: Option<Uint128> = Some(Uint128::zero());
    let state = STATE.load(deps.storage)?;
    loop {
        let res: BidsResponse = deps.querier.query_wasm_smart(
            state.anchor_liquidation_queue.to_string(),
            &ExternalQueryMsg::BidsByUser {
                collateral_token: state.collateral_token.to_string(),
                bidder: env.contract.address.to_string(),
                start_after,
                limit: Some(31),
            },
        )?;
        for item in &res.bids {
            if !item.pending_liquidated_collateral.is_zero() {
                return Ok(ClaimableResponse { claimable: true });
            }
        }
        if res.bids.len() < 31 {
            break;
        }
        start_after = Some(res.bids.last().unwrap().idx);
    }
    Ok(ClaimableResponse { claimable: false })
}

fn query_permission(deps: Deps, address: String) -> StdResult<PermissionResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let permission = PERMISSIONS
        .may_load(deps.storage, address.as_slice())?
        .unwrap_or(Permission { submit_bid: false });
    Ok(PermissionResponse { permission })
}

fn query_unlockable(deps: Deps, env: Env) -> StdResult<UnlockableResponse> {
    let mut keys = CLAIM_LIST.keys(deps.storage, None, None, Order::Ascending);
    let state = STATE.load(deps.storage)?;
    if let Some(key) = keys.next() {
        let claim = CLAIM_LIST.load(deps.storage, U32Key::from(key))?;
        if claim.timestamp.plus_seconds(state.lock_period) <= env.block.time {
            return Ok(UnlockableResponse { unlockable: true });
        }
    }
    Ok(UnlockableResponse { unlockable: false })
}

fn query_last_deposit_timestamp(deps: Deps, address: String) -> StdResult<TimestampResponse> {
    let address = deps.api.addr_canonicalize(&address)?;
    let last_timestamp = LAST_DEPOSIT.may_load(deps.storage, address.as_slice())?;
    if let Some(timestamp) = last_timestamp {
        Ok(TimestampResponse { timestamp })
    } else {
        Ok(TimestampResponse {
            timestamp: Timestamp::default(),
        })
    }
}
