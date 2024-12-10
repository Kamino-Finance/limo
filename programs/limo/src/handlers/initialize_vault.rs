use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{seeds, state::GlobalConfig, LimoError};

pub fn handler_initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
    msg!(
        "Initializing vault for global config {} with mint {}",
        ctx.accounts.global_config.key(),
        ctx.accounts.mint.key(),
    );

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(mut,
        has_one = admin_authority @ LimoError::InvalidAdminAuthority,
        has_one = pda_authority @ LimoError::InvalidPdaAuthority,
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,
    #[account(
        seeds = [seeds::GLOBAL_AUTH, global_config.key().as_ref()],
        bump = global_config.load()?.pda_authority_bump as u8,
    )]
    pub pda_authority: AccountInfo<'info>,

    #[account(
        mint::token_program = token_program,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init,
        seeds = [seeds::ESCROW_VAULT, global_config.key().as_ref(), mint.key().as_ref()],
        bump,
        payer = admin_authority,
        token::mint = mint,
        token::authority = pda_authority,
        token::token_program = token_program,
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
