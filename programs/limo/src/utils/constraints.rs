use anchor_lang::{err, prelude::*, require, Key, Result, ToAccountInfo};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token::{self, spl_token},
    token_2022::spl_token_2022,
    token_interface::TokenAccount,
};
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

pub mod token_2022 {
    use anchor_lang::{err, Key};
    use anchor_spl::{
        token::spl_token,
        token_2022::{
            spl_token_2022, spl_token_2022::extension::confidential_transfer::EncryptedBalance,
        },
        token_interface::spl_token_2022::extension::{
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
    };
    use bytemuck::Zeroable;
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

    use crate::{xmsg, LimoError};

    const VALID_LIQUIDITY_TOKEN_EXTENSIONS: &[ExtensionType] = &[
        ExtensionType::ConfidentialTransferFeeConfig,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::MintCloseAuthority,
        ExtensionType::MetadataPointer,
        ExtensionType::PermanentDelegate,
        ExtensionType::TransferFeeConfig,
        ExtensionType::TokenMetadata,
        ExtensionType::TransferHook,
    ];

    pub fn validate_token_extensions(
        mint_acc_info: &AccountInfo,
        token_acc_infos: Vec<&AccountInfo>,
    ) -> anchor_lang::Result<()> {
        if mint_acc_info.owner == &spl_token::id() {
            return Ok(());
        }

        let mint_data = mint_acc_info.data.borrow();
        let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

        let token_accounts_data = token_acc_infos
            .iter()
            .map(|acc| {
                if acc.owner == &spl_token::id() {
                    xmsg!(
                        "Invalid token account owner: {:?}, for account {:?}",
                        acc.owner,
                        acc.key()
                    );
                    return err!(LimoError::InvalidTokenAccount);
                }
                Ok(acc.data.borrow())
            })
            .collect::<Result<Vec<_>, _>>()?;

        for mint_ext in mint.get_extension_types()? {
            if !VALID_LIQUIDITY_TOKEN_EXTENSIONS.contains(&mint_ext) {
                xmsg!(
                    "Invalid liquidity token (2022) extension: {:?}, supported extensions: {:?}",
                    mint_ext,
                    VALID_LIQUIDITY_TOKEN_EXTENSIONS
                );
                return err!(LimoError::UnsupportedTokenExtension);
            }
            if mint_ext == ExtensionType::TransferFeeConfig {
                let ext = mint
                    .get_extension::<spl_token_2022::extension::transfer_fee::TransferFeeConfig>(
                    )?;
                if <u16>::from(ext.older_transfer_fee.transfer_fee_basis_points) != 0
                    || <u16>::from(ext.newer_transfer_fee.transfer_fee_basis_points) != 0
                {
                    xmsg!("Transfer fee must be 0 for tokens, got: {:?}", ext);
                    return err!(LimoError::UnsupportedTokenExtension);
                }
            } else if mint_ext == ExtensionType::TransferHook {
                let ext =
                    mint.get_extension::<spl_token_2022::extension::transfer_hook::TransferHook>()?;
                let hook_program_id: Option<Pubkey> = ext.program_id.into();
                if hook_program_id.is_some() {
                    xmsg!(
                        "Transfer hook program id must not be set for liquidity tokens, got {:?}",
                        ext
                    );
                    return err!(LimoError::UnsupportedTokenExtension);
                }
            } else if mint_ext == ExtensionType::ConfidentialTransferMint {
                let ext = mint
                .get_extension::<spl_token_2022::extension::confidential_transfer::ConfidentialTransferMint>(
                )?;
                if bool::from(ext.auto_approve_new_accounts) {
                    xmsg!(
                        "Auto approve new accounts must be false for liquidity tokens, got {:?}",
                        ext
                    );
                    return err!(LimoError::UnsupportedTokenExtension);
                }

                for token_acc_data in token_accounts_data.iter() {
                    let token_acc = StateWithExtensions::<spl_token_2022::state::Account>::unpack(
                        token_acc_data,
                    )?;
                    if let Ok(token_acc_ext) = token_acc.get_extension::<spl_token_2022::extension::confidential_transfer::ConfidentialTransferAccount>() {
                        if bool::from(token_acc_ext.allow_confidential_credits) {
                            xmsg!(
                                "Allow confidential credits must be false for token accounts, got {:?}",
                                token_acc_ext
                            );
                            return err!(LimoError::UnsupportedTokenExtension);
                        }
                        if token_acc_ext.pending_balance_lo != EncryptedBalance::zeroed()
                            && token_acc_ext.pending_balance_hi != EncryptedBalance::zeroed()
                        {
                            xmsg!(
                                "Pending balance must be zero for token accounts, got {:?}",
                                token_acc_ext
                            );
                            return err!(LimoError::UnsupportedTokenExtension);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn get_token_account_checked(
    account: &AccountInfo,
    expected_mint: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<TokenAccount> {
    if account.data_len() == 0 {
        return err!(LimoError::UninitializedTokenAccount);
    }

    if *account.owner != spl_token::id() && *account.owner != spl_token_2022::id() {
        return err!(LimoError::InvalidTokenAccountOwner);
    }

    let token_account = match TokenAccount::try_deserialize(&mut &account.data.borrow()[..]) {
        Ok(ta) => ta,
        Err(_) => return err!(LimoError::InvalidAccount),
    };

    if token_account.mint != *expected_mint {
        return err!(LimoError::InvalidTokenMint);
    }

    if token_account.owner != *expected_owner {
        return err!(LimoError::InvalidTokenAuthority);
    }

    Ok(token_account)
}
