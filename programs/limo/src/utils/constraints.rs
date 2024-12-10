use anchor_lang::{err, prelude::*, require, Key, Result, ToAccountInfo};
use anchor_spl::{associated_token::get_associated_token_address_with_program_id, token};
use express_relay::{cpi::accounts::CheckPermission, sdk::cpi::check_permission_cpi};

use crate::{GlobalConfig, LimoError};

pub fn emergency_mode_disabled(global_config: &AccountLoader<GlobalConfig>) -> Result<()> {
    if global_config.load()?.emergency_mode > 0 {
        return err!(LimoError::EmergencyModeEnabled);
    }
    Ok(())
}

pub fn flash_taking_orders_disabled(global_config: &AccountLoader<GlobalConfig>) -> Result<()> {
    if global_config.load()?.flash_take_order_blocked > 0 {
        return err!(LimoError::FlashTakeOrderBlocked);
    }
    Ok(())
}

pub fn create_new_orders_disabled(global_config: &AccountLoader<GlobalConfig>) -> Result<()> {
    if global_config.load()?.new_orders_blocked > 0 {
        return err!(LimoError::CreatingNewOrdersBlocked);
    }
    Ok(())
}

pub fn taking_orders_disabled(global_config: &AccountLoader<GlobalConfig>) -> Result<()> {
    if global_config.load()?.orders_taking_blocked > 0 {
        return err!(LimoError::OrderTakingBlocked);
    }
    Ok(())
}

pub fn check_permission_express_relay_and_get_fees<'a>(
    sysvar_instructions: &AccountInfo<'a>,
    permission: &AccountInfo<'a>,
    pda_authority: &AccountInfo<'a>,
    config_router: &AccountInfo<'a>,
    express_relay_metadata: &AccountInfo<'a>,
    express_relay_program: &AccountInfo<'a>,
    order_key: Pubkey,
) -> Result<u64> {
    let express_relay_check_permission_accounts = CheckPermission {
        sysvar_instructions: sysvar_instructions.to_account_info(),
        permission: permission.to_account_info(),
        router: pda_authority.to_account_info(),
        config_router: config_router.to_account_info(),
        express_relay_metadata: express_relay_metadata.to_account_info(),
    };

    require!(
        permission.key() == order_key,
        LimoError::PermissionDoesNotMatchOrder
    );

    let fees = check_permission_cpi(
        express_relay_check_permission_accounts,
        express_relay_program.to_account_info(),
    )?;

    Ok(fees)
}

pub fn verify_ata(
    wallet: &Pubkey,
    mint: &Pubkey,
    ata_account_key: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<()> {
    let expected_ata = get_associated_token_address_with_program_id(wallet, mint, token_program_id);

    require_keys_eq!(
        ata_account_key.key(),
        expected_ata,
        LimoError::InvalidAtaAddress
    );

    Ok(())
}

pub fn is_permissionless_order_taking_allowed(global_config: &GlobalConfig) -> bool {
    global_config.is_order_taking_permissionless == 1
}

pub fn is_wsol(mint: &Pubkey) -> bool {
    *mint == token::spl_token::native_mint::ID
}
