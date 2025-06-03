#![allow(clippy::too_many_arguments)]
use std::cmp;

use anchor_lang::prelude::*;
use solana_program::clock;

use crate::{
    dbg_msg,
    state::*,
    utils::{
        consts::UPDATE_GLOBAL_CONFIG_BYTE_SIZE,
        fraction::{Fraction, FractionExtra},
    },
    LimoError,
};

pub fn initialize_global_config(
    global_config: &mut GlobalConfig,
    admin_authority: Pubkey,
    pda_authority: Pubkey,
    pda_bump: u64,
    pda_authority_previous_lamports_balance: u64,
) {
    global_config.emergency_mode = 0;
    global_config.pda_authority = pda_authority;
    global_config.pda_authority_bump = pda_bump;
    global_config.admin_authority = admin_authority;
    global_config.admin_authority_cached = admin_authority;
    global_config.total_tip_amount = 0;
    global_config.host_tip_amount = 0;
    global_config.pda_authority_previous_lamports_balance = pda_authority_previous_lamports_balance;
}

pub fn create_order(
    order: &mut Order,
    global_config: Pubkey,
    owner: Pubkey,
    input_amount: u64,
    output_amount: u64,
    input_mint: Pubkey,
    output_mint: Pubkey,
    input_mint_program_id: Pubkey,
    output_mint_program_id: Pubkey,
    order_type: u8,
    in_vault_bump: u8,
    current_timestamp: i64,
) -> Result<()> {
    order.global_config = global_config;
    order.initial_input_amount = input_amount;
    order.remaining_input_amount = input_amount;
    order.expected_output_amount = output_amount;
    order.number_of_fills = 0;
    order.filled_output_amount = 0;
    order.input_mint = input_mint;
    order.input_mint_program_id = input_mint_program_id;
    order.output_mint = output_mint;
    order.output_mint_program_id = output_mint_program_id;
    order.maker = owner;
    order.status = OrderStatus::Active as u8;
    order.order_type = order_type;
    order.in_vault_bump = in_vault_bump;
    order.last_updated_timestamp = current_timestamp.try_into().expect("Negative timestamp");
    order.counterparty = Pubkey::default();
    order.permissionless = 0;

    Ok(())
}

pub fn update_order(order: &mut Order, mode: UpdateOrderMode, value: &[u8]) -> Result<()> {
    match mode {
        UpdateOrderMode::UpdatePermissionless => {
            require!(value.len() == 1, LimoError::InvalidParameterType);
            msg!("update_order mode={:?}", mode);
            msg!("new={} prev={}", value[0], order.permissionless);
            order.permissionless = value[0];
        }
        UpdateOrderMode::UpdateCounterparty => {
            require!(value.len() == 32, LimoError::InvalidParameterType);
            msg!("update_order mode={:?}", mode);
            msg!("new={:?} prev={}", &value[..32], order.counterparty);
            order.counterparty = Pubkey::new_from_array(
                value[..32]
                    .try_into()
                    .map_err(|_| LimoError::InvalidParameterType)?,
            );
        }
    }
    Ok(())
}

pub fn close_order_and_claim_tip(
    order: &mut Order,
    global_config: &mut GlobalConfig,
    current_timestamp: u64,
) -> Result<()> {
    require!(
        order.status == OrderStatus::Active as u8 || order.status == OrderStatus::Filled as u8,
        LimoError::OrderCanNotBeCanceled
    );

    require!(
        current_timestamp >= order.last_updated_timestamp + global_config.order_close_delay_seconds,
        LimoError::NotEnoughTimePassedSinceLastUpdate
    );

    require!(
        order.flash_ix_lock == 0,
        LimoError::OrderWithinFlashOperation
    );

    order.status = OrderStatus::Cancelled as u8;

    global_config.total_tip_amount -= order.tip_amount;

    Ok(())
}

pub fn withdraw_host_tip(
    global_config: &mut GlobalConfig,
    pda_authority_balance: u64,
) -> Result<u64> {
    require_gte!(
        pda_authority_balance,
        global_config.host_tip_amount,
        LimoError::InvalidHostTipBalance
    );
    let host_tip_amount = global_config.host_tip_amount;
    global_config.total_tip_amount -= host_tip_amount;
    global_config.host_tip_amount = 0;
    Ok(host_tip_amount)
}

pub fn flash_withdraw_order_input(
    order: &mut Order,
    input_amount: u64,
    output_amount: u64,
) -> Result<TakeOrderEffects> {
    let TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    } = take_order_calcs(order, input_amount, output_amount)?;

    require!(
        order.flash_ix_lock == 0,
        LimoError::OrderWithinFlashOperation
    );

    order.flash_ix_lock = 1;
    Ok(TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    })
}

pub fn flash_pay_order_output(
    global_config: &mut GlobalConfig,
    order: &mut Order,
    input_amount: u64,
    output_amount: u64,
    tip_amount: u64,
    current_timestamp: clock::UnixTimestamp,
) -> Result<TakeOrderEffects> {
    let TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    } = take_order_calcs(order, input_amount, output_amount)?;

    require!(
        order.flash_ix_lock == 1,
        LimoError::OrderNotWithinFlashOperation
    );

    update_take_order_accounting_and_tips(
        global_config,
        order,
        input_to_send_to_taker,
        output_to_send_to_maker,
        tip_amount,
        current_timestamp,
    )?;

    order.flash_ix_lock = 0;
    Ok(TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    })
}

pub fn take_order_calcs(
    order: &Order,
    input_amount: u64,
    output_amount: u64,
) -> Result<TakeOrderEffects> {
    require!(input_amount > 0, LimoError::OrderInputAmountInvalid);

    require!(
        order.status == OrderStatus::Active as u8,
        LimoError::OrderNotActive
    );

    require!(
        input_amount <= order.remaining_input_amount,
        LimoError::OrderInputAmountTooLarge
    );

    let input_to_send_to_taker = input_amount;
    let minimum_output_to_send_to_maker_u128 = (u128::from(input_to_send_to_taker)
        * u128::from(order.expected_output_amount))
    .div_ceil(u128::from(order.initial_input_amount));

    let minimum_output_to_send_to_maker = u64::try_from(minimum_output_to_send_to_maker_u128)
        .map_err(|_| dbg_msg!(LimoError::MathOverflow))?;

    let output_to_send_to_maker = cmp::max(output_amount, minimum_output_to_send_to_maker);

    if output_to_send_to_maker != output_amount {
        msg!("output_amount: {}", output_amount);
        msg!(
            "minimum_output_to_send_to_maker: {}",
            minimum_output_to_send_to_maker
        );
        return err!(LimoError::OrderOutputAmountInvalid);
    }

    msg!("input_to_send_to_taker: {}", input_to_send_to_taker);
    msg!("output_to_send_to_maker: {}", output_to_send_to_maker);

    Ok(TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    })
}

pub fn take_order(
    global_config: &mut GlobalConfig,
    order: &mut Order,
    input_amount: u64,
    tip_amount: u64,
    current_timestamp: clock::UnixTimestamp,
    output_amount: u64,
) -> Result<TakeOrderEffects> {
    require!(
        order.flash_ix_lock == 0,
        LimoError::OrderWithinFlashOperation
    );

    let TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    } = take_order_calcs(order, input_amount, output_amount)?;

    update_take_order_accounting_and_tips(
        global_config,
        order,
        input_to_send_to_taker,
        output_to_send_to_maker,
        tip_amount,
        current_timestamp,
    )?;

    Ok(TakeOrderEffects {
        input_to_send_to_taker,
        output_to_send_to_maker,
    })
}

pub fn update_global_config(
    global_config: &mut GlobalConfig,
    mode: UpdateGlobalConfigMode,
    value: &[u8; UPDATE_GLOBAL_CONFIG_BYTE_SIZE],
    ts: u64,
) -> Result<()> {
    match mode {
        UpdateGlobalConfigMode::UpdateEmergencyMode
        | UpdateGlobalConfigMode::UpdateFlashTakeOrderBlocked
        | UpdateGlobalConfigMode::UpdateBlockNewOrders
        | UpdateGlobalConfigMode::UpdateBlockOrderTaking
        | UpdateGlobalConfigMode::UpdateOrderTakingPermissionless => {
            let value = value[0];
            update_global_config_flag(global_config, mode, value, ts)?;
        }
        UpdateGlobalConfigMode::UpdateHostFeeBps => {
            let value = u16::from_le_bytes(value[0..2].try_into().unwrap());
            require!(value <= 10000, LimoError::InvalidHostFee);
            msg!("update_global_config mode={:?} ts={}", mode, ts);
            msg!("new={} prev={}", value, global_config.host_fee_bps);
            global_config.host_fee_bps = value;
        }
        UpdateGlobalConfigMode::UpdateOrderCloseDelaySeconds => {
            let value = u64::from_le_bytes(value[0..8].try_into().unwrap());
            msg!("update_global_config mode={:?} ts={}", mode, ts);
            msg!(
                "new={} prev={}",
                value,
                global_config.order_close_delay_seconds
            );
            global_config.order_close_delay_seconds = value;
        }
        UpdateGlobalConfigMode::UpdateAdminAuthorityCached => {
            let value = Pubkey::new_from_array(value[0..32].try_into().unwrap());
            update_global_config_pubkey(global_config, mode, value, ts)?
        }
        UpdateGlobalConfigMode::UpdateTxnFeeCost => {
            let value = u64::from_le_bytes(value[0..8].try_into().unwrap());
            msg!("update_global_config mode={:?} ts={}", mode, ts);
            msg!("new={} prev={}", value, global_config.txn_fee_cost);
            global_config.txn_fee_cost = value;
        }
        UpdateGlobalConfigMode::UpdateAtaCreationCost => {
            let value = u64::from_le_bytes(value[0..8].try_into().unwrap());
            msg!("update_global_config mode={:?} ts={}", mode, ts);
            msg!("new={} prev={}", value, global_config.ata_creation_cost);
            global_config.ata_creation_cost = value;
        }
    }
    Ok(())
}

pub fn validate_pda_authority_balance_and_update_accounting(
    global_config: &mut GlobalConfig,
    pda_authority_balance: u64,
    tip: u64,
) -> Result<()> {
    require_gte!(
        pda_authority_balance - global_config.pda_authority_previous_lamports_balance,
        tip,
        LimoError::InvalidTipTransferAmount
    );
    require_gte!(
        pda_authority_balance,
        global_config.total_tip_amount,
        LimoError::InvalidTipBalance
    );

    global_config.pda_authority_previous_lamports_balance = pda_authority_balance;

    Ok(())
}

fn update_take_order_accounting_and_tips(
    global_config: &mut GlobalConfig,
    order: &mut Order,
    input_to_send_to_taker: u64,
    output_to_send_to_maker: u64,
    tip_amount: u64,
    current_timestamp: i64,
) -> Result<()> {
    order.remaining_input_amount = order
        .remaining_input_amount
        .checked_sub(input_to_send_to_taker)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    order.filled_output_amount = order
        .filled_output_amount
        .checked_add(output_to_send_to_maker)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    let TipCalcs {
        host_tip,
        maker_tip,
    } = tip_calcs(global_config, tip_amount)?;

    global_config.host_tip_amount = global_config
        .host_tip_amount
        .checked_add(host_tip)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    order.tip_amount = order
        .tip_amount
        .checked_add(maker_tip)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    global_config.total_tip_amount = global_config
        .total_tip_amount
        .checked_add(tip_amount)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    order.number_of_fills += 1;

    if order.remaining_input_amount == 0
        && order.filled_output_amount >= order.expected_output_amount
    {
        order.status = OrderStatus::Filled as u8;
    }
    order.last_updated_timestamp = current_timestamp.try_into().expect("Negative timestamp");
    Ok(())
}

fn tip_calcs(global_config: &GlobalConfig, tip_amount: u64) -> Result<TipCalcs> {
    let host_tip = (Fraction::from_bps(global_config.host_fee_bps) * Fraction::from(tip_amount))
        .to_ceil::<u64>();

    let maker_tip = tip_amount
        .checked_sub(host_tip)
        .ok_or_else(|| dbg_msg!(LimoError::MathOverflow))?;

    Ok(TipCalcs {
        host_tip,
        maker_tip,
    })
}

fn update_global_config_flag(
    global_config: &mut GlobalConfig,
    mode: UpdateGlobalConfigMode,
    value: u8,
    ts: u64,
) -> Result<()> {
    msg!("update_global_config_flag mode={:?} ts={}", mode, ts);

    if value != 0 && value != 1 {
        return err!(LimoError::InvalidFlag);
    }

    match mode {
        UpdateGlobalConfigMode::UpdateEmergencyMode => {
            msg!("new={} prev={}", value, global_config.emergency_mode,);
            global_config.emergency_mode = value;
        }
        UpdateGlobalConfigMode::UpdateFlashTakeOrderBlocked => {
            msg!(
                "new={} prev={}",
                value,
                global_config.flash_take_order_blocked,
            );
            global_config.flash_take_order_blocked = value;
        }
        UpdateGlobalConfigMode::UpdateBlockNewOrders => {
            msg!("new={} prev={}", value, global_config.new_orders_blocked,);
            global_config.new_orders_blocked = value;
        }
        UpdateGlobalConfigMode::UpdateBlockOrderTaking => {
            msg!("new={} prev={}", value, global_config.orders_taking_blocked,);
            global_config.orders_taking_blocked = value;
        }
        UpdateGlobalConfigMode::UpdateOrderTakingPermissionless => {
            msg!("Field deprecated");
        }
        _ => return Err(LimoError::InvalidConfigOption.into()),
    }

    Ok(())
}

fn update_global_config_pubkey(
    global_config: &mut GlobalConfig,
    mode: UpdateGlobalConfigMode,
    value: Pubkey,
    ts: u64,
) -> Result<()> {
    msg!("update_global_config_pubkey mode={:?} ts={}", mode, ts);

    match mode {
        UpdateGlobalConfigMode::UpdateAdminAuthorityCached => {
            msg!(
                "new={} prev={}",
                value,
                global_config.admin_authority_cached,
            );
            global_config.admin_authority_cached = value;
        }
        _ => return Err(LimoError::InvalidConfigOption.into()),
    }

    Ok(())
}
