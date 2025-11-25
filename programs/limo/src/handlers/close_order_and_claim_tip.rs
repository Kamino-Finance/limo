use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    global_seeds, operations,
    seeds::{self, GLOBAL_AUTH},
    state::Order,
    token_operations::{
        lamports_transfer_from_authority_to_account, transfer_from_vault_to_token_account,
    },
    utils::constraints::token_2022::validate_token_extensions,
    GlobalConfig, OrderDisplay,
};

pub fn handler_close_order_and_claim_tip(ctx: Context<CloseOrderAndClaimTip>) -> Result<()> {
    validate_token_extensions(
        &ctx.accounts.input_mint.to_account_info(),
        vec![&ctx.accounts.maker_input_ata.to_account_info()],
        true,
    )?;
    let order = &mut ctx.accounts.order.load_mut()?;
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    let ts = u64::try_from(Clock::get()?.unix_timestamp).unwrap();

    operations::close_order_and_claim_tip(order, global_config, ts)?;
    let pda_authority_bump = global_config.pda_authority_bump as u8;
    let gc = ctx.accounts.global_config.key();
    let seeds: &[&[u8]] = global_seeds!(pda_authority_bump, &gc);

    if order.remaining_input_amount > 0 {
        transfer_from_vault_to_token_account(
            ctx.accounts.maker_input_ata.to_account_info(),
            ctx.accounts.input_vault.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.input_mint.to_account_info(),
            ctx.accounts.input_token_program.to_account_info(),
            seeds,
            order.remaining_input_amount,
            ctx.accounts.input_mint.decimals,
        )
        .unwrap();
    }

    if order.tip_amount > 0 {
        lamports_transfer_from_authority_to_account(
            ctx.accounts.maker.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            seeds,
            order.tip_amount,
        )?;
    }

    global_config.pda_authority_previous_lamports_balance = ctx.accounts.pda_authority.lamports();

    emit_cpi!(OrderDisplay {
        initial_input_amount: order.initial_input_amount,
        expected_output_amount: order.expected_output_amount,
        remaining_input_amount: order.remaining_input_amount,
        filled_output_amount: order.filled_output_amount,
        tip_amount: order.tip_amount,
        number_of_fills: order.number_of_fills,
        on_event_output_amount_filled: 0,
        on_event_tip_amount: 0,
        order_type: order.order_type,
        status: order.status,
        last_updated_timestamp: order.last_updated_timestamp,
    });

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct CloseOrderAndClaimTip<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(mut,
        has_one = maker,
        has_one = global_config,
        has_one = input_mint,
        has_one = output_mint,
        close = maker
    )]
    pub order: AccountLoader<'info, Order>,

    #[account(
        mut,
        has_one = pda_authority,
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(mut)]
    pub pda_authority: AccountInfo<'info>,

    #[account(
        mint::token_program = input_token_program,
    )]
    pub input_mint: Box<InterfaceAccount<'info, Mint>>,

    pub output_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        token::mint = input_mint,
        token::authority = maker
    )]
    pub maker_input_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        seeds = [seeds::ESCROW_VAULT, global_config.key().as_ref(), input_mint.key().as_ref()],
        bump,
        token::mint = input_mint,
        token::authority = pda_authority
    )]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub input_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
