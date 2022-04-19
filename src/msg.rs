use crate::state::Permission;
use cosmwasm_std::{Addr, Binary, Decimal, Decimal256, Timestamp, Uint128, Uint256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub swap_wallet: Addr,
    pub anchor_liquidation_queue: Option<Addr>,
    pub collateral_token: Option<Addr>,
    pub price_oracle: Option<Addr>,
    pub astroport_router: Option<Addr>,
    pub lock_period: Option<u64>,
    pub withdraw_lock: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Deposit {},
    WithdrawUst {
        share: Uint128,
    },
    WithdrawBLuna {
        share: Uint128,
    },
    ActivateBid {},
    SubmitBid {
        amount: Uint128,
        premium_slot: u8,
    },
    ClaimLiquidation {},
    Unlock {},
    Swap {},
    SetPermission {
        address: Addr,
        new_permission: Permission,
    },
    UpdateConfig {
        owner: Option<Addr>,
        paused: Option<bool>,
        swap_wallet: Option<Addr>,
        lock_period: Option<u64>,
        withdraw_lock: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalMsg {
    ActivateBids {
        collateral_token: String,
        bids_idx: Option<Vec<Uint128>>,
    },
    SubmitBid {
        collateral_token: String,
        premium_slot: u8,
    },
    RetractBid {
        bid_idx: Uint128,
        amount: Option<Uint256>,
    },
    ClaimLiquidations {
        collateral_token: String,
        bids_idx: Option<Vec<Uint128>>,
    },
    Transfer {
        recipient: String,
        amount: Uint128,
    },
    ExecuteSwapOperations {
        operations: Vec<SwapOperation>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
        max_spread: Option<Decimal>,
    },
    Send {
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    Swap {
        to: Addr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetInfo {},
    Config {},
    Balance { address: String },
    TotalCap {},
    Activatable {},
    Claimable {},
    Permission { address: String },
    Unlockable {},
    LastDepositTimestamp { address: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub total_supply: Uint128,
    pub locked_b_luna: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub paused: bool,
    pub swap_wallet: String,
    pub anchor_liquidation_queue: String,
    pub collateral_token: String,
    pub price_oracle: String,
    pub astroport_router: String,
    pub lock_period: u64,
    pub withdraw_lock: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TotalCapResponse {
    pub total_cap: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ActivatableResponse {
    pub activatable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ClaimableResponse {
    pub claimable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PermissionResponse {
    pub permission: Permission,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnlockableResponse {
    pub unlockable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalQueryMsg {
    // GetCount returns the current count as a json-encoded number
    Balance {
        address: String,
    },
    BidsByUser {
        collateral_token: String,
        bidder: String,
        start_after: Option<Uint128>,
        limit: Option<u8>,
    },
    Price {
        base: String,
        quote: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Cw20BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidsResponse {
    pub bids: Vec<BidResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidResponse {
    pub idx: Uint128,
    pub collateral_token: String,
    pub premium_slot: u8,
    pub bidder: String,
    pub amount: Uint256,
    pub product_snapshot: Decimal256,
    pub sum_snapshot: Decimal256,
    pub pending_liquidated_collateral: Uint256,
    pub wait_end: Option<u64>,
    pub epoch_snapshot: Uint128,
    pub scale_snapshot: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceResponse {
    pub rate: Decimal256,
    pub last_updated_base: u64,
    pub last_updated_quote: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TimestampResponse {
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapOperation {
    NativeSwap {
        offer_denom: String,
        ask_denom: String,
    },
    AstroSwap {
        offer_asset_info: AssetInfo,
        ask_asset_info: AssetInfo,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token { contract_addr: Addr },
    NativeToken { denom: String },
}
