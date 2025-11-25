use anchor_lang::{
    prelude::*, solana_program::instruction::Instruction, AnchorDeserialize, Discriminator,
};

use super::flash_ixs::{check_same_accounts, ix_utils};
use crate::LimoError;

pub fn ensure_end_ix_match<T>(instruction_sysvar_account_info: &AccountInfo) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_end_ix_match_internal(&instruction_loader)
}

fn ensure_end_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let end_ix = search_end_ix(current_idx, instruction_loader)?;

    if let Some(discriminator) = end_ix.data.get(..8) {
        if discriminator != T::discriminator() {
            msg!("End ix is not the expected one");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    } else {
        msg!("End ix has no valid discriminator");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    let current_ix = instruction_loader.load_instruction_at(current_idx)?;
    check_same_accounts(&current_ix, &end_ix)?;

    Ok(T::try_from_slice(&end_ix.data[8..])?)
}

fn search_end_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<Instruction> {
    let mut found_end_ix = None;
    let mut ix_iterator =
        ix_utils::IxIterator::new_at(current_idx.checked_add(1).unwrap(), instruction_loader);

    for ix in ix_iterator.by_ref() {
        if let Err(error) = ix {
            msg!("Unexpected error encountered while iterating over instructions");
            return Err(error.into());
        }
        let ix = ix?;

        if ix.program_id == crate::id() {
            found_end_ix = Some(ix);
            break;
        }
    }

    let end_ix = found_end_ix.ok_or_else(|| error!(LimoError::FlashIxsNotEnded))?;

    Ok(end_ix)
}

pub fn ensure_start_ix_match<T>(instruction_sysvar_account_info: &AccountInfo) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_start_ix_match_internal(&instruction_loader)
}

fn ensure_start_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let start_ix = search_start_ix(current_idx, instruction_loader)?;

    if let Some(discriminator) = start_ix.data.get(..8) {
        if discriminator != T::discriminator() {
            msg!("Start ix is not the expected one");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    } else {
        msg!("Start ix has no valid discriminator");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    let current_ix = instruction_loader.load_instruction_at(current_idx)?;
    check_same_accounts(&start_ix, &current_ix)?;

    Ok(T::try_from_slice(&start_ix.data[8..])?)
}

fn search_start_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<Instruction> {
    let mut found_start_ix = None;

    for idx in (0..current_idx).rev() {
        let ix = instruction_loader.load_instruction_at(idx)?;
        msg!("ix {} ix program: {:?}", idx, ix.program_id);
        if ix.program_id == crate::id() {
            found_start_ix = Some(ix);
            break;
        }
    }

    let start_ix = found_start_ix.ok_or_else(|| error!(LimoError::FlashIxsNotStarted))?;

    Ok(start_ix)
}
