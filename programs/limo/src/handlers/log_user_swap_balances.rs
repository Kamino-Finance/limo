use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::Mint;

use crate::{utils::constraints::get_token_account_checked, UserSwapBalances};

pub fn handler_log_user_swap_balances(ctx: Context<LogUserSwapBalances>) -> Result<()> {
    let lamports_balance = ctx.accounts.maker.lamports();

    let input_balance = if ctx.accounts.input_ta.data_len() > 0 {
        let input_token_account = get_token_account_checked(
            &ctx.accounts.input_ta.to_account_info(),
            &ctx.accounts.input_mint.key(),
            &ctx.accounts.maker.key(),
        )?;

        input_token_account.amount
    } else {
        0
    };

    let output_balance = if ctx.accounts.output_ta.data_len() > 0 {
        let output_token_account = get_token_account_checked(
            &ctx.accounts.output_ta.to_account_info(),
            &ctx.accounts.output_mint.key(),
            &ctx.accounts.maker.key(),
        )?;

        output_token_account.amount
    } else {
        0
    };

    emit_cpi!(UserSwapBalances {
        user_lamports: lamports_balance,
        input_ta_balance: input_balance,
        output_ta_balance: output_balance,
    });

    Ok(())
}

#[event_cpi]
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
