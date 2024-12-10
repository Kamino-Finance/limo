use anchor_lang::{prelude::*, Accounts};

use crate::state::GlobalConfig;

pub fn handler_initialize_global_config(ctx: Context<InitializeGlobalConfig>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_init()?;

    let admin_authority = ctx.accounts.admin_authority.key();
    let pda_authority = ctx.accounts.pda_authority.key();
    let pda_bump: u64 = ctx.bumps.pda_authority.into();
    let pda_authority_previous_lamports_balance = ctx.accounts.pda_authority.lamports();

    crate::operations::initialize_global_config(
        global_config,
        admin_authority,
        pda_authority,
        pda_bump,
        pda_authority_previous_lamports_balance,
    );

    msg!(
        "Initializing global config with global authority {} and bump {}",
        ctx.accounts.pda_authority.key(),
        global_config.pda_authority_bump
    );

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeGlobalConfig<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(mut,
        seeds = [b"authority".as_ref(), global_config.key().as_ref()],
        bump)]
    pub pda_authority: AccountInfo<'info>,

    #[account(zero)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
}
