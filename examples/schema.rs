use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use terra_deposit_withdraw::msg::{
    ActivatableResponse, BalanceResponse, ClaimableResponse, ConfigResponse, ExecuteMsg,
    InfoResponse, InstantiateMsg, PermissionResponse, QueryMsg, TimestampResponse,
    TotalCapResponse, UnlockableResponse,
};
use terra_deposit_withdraw::state::State;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(State), &out_dir);
    export_schema(&schema_for!(InfoResponse), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(BalanceResponse), &out_dir);
    export_schema(&schema_for!(TotalCapResponse), &out_dir);
    export_schema(&schema_for!(ActivatableResponse), &out_dir);
    export_schema(&schema_for!(ClaimableResponse), &out_dir);
    export_schema(&schema_for!(PermissionResponse), &out_dir);
    export_schema(&schema_for!(UnlockableResponse), &out_dir);
    export_schema(&schema_for!(TimestampResponse), &out_dir);
}
