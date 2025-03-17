use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::Mint;

use crate::{
    seeds,
    utils::{constraints::get_token_account_checked, consts::USER_SWAP_BALANCE_STATE_SIZE},
    GetBalancesCheckedResult, UserSwapBalanceDiffs, UserSwapBalancesState,
};

pub fn handler_log_user_swap_balances_start(
    ctx: Context<LogUserSwapBalancesStart>,
    _swap_program_id: Pubkey,
) -> Result<()> {
    let balances = get_balances_checked(&ctx.accounts.base_accounts)?;

    let user_swap_balance_state = &mut ctx.accounts.user_swap_balance_state.load_init()?;
    user_swap_balance_state.user_lamports = balances.lamports_balance;
    user_swap_balance_state.input_ta_balance = balances.input_balance;
    user_swap_balance_state.output_ta_balance = balances.output_balance;

    Ok(())
}

pub fn handler_log_user_swap_balances_end(
    ctx: Context<LogUserSwapBalancesEnd>,
    swap_program_id: Pubkey,
) -> Result<()> {
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
}

#[event_cpi]
#[derive(Accounts)]
pub struct LogUserSwapBalancesStart<'info> {
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
}

#[event_cpi]
#[derive(Accounts)]
pub struct LogUserSwapBalancesEnd<'info> {
    base_accounts: LogUserSwapBalances<'info>,

    #[account(mut,
        seeds = [seeds::USER_SWAP_BALANCES_SEED, base_accounts.maker.key().as_ref()],
        bump,
    )]
    pub user_swap_balance_state: AccountLoader<'info, UserSwapBalancesState>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
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
