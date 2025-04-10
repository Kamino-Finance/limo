use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::Mint;
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    instruction::{LogUserSwapBalancesEnd, LogUserSwapBalancesStart},
    seeds,
    utils::{
        constraints::get_token_account_checked, consts::USER_SWAP_BALANCE_STATE_SIZE,
        log_user_swap_balance_introspection,
    },
    GetBalancesCheckedResult, UserSwapBalanceDiffs, UserSwapBalancesState,
};

pub fn handler_log_user_swap_balances_start(
    ctx: Context<LogUserSwapBalancesStartContext>,
) -> Result<()> {
    let swap_program_id = ctx.accounts.base_accounts.swap_program_id.key();
    log_user_swap_balance_introspection::ensure_end_ix_match::<LogUserSwapBalancesEnd>(
        &ctx.accounts.sysvar_instructions,
        &swap_program_id,
    )?;

    let balances = get_balances_checked(&ctx.accounts.base_accounts)?;

    let user_swap_balance_state = &mut ctx.accounts.user_swap_balance_state.load_init()?;
    user_swap_balance_state.user_lamports = balances.lamports_balance;
    user_swap_balance_state.input_ta_balance = balances.input_balance;
    user_swap_balance_state.output_ta_balance = balances.output_balance;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn handler_log_user_swap_balances_end(
    ctx: Context<LogUserSwapBalancesEndContext>,
    simulated_swap_amount_out: u64,
    simulated_ts: u64,
    minimum_amount_out: u64,
    swap_amount_in: u64,
    simulated_amount_out_next_best: u64,
    aggregator: u8,
    next_best_aggregator: u8,
) -> Result<()> {
    let swap_program_id = ctx.accounts.base_accounts.swap_program_id.key();
    log_user_swap_balance_introspection::ensure_start_ix_match::<LogUserSwapBalancesStart>(
        &ctx.accounts.sysvar_instructions,
        &swap_program_id,
    )?;

    let balances = get_balances_checked(&ctx.accounts.base_accounts)?;

    {
        let user_swap_balance_state = &mut ctx.accounts.user_swap_balance_state.load()?;

        emit_cpi!(UserSwapBalanceDiffs {
            user_lamports_before: user_swap_balance_state.user_lamports,
            input_ta_balance_before: user_swap_balance_state.input_ta_balance,
            output_ta_balance_before: user_swap_balance_state.output_ta_balance,
            user_lamports_after: balances.lamports_balance,
            input_ta_balance_after: balances.input_balance,
            output_ta_balance_after: balances.output_balance,
            swap_program: swap_program_id,
            simulated_swap_amount_out,
            simulated_ts,
            minimum_amount_out,
            swap_amount_in,
            simulated_amount_out_next_best,
            aggregator,
            next_best_aggregator,
        });
    }

    ctx.accounts
        .user_swap_balance_state
        .close(ctx.accounts.base_accounts.maker.to_account_info().clone())?;

    Ok(())
}

#[derive(Accounts)]
pub struct LogUserSwapBalances<'info> {
    #[account()]
    pub maker: Signer<'info>,

    pub input_mint: Box<InterfaceAccount<'info, Mint>>,

    pub output_mint: Box<InterfaceAccount<'info, Mint>>,

    pub input_ta: UncheckedAccount<'info>,

    pub output_ta: UncheckedAccount<'info>,

    pub pda_referrer: Option<AccountInfo<'info>>,

    pub swap_program_id: AccountInfo<'info>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct LogUserSwapBalancesStartContext<'info> {
    base_accounts: LogUserSwapBalances<'info>,

    #[account(
        init,
        seeds = [seeds::USER_SWAP_BALANCES_SEED, base_accounts.maker.key().as_ref()],
        bump,
        payer = base_accounts.maker,
        space = USER_SWAP_BALANCE_STATE_SIZE + 8
    )]
    pub user_swap_balance_state: AccountLoader<'info, UserSwapBalancesState>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = SysInstructions::id())]
    pub sysvar_instructions: AccountInfo<'info>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct LogUserSwapBalancesEndContext<'info> {
    base_accounts: LogUserSwapBalances<'info>,

    #[account(mut,
        seeds = [seeds::USER_SWAP_BALANCES_SEED, base_accounts.maker.key().as_ref()],
        bump,
    )]
    pub user_swap_balance_state: AccountLoader<'info, UserSwapBalancesState>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,

    #[account(address = SysInstructions::id())]
    pub sysvar_instructions: AccountInfo<'info>,
}

pub fn get_balances_checked(ctx: &LogUserSwapBalances) -> Result<GetBalancesCheckedResult> {
    let lamports_balance = ctx.maker.lamports();

    let input_balance = if ctx.input_ta.data_len() > 0 {
        let input_token_account = get_token_account_checked(
            &ctx.input_ta.to_account_info(),
            &ctx.input_mint.key(),
            &ctx.maker.key(),
        )?;

        input_token_account.amount
    } else {
        0
    };

    let output_balance = if ctx.output_ta.data_len() > 0 {
        let output_token_account = get_token_account_checked(
            &ctx.output_ta.to_account_info(),
            &ctx.output_mint.key(),
            &ctx.maker.key(),
        )?;

        output_token_account.amount
    } else {
        0
    };

    Ok(GetBalancesCheckedResult {
        lamports_balance,
        input_balance,
        output_balance,
    })
}
