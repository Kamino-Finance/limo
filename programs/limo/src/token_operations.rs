use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
};
use anchor_spl::{
    token::{spl_token, TokenAccount},
    token_interface,
};
use solana_program::{
    program::{invoke, invoke_signed},
    program_pack::Pack,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

#[allow(clippy::too_many_arguments)]
pub fn transfer_from_user_to_token_account<'a>(
    user_token_account: AccountInfo<'a>,
    destination_token_account: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    token_mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    deposit_amount: u64,
    token_decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new(
            token_program.clone(),
            token_interface::TransferChecked {
                from: user_token_account,
                to: destination_token_account,
                authority: user_authority,
                mint: token_mint,
            },
        ),
        deposit_amount,
        token_decimals,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn transfer_from_vault_to_token_account<'a>(
    user_token_account: AccountInfo<'a>,
    vault_token_account: AccountInfo<'a>,
    pda_authority: AccountInfo<'a>,
    token_mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    deposit_amount: u64,
    token_decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            token_program.clone(),
            token_interface::TransferChecked {
                from: vault_token_account,
                to: user_token_account,
                authority: pda_authority,
                mint: token_mint,
            },
            &[authority_signer_seeds],
        ),
        deposit_amount,
        token_decimals,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn lamports_transfer_from_authority_to_account<'a>(
    user_account: AccountInfo<'a>,
    authority_account: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    amount_lamports: u64,
) -> Result<()> {
    let ix = system_instruction::transfer(authority_account.key, user_account.key, amount_lamports);

    invoke_signed(
        &ix,
        &[
            authority_account.clone(),
            user_account.clone(),
            system_program.clone(),
        ],
        &[authority_signer_seeds],
    )?;

    Ok(())
}

pub fn native_transfer_from_user_to_account<'a>(
    from_account: AccountInfo<'a>,
    to_account: AccountInfo<'a>,
    amount: u64,
) -> Result<()> {
    let transfer_ix = system_instruction::transfer(from_account.key, to_account.key, amount);

    invoke(&transfer_ix, &[from_account.clone(), to_account.clone()])?;

    Ok(())
}

pub fn native_transfer_from_authority_to_user<'a>(
    authority: AccountInfo<'a>,
    to_account: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    amount: u64,
) -> Result<()> {
    let transfer_ix = system_instruction::transfer(authority.key, to_account.key, amount);

    invoke_signed(
        &transfer_ix,
        &[authority.clone(), to_account.clone()],
        &[authority_signer_seeds],
    )?;

    Ok(())
}

pub fn close_ata_accounts_with_signer_seeds<'a>(
    account_to_close: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
) -> Result<()> {
    token_interface::close_account(CpiContext::new_with_signer(
        token_program.clone(),
        token_interface::CloseAccount {
            account: account_to_close,
            destination,
            authority,
        },
        &[authority_signer_seeds],
    ))?;

    Ok(())
}

pub fn initialize_intermediary_token_account_with_signer_seeds<'a>(
    intermediary_token_account: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    rent_sysvar: AccountInfo<'a>,
    token_account_signer_seeds: &[&[u8]],
    authority_signer_seeds: &[&[u8]],
) -> Result<()> {
    let token_account_len = if *token_program.key == token_interface::ID {
        token_interface::spl_token_2022::state::Account::LEN
    } else {
        TokenAccount::LEN
    };

    let rent_exempt_balance = Rent::get()?.minimum_balance(token_account_len);
    let current_lamports_balance = intermediary_token_account.lamports();

    if current_lamports_balance == 0 {
        let create_ix = system_instruction::create_account(
            authority.key,
            intermediary_token_account.key,
            rent_exempt_balance,
            token_account_len as u64,
            token_program.key,
        );

        invoke_signed(
            &create_ix,
            &[authority.clone(), intermediary_token_account.clone()],
            &[authority_signer_seeds, token_account_signer_seeds],
        )?;
    } else {
        let lamports_needed = rent_exempt_balance.saturating_sub(current_lamports_balance);

        if lamports_needed > 0 {
            let transfer_ix = system_instruction::transfer(
                authority.key,
                intermediary_token_account.key,
                lamports_needed,
            );

            invoke_signed(
                &transfer_ix,
                &[authority.clone(), intermediary_token_account.clone()],
                &[authority_signer_seeds],
            )?;
        }

        let allocate_ix =
            system_instruction::allocate(intermediary_token_account.key, token_account_len as u64);

        let assign_ix = system_instruction::assign(intermediary_token_account.key, &spl_token::ID);

        invoke_signed(
            &allocate_ix,
            &[authority.clone(), intermediary_token_account.clone()],
            &[authority_signer_seeds, token_account_signer_seeds],
        )?;

        invoke_signed(
            &assign_ix,
            &[authority.clone(), intermediary_token_account.clone()],
            &[authority_signer_seeds, token_account_signer_seeds],
        )?;
    }

    token_interface::initialize_account(CpiContext::new_with_signer(
        token_program.clone(),
        token_interface::InitializeAccount {
            account: intermediary_token_account,
            mint,
            authority,
            rent: rent_sysvar,
        },
        &[authority_signer_seeds],
    ))?;

    Ok(())
}
