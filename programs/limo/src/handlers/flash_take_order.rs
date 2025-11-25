use std::cmp::min;

use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
        sysvar::instructions::get_instruction_relative,
    },
    Accounts,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use express_relay::{program::ExpressRelay, state::ExpressRelayMetadata};
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    global_seeds,
    instruction::{FlashTakeOrderEnd, FlashTakeOrderStart},
    intermediary_seeds,
    operations::{
        self, flash_pay_order_output, validate_pda_authority_balance_and_update_accounting,
    },
    seeds::{self, GLOBAL_AUTH, INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT},
    state::{GlobalConfig, Order, TakeOrderEffects},
    token_operations::{
        close_ata_accounts_with_signer_seeds,
        initialize_intermediary_token_account_with_signer_seeds,
        native_transfer_from_authority_to_user, native_transfer_from_user_to_account,
        transfer_from_user_to_token_account, transfer_from_vault_to_token_account,
    },
    utils::{
        constraints::{
            check_permission_express_relay_and_get_fees, is_counterparty_matching, is_wsol,
            token_2022::validate_token_extensions, verify_ata,
        },
        flash_ixs,
    },
    LimoError, OrderDisplay,
};

fn handler_checks(ctx: &Context<FlashTakeOrder>) -> Result<()> {
    validate_token_extensions(
        &ctx.accounts.input_mint.to_account_info(),
        vec![&ctx.accounts.taker_input_ata.to_account_info()],
        false,
    )?;
    if let Some(maker_output_ata_account) = ctx.accounts.maker_output_ata.as_ref() {
        validate_token_extensions(
            &ctx.accounts.output_mint.to_account_info(),
            vec![
                &ctx.accounts.taker_output_ata.to_account_info(),
                &maker_output_ata_account.to_account_info(),
            ],
            false,
        )?;
    } else {
        validate_token_extensions(
            &ctx.accounts.output_mint.to_account_info(),
            vec![&ctx.accounts.taker_output_ata.to_account_info()],
            false,
        )?;
    }

    let instruction_sysvar_account = ctx.accounts.sysvar_instructions.to_account_info();
    let current_ix_progrm_id = get_instruction_relative(0, &instruction_sysvar_account)?.program_id;

    require!(current_ix_progrm_id == crate::ID, LimoError::CPINotAllowed);
    require!(
        get_stack_height() <= TRANSACTION_LEVEL_STACK_HEIGHT,
        LimoError::CPINotAllowed
    );

    if let Some(maker_output_ata_account) = ctx.accounts.maker_output_ata.as_ref() {
        verify_ata(
            &ctx.accounts.maker.key(),
            &ctx.accounts.output_mint.key(),
            &maker_output_ata_account.key(),
            &ctx.accounts.output_token_program.key(),
        )?;
    } else {
        require!(
            is_wsol(&ctx.accounts.output_mint.key()),
            LimoError::MakerOutputAtaRequired
        );
    }

    Ok(())
}

pub fn handler_start(
    ctx: Context<FlashTakeOrder>,
    input_amount: u64,
    min_output_amount: u64,
    tip_amount_permissionless_taking: u64,
) -> Result<()> {
    handler_checks(&ctx)?;

    let pay: FlashTakeOrderEnd = flash_ixs::ensure_second_ix_match(
        &ctx.accounts.sysvar_instructions,
        &ctx.accounts.input_mint.key(),
        &ctx.accounts.output_mint.key(),
    )?;

    require_eq!(
        input_amount,
        pay.input_amount,
        LimoError::FlashIxsArgsMismatch
    );
    require_eq!(
        min_output_amount,
        pay.min_output_amount,
        LimoError::FlashIxsArgsMismatch
    );
    require_eq!(
        tip_amount_permissionless_taking,
        pay.tip_amount_permissionless_taking,
        LimoError::FlashIxsArgsMismatch
    );

    let order = &mut ctx.accounts.order.load_mut()?;
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    let TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker: _,
    } = operations::flash_withdraw_order_input(order, input_amount, min_output_amount)?;

    let gc = ctx.accounts.global_config.key();
    let seeds: &[&[u8]] = global_seeds!(global_config.pda_authority_bump as u8, &gc);

    transfer_from_vault_to_token_account(
        ctx.accounts.taker_input_ata.to_account_info(),
        ctx.accounts.input_vault.to_account_info(),
        ctx.accounts.pda_authority.to_account_info(),
        ctx.accounts.input_mint.to_account_info(),
        ctx.accounts.input_token_program.to_account_info(),
        seeds,
        input_to_send_to_taker,
        ctx.accounts.input_mint.decimals,
    )?;

    order.flash_start_taker_output_balance = ctx.accounts.taker_output_ata.amount;

    Ok(())
}

pub fn handler_end(
    ctx: Context<FlashTakeOrder>,
    input_amount: u64,
    min_output_amount: u64,
    tip_amount_permissionless_taking: u64,
) -> Result<()> {
    handler_checks(&ctx)?;

    let withdraw: FlashTakeOrderStart = flash_ixs::ensure_first_ix_match(
        &ctx.accounts.sysvar_instructions,
        &ctx.accounts.input_mint.key(),
        &ctx.accounts.output_mint.key(),
    )?;

    require_eq!(
        input_amount,
        withdraw.input_amount,
        LimoError::FlashIxsArgsMismatch
    );
    require_eq!(
        min_output_amount,
        withdraw.min_output_amount,
        LimoError::FlashIxsArgsMismatch
    );
    require_eq!(
        tip_amount_permissionless_taking,
        withdraw.tip_amount_permissionless_taking,
        LimoError::FlashIxsArgsMismatch
    );

    let global_config = &mut ctx.accounts.global_config.load_mut()?;
    let is_filled_by_per = ctx.accounts.permission.is_some();

    let (is_order_permissionless, order_counterparty) = {
        let order = &ctx.accounts.order.load()?;
        (order.permissionless != 0, order.counterparty)
    };

    let tip = check_permission_and_get_tip(
        &ctx,
        &order_counterparty,
        tip_amount_permissionless_taking,
        is_order_permissionless,
        is_filled_by_per,
    )?;

    let order = &mut ctx.accounts.order.load_mut()?;

    let TakeOrderEffects {
        input_to_send_to_taker: _,
        output_to_send_to_maker,
    } = call_operations_and_get_effects(
        &ctx,
        global_config,
        order,
        input_amount,
        min_output_amount,
        tip,
    )?;

    send_output_token_amount(&ctx, global_config, output_to_send_to_maker)?;

    tip_transfer_and_validation(&ctx, global_config, tip, is_filled_by_per)?;

    order.flash_start_taker_output_balance = 0;

    emit_cpi!(OrderDisplay {
        initial_input_amount: order.initial_input_amount,
        expected_output_amount: order.expected_output_amount,
        remaining_input_amount: order.remaining_input_amount,
        filled_output_amount: order.filled_output_amount,
        tip_amount: order.tip_amount,
        number_of_fills: order.number_of_fills,
        on_event_output_amount_filled: output_to_send_to_maker,
        on_event_tip_amount: tip,
        order_type: order.order_type,
        status: order.status,
        last_updated_timestamp: order.last_updated_timestamp,
    });

    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct FlashTakeOrder<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(mut,
        address = order.load()?.maker
    )]
    pub maker: AccountInfo<'info>,

    #[account(
        mut,
        has_one = pda_authority,
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(mut)]
    pub pda_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = global_config,
        has_one = input_mint,
        has_one = output_mint
    )]
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
        seeds = [seeds::ESCROW_VAULT, global_config.key().as_ref(), input_mint.key().as_ref()],
        bump = order.load()?.in_vault_bump,
        token::mint = input_mint,
        token::authority = pda_authority
    )]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = input_mint,
        token::authority = taker
    )]
    pub taker_input_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = output_mint,
        token::authority = taker
    )]
    pub taker_output_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        seeds = [INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT, order.key().as_ref()],
        bump
    )]
    pub intermediary_output_token_account: Option<UncheckedAccount<'info>>,

    #[account(mut,
        token::mint = output_mint,
        token::authority = maker
    )]
    pub maker_output_ata: Option<Box<InterfaceAccount<'info, TokenAccount>>>,

    #[account(address = express_relay::ID)]
    pub express_relay: Program<'info, ExpressRelay>,

    #[account(seeds = [express_relay::state::SEED_METADATA], bump, seeds::program = express_relay.key())]
    pub express_relay_metadata: Account<'info, ExpressRelayMetadata>,

    #[account(address = SysInstructions::id())]
    pub sysvar_instructions: AccountInfo<'info>,

    pub permission: Option<AccountInfo<'info>>,

    #[account(seeds = [express_relay::state::SEED_CONFIG_ROUTER, pda_authority.key().as_ref()], bump, seeds::program = express_relay.key())]
    pub config_router: UncheckedAccount<'info>,

    pub input_token_program: Interface<'info, TokenInterface>,
    pub output_token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
}

fn check_permission_and_get_tip(
    ctx: &Context<FlashTakeOrder>,
    order_counterparty: &Pubkey,
    tip_amount_permissionless_taking: u64,
    is_order_permissionless: bool,
    is_filled_by_per: bool,
) -> Result<u64> {
    if !is_order_permissionless && !is_filled_by_per {
        return err!(LimoError::PermissionRequiredPermissionlessNotEnabled);
    }

    if !is_counterparty_matching(order_counterparty, &ctx.accounts.taker.key()) {
        return err!(LimoError::CounterpartyDisallowed);
    }

    let tip = if let Some(permission_account) = ctx.accounts.permission.as_ref() {
        check_permission_express_relay_and_get_fees(
            &ctx.accounts.sysvar_instructions,
            permission_account,
            &ctx.accounts.pda_authority,
            &ctx.accounts.config_router,
            &ctx.accounts.express_relay_metadata.to_account_info(),
            &ctx.accounts.express_relay,
            ctx.accounts.order.key(),
        )?
    } else {
        tip_amount_permissionless_taking
    };

    Ok(tip)
}

fn call_operations_and_get_effects(
    ctx: &Context<FlashTakeOrder>,
    global_config: &mut GlobalConfig,
    order: &mut Order,
    input_amount: u64,
    min_output_amount: u64,
    tip: u64,
) -> Result<TakeOrderEffects> {
    let clock = Clock::get()?;

    let taker_output_ata_balance_diff =
        ctx.accounts.taker_output_ata.amount - order.flash_start_taker_output_balance;

    let output_amount = if taker_output_ata_balance_diff == 0 {
        min_output_amount
    } else {
        min(taker_output_ata_balance_diff, min_output_amount)
    };

    let take_order_effects = flash_pay_order_output(
        global_config,
        order,
        input_amount,
        output_amount,
        tip,
        clock.unix_timestamp,
    )?;

    Ok(take_order_effects)
}

fn send_output_token_amount(
    ctx: &Context<FlashTakeOrder>,
    global_config: &GlobalConfig,
    output_to_send_to_maker: u64,
) -> Result<()> {
    let gc = ctx.accounts.global_config.key();
    let seeds: &[&[u8]] = global_seeds!(global_config.pda_authority_bump as u8, &gc);

    let output_is_wsol = is_wsol(&ctx.accounts.output_mint.key());
    let output_destination_token_account = if output_is_wsol {
        let intermediary_output_token_account = ctx
            .accounts
            .intermediary_output_token_account
            .as_ref()
            .ok_or(LimoError::IntermediaryOutputTokenAccountRequired)?;
        let order_key = ctx.accounts.order.key();
        let token_account_signer_seeds: &[&[u8]] =
            intermediary_seeds!(ctx.bumps.intermediary_output_token_account, &order_key);
        initialize_intermediary_token_account_with_signer_seeds(
            intermediary_output_token_account.to_account_info().clone(),
            ctx.accounts.output_mint.to_account_info(),
            ctx.accounts.output_token_program.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            token_account_signer_seeds,
            seeds,
        )?;

        intermediary_output_token_account.to_account_info()
    } else {
        ctx.accounts
            .maker_output_ata
            .as_ref()
            .ok_or(LimoError::MakerOutputAtaRequired)?
            .to_account_info()
    };

    transfer_from_user_to_token_account(
        ctx.accounts.taker_output_ata.to_account_info(),
        output_destination_token_account.clone(),
        ctx.accounts.taker.to_account_info(),
        ctx.accounts.output_mint.to_account_info(),
        ctx.accounts.output_token_program.to_account_info(),
        output_to_send_to_maker,
        ctx.accounts.output_mint.decimals,
    )?;

    if output_is_wsol {
        close_ata_accounts_with_signer_seeds(
            output_destination_token_account,
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.output_token_program.to_account_info(),
            seeds,
        )?;
        native_transfer_from_authority_to_user(
            ctx.accounts.pda_authority.to_account_info(),
            ctx.accounts.maker.to_account_info(),
            seeds,
            output_to_send_to_maker,
        )?;
    }

    Ok(())
}

fn tip_transfer_and_validation(
    ctx: &Context<FlashTakeOrder>,
    global_config: &mut GlobalConfig,
    tip: u64,
    is_filled_by_per: bool,
) -> Result<()> {
    if !is_filled_by_per {
        native_transfer_from_user_to_account(
            ctx.accounts.taker.to_account_info(),
            ctx.accounts.pda_authority.to_account_info(),
            tip,
        )?;
    }

    let pda_authority_balance = ctx.accounts.pda_authority.lamports();
    validate_pda_authority_balance_and_update_accounting(
        global_config,
        pda_authority_balance,
        tip,
    )?;

    Ok(())
}
