use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use solana_program::{program::invoke, system_instruction};

use crate::{
    operations, seeds,
    state::{GlobalConfig, Order},
    token_operations::transfer_from_user_to_token_account,
    utils::constraints::token_2022::validate_token_extensions,
    LimoError, OrderDisplay, OrderType,
};

pub fn handler_create_order(
    ctx: Context<CreateOrder>,
    input_amount: u64,
    output_amount: u64,
    order_type: u8,
) -> Result<()> {
    validate_token_extensions(
        &ctx.accounts.input_mint.to_account_info(),
        vec![&ctx.accounts.maker_ata.to_account_info()],
    )?;
    validate_token_extensions(&ctx.accounts.output_mint.to_account_info(), vec![])?;

    require!(input_amount > 0, LimoError::OrderInputAmountInvalid);
    require!(output_amount > 0, LimoError::OrderOutputAmountInvalid);
    require!(
        ctx.accounts.input_mint.key() != ctx.accounts.output_mint.key(),
        LimoError::OrderSameMint
    );
    OrderType::try_from(order_type).map_err(|_| LimoError::OrderTypeInvalid)?;

    let order = &mut ctx.accounts.order.load_init()?;
    let clock = Clock::get()?;

    operations::create_order(
        order,
        ctx.accounts.global_config.key(),
        ctx.accounts.maker.key(),
        input_amount,
        output_amount,
        ctx.accounts.input_mint.key(),
        ctx.accounts.output_mint.key(),
        ctx.accounts.input_token_program.key(),
        ctx.accounts.output_token_program.key(),
        order_type,
        ctx.bumps.input_vault,
        clock.unix_timestamp,
    )?;

    transfer_from_user_to_token_account(
        ctx.accounts.maker_ata.to_account_info(),
        ctx.accounts.input_vault.to_account_info(),
        ctx.accounts.maker.to_account_info(),
        ctx.accounts.input_mint.to_account_info(),
        ctx.accounts.input_token_program.to_account_info(),
        input_amount,
        ctx.accounts.input_mint.decimals,
    )?;

    let gc_state = ctx.accounts.global_config.load()?;
    let lamports = gc_state.ata_creation_cost + gc_state.txn_fee_cost;
    drop(gc_state);
    if lamports > 0 {
        let maker = ctx.accounts.maker.key();
        let gc = ctx.accounts.global_config.key();
        let ixn = system_instruction::transfer(&maker, &gc, lamports);

        invoke(
            &ixn,
            &[
                ctx.accounts.maker.to_account_info().clone(),
                ctx.accounts.global_config.to_account_info().clone(),
                ctx.accounts.system_program.to_account_info().clone(),
            ],
        )?;
    }

    msg!(
        "Created order {}, input_amount {}, input_mint {}, output_amount {}, output_mint {}",
        ctx.accounts.order.key(),
        input_amount,
        ctx.accounts.input_mint.key(),
        output_amount,
        ctx.accounts.output_mint.key(),
    );

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
pub struct CreateOrder<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(mut, has_one = pda_authority)]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account()]
    pub pda_authority: AccountInfo<'info>,

    #[account(zero)]
    pub order: AccountLoader<'info, Order>,

    #[account(
        mint::token_program = input_token_program,
    )]
    pub input_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mint::token_program = output_token_program,
    )]
    pub output_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        token::mint = input_mint,
        token::authority = maker
    )]
    pub maker_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        seeds = [seeds::ESCROW_VAULT, global_config.key().as_ref(), input_mint.key().as_ref()],
        bump,
        token::mint = input_mint,
        token::authority = pda_authority
    )]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub input_token_program: Interface<'info, TokenInterface>,
    pub output_token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
