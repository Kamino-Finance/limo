use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::UserSwapBalances;

pub fn handler_log_user_swap_balances(ctx: Context<LogUserSwapBalances>) -> Result<()> {
    let lamports_balance = ctx.accounts.maker.lamports();
    let output_balance = ctx.accounts.output_ta.amount;
    let input_balance = ctx.accounts.input_ta.amount;

    msg!(
        "Balances for user {}, lamports {}, input_amount {}, input_mint {}, output_amount {}, output_mint {}",
        ctx.accounts.maker.key(),
        lamports_balance,
        input_balance,
        ctx.accounts.input_mint.key(),
        output_balance,
        ctx.accounts.output_mint.key(),
    );

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

    #[account(mut,
        token::mint = input_mint,
        token::authority = maker
    )]
    pub input_ta: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
      token::mint = output_mint,
      token::authority = maker
  )]
    pub output_ta: Box<InterfaceAccount<'info, TokenAccount>>,
}
