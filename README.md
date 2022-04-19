# Terra-Deposit-Withdraw

This is a vault smart contract to help with Anchor liquidation bids.

Users can deposit with UST to get a share of the vault.

And withdraw (in UST or bLuna) as much as their asset share of the vault.

The owner can submit bids with specified premium slot and amount from the vault to Anchor liquidation queue.

And the owner can activate submitted bids and claim pending bLuna from Anchor to the vault.

The owner can transfer ownership to another address.

Submitting bids and transfer ownership is unique feature that only owner can execute.

## ExecuteMsg

### Deposit*

User deposit UST to vault.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### WithdrawUst

User withdraws UST from vault.

| Key   | Type    | Description                  |
|-------|---------|------------------------------|
| share | Uint128 | Share amount to withdraw UST |


### ActivateBid

Activate all bids.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### SubmitBid**

Submit bid with amount and premium slot from service.

| Key          | Type    | Description              |
|--------------|---------|--------------------------|
| amount       | Uint128 | UST amount to submit bid |
| premium_slot | u8      | Premium Slot (%)         |

### ClaimLiquidation

Withdraw all liquidated bLuna from Anchor Liquidation Queue.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### Unlock

Unlock locked bLuna.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### Swap

Swap unlocked bLuna into UST using astroport.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

### SetPermission

Swap unlocked bLuna into UST using astroport.

| Key            | Type       | Description                   |
|----------------|------------|-------------------------------|
| address        | Addr       | The address to set permission |
| new_permission | Permission | Permissions                   |

#### Permission(Struct)

| Key            | Type       | Description                  |
|----------------|------------|------------------------------|
| submit_bid     | bool       | `true` if able to submit bid |

### UpdateConfig***

Update configuration.

| Key           | Type          | Description                             |
|---------------|---------------|-----------------------------------------|
| owner         | Option\<Addr> | New owner address                       |
| paused        | Option\<bool> | `true` for pause, `false` for resume    |
| swap_wallet   | Option\<Addr> | New swap wallet address                 |
| lock_period   | Option\<u64>  | bLuna lock period                       |
| withdraw_lock | Option\<u64>  | Withdraw lock period after last deposit |


## QueryMsg

### GetInfo

Get total supply and locked bLuna amount.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

#### InfoResponse

| Key           | Type    | Description                      |
|---------------|---------|----------------------------------|
| total_supply  | Uint128 | Total supply amount of the vault |
| locked_b_luna | Uint128 | Locked bLuna amount              |

### Config

Get owner address and total supply.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

#### ConfigResponse

| Key                      | Type   | Description                               |
|--------------------------|--------|-------------------------------------------|
| owner                    | String | Owner address                             |
| paused                   | bool   | `true` if paused                          |
| swap_wallet              | String | Swap wallet contract address              |
| anchor_liquidation_queue | String | Anchor Liquidation Queue contract address |
| collateral_token         | String | Collateral Token (bLuna) address          |
| price_oracle             | String | Price Oracle contract address             |
| astroport_router         | String | Astroport Router contract address         |
| lock_period              | u64    | bLuna lock period                         |
| withdraw_lock            | u64    | Withdraw lock period after last deposit   |

### Balance

Get share of vault from address.

| Key     | Type   | Description            |
|---------|--------|------------------------|
| address | String | Address to get balance |

#### BalanceResponse

| Key          | Type    | Description                        |
|--------------|---------|------------------------------------|
| balance      | Uint128 | Balance amount of provided address |

### TotalCap

Get total cap in vault and anchor.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

#### TotalCapResponse

| Key       | Type    | Description                                                       |
|-----------|---------|-------------------------------------------------------------------|
| total_cap | Uint128 | Total cap amount in vault and pending in anchor liquidation queue |

### Activatable

Check if there are bids to activate.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

#### ActivatableResponse

| Key         | Type | Description                   |
|-------------|------|-------------------------------|
| activatable | bool | True if activate is available |

### Claimable

Check if there is pending liquidated collateral.

| Key | Type | Description |
|-----|------|-------------|
| -   | -    | -           |

#### ClaimableResponse

| Key       | Type | Description                    |
|-----------|------|--------------------------------|
| claimable | bool | True if liquidate is available |

### WithdrawableLimit

Get withdrawable UST for users.

| Key      | Type   | Description                            |
|----------|--------|----------------------------------------|
| addresss | String | Address to get withdrawable UST amount |

#### WithdrawableLimitResponse

| Key   | Type    | Description                    |
|-------|---------|--------------------------------|
| limit | Uint128 | True if liquidate is available |

### Permission

Get permission of the address.

| Key      | Type   | Description                           |
|----------|--------|---------------------------------------|
| addresss | String | Address to get permission information |

#### PermissionResponse

| Key        | Type       | Description            |
|------------|------------|------------------------|
| permission | Permission | Permission information |

*: Requires UST to be sent beforehand.

**: The user who has permission can execute only.

***: Only owner can execute.
