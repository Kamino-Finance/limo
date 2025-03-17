use std::num::TryFromIntError;

use anchor_lang::prelude::*;

pub mod handlers;
pub mod operations;
pub mod seeds;
pub mod state;
pub mod token_operations;
pub mod utils;
use num_enum::TryFromPrimitive;
use thiserror::Error;
use utils::{
    constraints::{
        create_new_orders_disabled, emergency_mode_disabled, flash_taking_orders_disabled,
        taking_orders_disabled,
    },
    consts::UPDATE_GLOBAL_CONFIG_BYTE_SIZE,
};

use crate::handlers::*;
pub use crate::state::*;

#[cfg(feature = "staging")]
declare_id!("sLim6uuAFC8kAWstWpu1r6oJD4T8VR6raukSpU2Zim7");

#[cfg(not(feature = "staging"))]
declare_id!("LiMoM9rMhrdYrfzUCxQppvxCSG1FcrUK9G8uLq4A1GF");

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "Kamino Liquidity Integration & Matching Orders (LIMO)",
    project_url: "https://swap.kamino.finance/",
    contacts: "email:security@kamino.finance",
    policy: "https://github.com/Kamino-Finance/audits/blob/master/docs/SECURITY.md",

       preferred_languages: "en",
    auditors: "OtterSec, Offside Labs, Sec3"
}

#[program]
pub mod limo {

    use super::*;

    pub fn initialize_global_config(ctx: Context<InitializeGlobalConfig>) -> Result<()> {
        handlers::initialize_global_config::handler_initialize_global_config(ctx)
    }

    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        handlers::initialize_vault::handler_initialize_vault(ctx)
    }

    #[access_control(create_new_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn create_order(
        ctx: Context<CreateOrder>,
        input_amount: u64,
        output_amount: u64,
        order_type: u8,
    ) -> Result<()> {
        handlers::create_order::handler_create_order(ctx, input_amount, output_amount, order_type)
    }

    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn close_order_and_claim_tip(ctx: Context<CloseOrderAndClaimTip>) -> Result<()> {
        handlers::close_order_and_claim_tip::handler_close_order_and_claim_tip(ctx)
    }

    #[access_control(taking_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn take_order(
        ctx: Context<TakeOrder>,
        input_amount: u64,
        min_output_amount: u64,
        tip_amount_permissionless_taking: u64,
    ) -> Result<()> {
        handlers::take_order::handler_take_order(
            ctx,
            input_amount,
            min_output_amount,
            tip_amount_permissionless_taking,
        )
    }

    #[access_control(taking_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(flash_taking_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn flash_take_order_start(
        ctx: Context<FlashTakeOrder>,
        input_amount: u64,
        min_output_amount: u64,
        tip_amount_permissionless_taking: u64,
    ) -> Result<()> {
        handlers::flash_take_order::handler_start(
            ctx,
            input_amount,
            min_output_amount,
            tip_amount_permissionless_taking,
        )
    }

    #[access_control(taking_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(flash_taking_orders_disabled(&ctx.accounts.global_config))]
    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn flash_take_order_end(
        ctx: Context<FlashTakeOrder>,
        input_amount: u64,
        min_output_amount: u64,
        tip_amount_permissionless_taking: u64,
    ) -> Result<()> {
        handlers::flash_take_order::handler_end(
            ctx,
            input_amount,
            min_output_amount,
            tip_amount_permissionless_taking,
        )
    }

    pub fn update_global_config(
        ctx: Context<UpdateGlobalConfig>,
        mode: u16,
        value: [u8; UPDATE_GLOBAL_CONFIG_BYTE_SIZE],
    ) -> Result<()> {
        handlers::update_global_config::handler_update_global_config(ctx, mode, &value)
    }

    pub fn update_global_config_admin(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
        handlers::update_global_config_admin::handler_update_global_config_admin(ctx)
    }

    #[access_control(emergency_mode_disabled(&ctx.accounts.global_config))]
    pub fn withdraw_host_tip(ctx: Context<WithdrawHostTip>) -> Result<()> {
        handlers::withdraw_host_tip::withdraw_host_tip(ctx)
    }

    pub fn log_user_swap_balances_start(
        ctx: Context<LogUserSwapBalancesStart>,
        swap_program_id: Pubkey,
    ) -> Result<()> {
        handlers::log_user_swap_balances::handler_log_user_swap_balances_start(ctx, swap_program_id)
    }

    pub fn log_user_swap_balances_end(
        ctx: Context<LogUserSwapBalancesEnd>,
        swap_program_id: Pubkey,
    ) -> Result<()> {
        handlers::log_user_swap_balances::handler_log_user_swap_balances_end(ctx, swap_program_id)
    }
}

#[error_code]
#[derive(Error, PartialEq, Eq, TryFromPrimitive)]
pub enum LimoError {
    #[msg("Order can't be canceled")]
    OrderCanNotBeCanceled,

    #[msg("Order not active")]
    OrderNotActive,

    #[msg("Invalid admin authority")]
    InvalidAdminAuthority,

    #[msg("Invalid pda authority")]
    InvalidPdaAuthority,

    #[msg("Invalid config option")]
    InvalidConfigOption,

    #[msg("Order owner account is not the order owner")]
    InvalidOrderOwner,

    #[msg("Out of range integral conversion attempted")]
    OutOfRangeIntegralConversion,

    #[msg("Invalid boolean flag, valid values are 0 and 1")]
    InvalidFlag,

    #[msg("Mathematical operation with overflow")]
    MathOverflow,

    #[msg("Order input amount invalid")]
    OrderInputAmountInvalid,

    #[msg("Order output amount invalid")]
    OrderOutputAmountInvalid,

    #[msg("Host fee bps must be between 0 and 10000")]
    InvalidHostFee,

    #[msg("Conversion between integers failed")]
    IntegerOverflow,

    #[msg("Tip balance less than accounted tip")]
    InvalidTipBalance,

    #[msg("Tip transfer amount is less than expected")]
    InvalidTipTransferAmount,

    #[msg("Host tup amount is less than accounted for")]
    InvalidHostTipBalance,

    #[msg("Order within flash operation - all otehr actions are blocked")]
    OrderWithinFlashOperation,

    #[msg("CPI not allowed")]
    CPINotAllowed,

    #[msg("Flash take_order is blocked")]
    FlashTakeOrderBlocked,

    #[msg("Some unexpected instructions are present in the tx. Either before or after the flash ixs, or some ix target the same program between")]
    FlashTxWithUnexpectedIxs,

    #[msg("Flash ixs initiated without the closing ix in the transaction")]
    FlashIxsNotEnded,

    #[msg("Flash ixs ended without the starting ix in the transaction")]
    FlashIxsNotStarted,

    #[msg("Some accounts differ between the two flash ixs")]
    FlashIxsAccountMismatch,

    #[msg("Some args differ between the two flash ixs")]
    FlashIxsArgsMismatch,

    #[msg("Order is not within flash operation")]
    OrderNotWithinFlashOperation,

    #[msg("Emergency mode is enabled")]
    EmergencyModeEnabled,

    #[msg("Creating new ordersis blocked")]
    CreatingNewOrdersBlocked,

    #[msg("Orders taking is blocked")]
    OrderTakingBlocked,

    #[msg("Order input amount larger than the remaining")]
    OrderInputAmountTooLarge,

    #[msg("Permissionless order taking not enabled, please provide permission account")]
    PermissionRequiredPermissionlessNotEnabled,

    #[msg("Permission address does not match order address")]
    PermissionDoesNotMatchOrder,

    #[msg("Invalid ata address")]
    InvalidAtaAddress,

    #[msg("Maker output ata required when output mint is not WSOL")]
    MakerOutputAtaRequired,

    #[msg("Intermediary output token account required when output mint is WSOL")]
    IntermediaryOutputTokenAccountRequired,

    #[msg("Not enough balance for rent")]
    NotEnoughBalanceForRent,

    #[msg("Order can not be closed - Not enough time passed since last update")]
    NotEnoughTimePassedSinceLastUpdate,

    #[msg("Order input and output mints are the same")]
    OrderSameMint,

    #[msg("Mint has a token (2022) extension that is not supported")]
    UnsupportedTokenExtension,

    #[msg("Can't have an spl token mint with a t22 account")]
    InvalidTokenAccount,

    #[msg("The order type is invalid")]
    OrderTypeInvalid,

    #[msg("Token account is not initialized")]
    UninitializedTokenAccount,

    #[msg("Account is not owned by the token program")]
    InvalidTokenAccountOwner,

    #[msg("Account is not a valid token account")]
    InvalidAccount,

    #[msg("Token account has incorrect mint")]
    InvalidTokenMint,

    #[msg("Token account has incorrect authority")]
    InvalidTokenAuthority,
}

impl From<TryFromIntError> for LimoError {
    fn from(_: TryFromIntError) -> LimoError {
        LimoError::OutOfRangeIntegralConversion
    }
}

pub type LimoResult<T> = std::result::Result<T, LimoError>;
