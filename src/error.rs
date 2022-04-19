use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Standard Error")]
    Std(#[from] StdError),

    #[error("Overflow")]
    OverflowError(#[from] OverflowError),

    #[error("Conversion Overflow")]
    ConversionOverflowError(#[from] ConversionOverflowError),

    #[error("Divide By Zero")]
    DivideByZeroError {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Insufficient")]
    Insufficient {},

    #[error("Invalidate Input")]
    Invalidate {},

    #[error("Locked")]
    Locked {},

    #[error("Paused")]
    Paused {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
