use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::Instruction,
        sysvar::instructions::{load_current_index_checked, load_instruction_at_checked},
    },
    AnchorDeserialize, Discriminator,
};

use crate::LimoError;

pub fn ensure_end_ix_match<T>(
    instruction_sysvar_account_info: &AccountInfo,
    swap_program_id: &Pubkey,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_end_ix_match_internal(&instruction_loader, swap_program_id)
}

fn ensure_end_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
    swap_program_id: &Pubkey,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let end_ix = search_end_ix(current_idx, instruction_loader, swap_program_id)?;

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
    if end_ix.accounts.len() != current_ix.accounts.len() {
        msg!("Number of accounts mismatch between start and end ix");
        return err!(LimoError::FlashIxsAccountMismatch);
    }

    for (idx, (account_curr, account_other)) in current_ix
        .accounts
        .iter()
        .zip(end_ix.accounts.iter())
        .enumerate()
    {
        let account_curr_pk = &account_curr.pubkey;
        let account_other_pk = &account_other.pubkey;
        if account_curr_pk != account_other_pk {
            msg!("Some accounts in log_user_swap_balances tx differ. index: {idx}, start:{account_curr_pk}, end:{account_other_pk}",);
            return err!(LimoError::FlashIxsAccountMismatch);
        }
    }

    Ok(T::try_from_slice(&end_ix.data[8..])?)
}

fn search_end_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
    swap_program_id: &Pubkey,
) -> Result<Instruction> {
    let mut found_swap_ix = false;
    let mut found_end_ix = None;
    let mut ix_iterator =
        ix_utils::IxIterator::new_at(current_idx.checked_add(1).unwrap(), instruction_loader);

    for ix in ix_iterator.by_ref() {
        if ix.is_err() {
            msg!("Unexpected error encountered while iterating over instructions");
        }
        let ix = ix?;

        if ix.program_id == crate::id() {
            found_end_ix = Some(ix);
            break;
        } else if ix.program_id == *swap_program_id {
            if found_swap_ix {
                msg!("More than one swap instruction found between start and end");
                return err!(LimoError::FlashTxWithUnexpectedIxs);
            }
            found_swap_ix = true;
        } else {
            msg!("Unexpected instruction found between start and end");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    }

    let end_ix = found_end_ix.ok_or_else(|| error!(LimoError::FlashIxsNotEnded))?;

    if !found_swap_ix {
        msg!("No swap instruction found between start and end");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    Ok(end_ix)
}

pub fn ensure_start_ix_match<T>(
    instruction_sysvar_account_info: &AccountInfo,
    swap_program_id: &Pubkey,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_start_ix_match_internal(&instruction_loader, swap_program_id)
}

fn ensure_start_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
    swap_program_id: &Pubkey,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let start_ix = search_start_ix(current_idx, instruction_loader, swap_program_id)?;

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
    if start_ix.accounts.len() != current_ix.accounts.len() {
        msg!("Number of accounts mismatch between start and end ix");
        return err!(LimoError::FlashIxsAccountMismatch);
    }

    for (idx, (account_curr, account_other)) in current_ix
        .accounts
        .iter()
        .zip(start_ix.accounts.iter())
        .enumerate()
    {
        let account_curr_pk = &account_curr.pubkey;
        let account_other_pk = &account_other.pubkey;
        if account_curr_pk != account_other_pk {
            msg!("Some accounts in log_user_swap_balances tx differ. index: {idx}, end:{account_curr_pk}, start:{account_other_pk}",);
            return err!(LimoError::FlashIxsAccountMismatch);
        }
    }

    Ok(T::try_from_slice(&start_ix.data[8..])?)
}

fn search_start_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
    swap_program_id: &Pubkey,
) -> Result<Instruction> {
    let mut found_swap_ix = false;
    let mut found_start_ix = None;

    for idx in (0..current_idx).rev() {
        let ix = instruction_loader.load_instruction_at(idx)?;
        msg!("ix {} ix program: {:?}", idx, ix.program_id);
        if ix.program_id == crate::id() {
            found_start_ix = Some(ix);
            break;
        } else if ix.program_id == *swap_program_id {
            if found_swap_ix || found_start_ix.is_some() {
                msg!("Multiple swap instructions or swap instruction before start ix");
                return err!(LimoError::FlashTxWithUnexpectedIxs);
            }
            found_swap_ix = true;
        } else if found_start_ix.is_some() {
            msg!("Unexpected instruction between start and end");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    }

    let start_ix = found_start_ix.ok_or_else(|| error!(LimoError::FlashIxsNotStarted))?;

    if !found_swap_ix {
        msg!("No swap instruction found between start and end");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    Ok(start_ix)
}

mod ix_utils {
    use super::*;

    pub trait InstructionLoader {
        fn load_instruction_at(
            &self,
            index: usize,
        ) -> std::result::Result<Instruction, ProgramError>;
        fn load_current_index(&self) -> std::result::Result<u16, ProgramError>;
    }

    pub struct BpfInstructionLoader<'a, 'info> {
        pub instruction_sysvar_account_info: &'a AccountInfo<'info>,
    }

    impl<'a, 'info> InstructionLoader for BpfInstructionLoader<'a, 'info> {
        fn load_instruction_at(
            &self,
            index: usize,
        ) -> std::result::Result<Instruction, ProgramError> {
            load_instruction_at_checked(index, self.instruction_sysvar_account_info)
        }

        fn load_current_index(&self) -> std::result::Result<u16, ProgramError> {
            load_current_index_checked(self.instruction_sysvar_account_info)
        }
    }

    pub struct IxIterator<'a, IxLoader: InstructionLoader> {
        current_ix: usize,
        instruction_loader: &'a IxLoader,
    }

    impl<'a, IxLoader> IxIterator<'a, IxLoader>
    where
        IxLoader: InstructionLoader,
    {
        pub fn new_at(start_ix_index: usize, instruction_loader: &'a IxLoader) -> Self {
            Self {
                current_ix: start_ix_index,
                instruction_loader,
            }
        }
    }

    impl<IxLoader> Iterator for IxIterator<'_, IxLoader>
    where
        IxLoader: InstructionLoader,
    {
        type Item = std::result::Result<Instruction, ProgramError>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.instruction_loader.load_instruction_at(self.current_ix) {
                Ok(ix) => {
                    self.current_ix = self.current_ix.checked_add(1).unwrap();
                    Some(Ok(ix))
                }
                Err(ProgramError::InvalidArgument) => None,
                Err(e) => Some(Err(e)),
            }
        }
    }
}
