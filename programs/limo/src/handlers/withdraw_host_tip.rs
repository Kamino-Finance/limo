use anchor_lang::{prelude::*, Accounts};

use crate::{
    global_seeds, operations, seeds::GLOBAL_AUTH,
    token_operations::lamports_transfer_from_authority_to_account, GlobalConfig,
};

pub fn withdraw_host_tip(ctx: Context<WithdrawHostTip>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    let pda_authority_balance = ctx.accounts.pda_authority.lamports();
    let host_tip_to_withdraw = operations::withdraw_host_tip(global_config, pda_authority_balance)?;

    let pda_authority_bump = global_config.pda_authority_bump as u8;
    let gc = ctx.accounts.global_config.key();
    let seeds: &[&[u8]] = global_seeds!(pda_authority_bump, &gc);

    if host_tip_to_withdraw > 0 {
        lamports_transfer_from_authority_to_account(
            ctx.accounts.admin_authority.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            seeds,
            host_tip_to_withdraw,
        )?;
    }

    global_config.pda_authority_previous_lamports_balance = ctx.accounts.pda_authority.lamports();

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawHostTip<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(
        mut,
        has_one = pda_authority,
        has_one = admin_authority
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(mut)]
    pub pda_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
