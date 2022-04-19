use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map, U32Key};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub total_supply: Uint128,
    pub locked_b_luna: Uint128,
    pub swap_wallet: Addr,
    pub paused: bool,
    pub anchor_liquidation_queue: Addr,
    pub collateral_token: Addr,
    pub price_oracle: Addr,
    pub astroport_router: Addr,
    pub lock_period: u64,
    pub withdraw_lock: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TokenRecord {
    pub amount: Uint128,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Permission {
    pub submit_bid: bool,
}

pub const BALANCES: Map<&[u8], Uint128> = Map::new("balance");

pub const LAST_DEPOSIT: Map<&[u8], Timestamp> = Map::new("last_deposit");

pub const PERMISSIONS: Map<&[u8], Permission> = Map::new("permission");

pub const STATE: Item<State> = Item::new("state");

pub const CLAIM_LIST: Map<U32Key, TokenRecord> = Map::new("claim_list");
