use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
        sysvar::instructions::get_instruction_relative,
    },
    Accounts, Discriminator,
};
use anchor_spl::token_interface::TokenAccount;
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    instruction::{AssertUserSwapBalancesEnd, AssertUserSwapBalancesStart},
    operations::validate_user_swap_balances,
    seeds,
    utils::{assert_user_swap_balance_introspection, consts::USER_SWAP_BALANCE_STATE_SIZE},
    GetBalancesCheckedResult, LimoError, UserSwapBalancesState,
};

macro_rules! get_user_balances_checked {
    ($ctx:expr) => {{
        GetBalancesCheckedResult {
            lamports_balance: $ctx.maker.lamports(),
            input_balance: $ctx.input_ta.amount,
            output_balance: $ctx.output_ta.amount,
        }
    }};
}

macro_rules! check_cpi_not_allowed {
    ($ctx:expr) => {{
        let instruction_sysvar_account = $ctx.accounts.sysvar_instructions.to_account_info();
        let current_ix_program_id =
            get_instruction_relative(0, &instruction_sysvar_account)?.program_id;
        require_keys_eq!(current_ix_program_id, crate::ID, LimoError::CPINotAllowed);
        require!(
            get_stack_height() <= TRANSACTION_LEVEL_STACK_HEIGHT,
            LimoError::CPINotAllowed
        );
    }};
}

pub fn handler_assert_user_swap_balances_start(
    ctx: Context<AssertUserSwapBalancesStartContext>,
) -> Result<()> {
    check_cpi_not_allowed!(ctx);
    assert_user_swap_balance_introspection::ensure_end_ix_match::<AssertUserSwapBalancesEnd>(
        &ctx.accounts.sysvar_instructions,
        &AssertUserSwapBalancesStart::discriminator(),
    )?;

    let balances = get_user_balances_checked!(&ctx.accounts);

    let user_swap_balance_state = &mut ctx.accounts.user_swap_balance_state.load_init()?;
    user_swap_balance_state.user_lamports = balances.lamports_balance;
    user_swap_balance_state.input_ta_balance = balances.input_balance;
    user_swap_balance_state.output_ta_balance = balances.output_balance;

    Ok(())
}

pub fn handler_assert_user_swap_balances_end(
    ctx: Context<AssertUserSwapBalancesEndContext>,
    max_input_amount_change: u64,
    min_output_amount_change: u64,
) -> Result<()> {
    check_cpi_not_allowed!(ctx);
    assert_user_swap_balance_introspection::ensure_start_ix_match::<AssertUserSwapBalancesStart>(
        &ctx.accounts.sysvar_instructions,
        &AssertUserSwapBalancesEnd::discriminator(),
    )?;

    let balances = get_user_balances_checked!(&ctx.accounts);

    {
        let user_swap_balance_state = &mut ctx.accounts.user_swap_balance_state.load()?;
        validate_user_swap_balances(
            user_swap_balance_state,
            balances,
            max_input_amount_change,
            min_output_amount_change,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct AssertUserSwapBalancesStartContext<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        token::authority = maker
    )]
    pub input_ta: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        token::authority = maker
    )]
    pub output_ta: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        seeds = [seeds::ASSERT_SWAP_BALANCES_SEED, maker.key().as_ref()],
        bump,
        payer = maker,
        space = USER_SWAP_BALANCE_STATE_SIZE + 8
    )]
    pub user_swap_balance_state: AccountLoader<'info, UserSwapBalancesState>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = SysInstructions::id())]
    pub sysvar_instructions: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AssertUserSwapBalancesEndContext<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        token::authority = maker
    )]
    pub input_ta: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        token::authority = maker
    )]
    pub output_ta: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        seeds = [seeds::ASSERT_SWAP_BALANCES_SEED, maker.key().as_ref()],
        bump,
        close = maker,
    )]
    pub user_swap_balance_state: AccountLoader<'info, UserSwapBalancesState>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = SysInstructions::id())]
    pub sysvar_instructions: AccountInfo<'info>,
}
