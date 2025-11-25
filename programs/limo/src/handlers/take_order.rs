use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use express_relay::{program::ExpressRelay, state::ExpressRelayMetadata};
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    global_seeds, intermediary_seeds,
    operations::{self, validate_pda_authority_balance_and_update_accounting},
    seeds::{self, GLOBAL_AUTH, INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT},
    state::{GlobalConfig, Order, TakeOrderEffects},
    token_operations::{
        close_ata_accounts_with_signer_seeds,
        initialize_intermediary_token_account_with_signer_seeds,
        native_transfer_from_authority_to_user, native_transfer_from_user_to_account,
        transfer_from_user_to_token_account, transfer_from_vault_to_token_account,
    },
    utils::constraints::{
        check_permission_express_relay_and_get_fees, is_counterparty_matching, is_wsol,
        token_2022::validate_token_extensions, verify_ata,
    },
    LimoError, OrderDisplay,
};

pub fn handler_take_order(
    ctx: Context<TakeOrder>,
    input_amount: u64,
    min_output_amount: u64,
    tip_amount_permissionless_taking: u64,
) -> Result<()> {
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

    let global_config = &mut ctx.accounts.global_config.load_mut()?;
    let is_filled_by_per = ctx.accounts.permission.is_some();

    let (is_order_permissionless, counterparty) = {
        let order = &ctx.accounts.order.load()?;
        (order.permissionless != 0, order.counterparty)
    };

    let tip = check_permission_and_get_tip(
        &ctx,
        &counterparty,
        tip_amount_permissionless_taking,
        is_order_permissionless,
        is_filled_by_per,
    )?;

    let order = &mut ctx.accounts.order.load_mut()?;
    let clock = Clock::get()?;

    let TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    } = operations::take_order(
        global_config,
        order,
        input_amount,
        tip,
        clock.unix_timestamp,
        min_output_amount,
    )?;

    transfer_output_to_maker_and_input_to_taker(
        &ctx,
        global_config,
        input_to_send_to_taker,
        output_to_send_to_maker,
    )?;

    tip_transfer_and_validation(&ctx, global_config, tip, is_filled_by_per)?;

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
pub struct TakeOrder<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(mut,
        address = order.load()?.maker)]
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
        token::authority = maker,
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

    pub rent: Sysvar<'info, Rent>,

    pub system_program: Program<'info, System>,
}

fn check_permission_and_get_tip(
    ctx: &Context<TakeOrder>,
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

    let tip = if !is_filled_by_per {
        tip_amount_permissionless_taking
    } else {
        check_permission_express_relay_and_get_fees(
            &ctx.accounts.sysvar_instructions,
            ctx.accounts.permission.as_ref().unwrap(),
            &ctx.accounts.pda_authority,
            &ctx.accounts.config_router,
            &ctx.accounts.express_relay_metadata.to_account_info(),
            &ctx.accounts.express_relay,
            ctx.accounts.order.key(),
        )?
    };

    Ok(tip)
}

fn transfer_output_to_maker_and_input_to_taker(
    ctx: &Context<TakeOrder>,
    global_config: &mut GlobalConfig,
    input_to_send_to_taker: u64,
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
        let maker_output_ata_account = ctx
            .accounts
            .maker_output_ata
            .as_ref()
            .ok_or(LimoError::MakerOutputAtaRequired)?;
        verify_ata(
            &ctx.accounts.maker.key(),
            &ctx.accounts.output_mint.key(),
            &maker_output_ata_account.key(),
            &ctx.accounts.output_token_program.key(),
        )?;
        maker_output_ata_account.to_account_info()
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

    Ok(())
}

fn tip_transfer_and_validation(
    ctx: &Context<TakeOrder>,
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
