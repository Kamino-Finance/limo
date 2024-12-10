use anchor_lang::{
    prelude::*,
    solana_program::{
        self,
        instruction::Instruction,
        sysvar::instructions::{load_current_index_checked, load_instruction_at_checked},
    },
    AnchorDeserialize, Discriminator,
};
use anchor_spl::{associated_token, token::spl_token, token_2022};
use solana_program::pubkey;

use crate::LimoError;

const COMPUTE_BUDGET_PUBKEY: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");

pub fn ensure_second_ix_match<T>(instruction_sysvar_account_info: &AccountInfo) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_second_ix_match_internal(&instruction_loader)
}

fn ensure_second_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let second_ix = search_second_ix(current_idx, instruction_loader)?;
    if let Some(discriminator) = second_ix.data.get(..8) {
        if discriminator != T::discriminator() {
            msg!("Extra ix is not the expected one");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    } else {
        msg!("Extra ix has no valid discriminator");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    let current_ix = instruction_loader.load_instruction_at(current_idx)?;
    if second_ix.accounts.len() != current_ix.accounts.len() {
        msg!("Number of accounts mismatch between first and second ix of couple");
        return err!(LimoError::FlashIxsAccountMismatch);
    }
    for (idx, (account_curr, account_other)) in current_ix
        .accounts
        .iter()
        .zip(second_ix.accounts.iter())
        .enumerate()
    {
        let account_curr_pk = &account_curr.pubkey;
        let account_other_pk = &account_other.pubkey;
        if account_curr_pk != account_other_pk {
            msg!("Some accounts in flash tx couple differs. index: {idx}, first:{account_curr_pk}, second:{account_other_pk}",);
            return err!(LimoError::FlashIxsAccountMismatch);
        }
    }

    Ok(T::try_from_slice(&second_ix.data[8..])?)
}

fn search_second_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<Instruction> {
    for idx in 0..current_idx {
        let ix = instruction_loader.load_instruction_at(idx)?;

        require!(
            program_id_allowed(ix.program_id),
            LimoError::FlashTxWithUnexpectedIxs
        );
    }

    let mut found_extra_ix = None;
    let mut ix_iterator =
        ix_utils::IxIterator::new_at(current_idx.checked_add(1).unwrap(), instruction_loader);

    for ix in ix_iterator.by_ref() {
        if ix.is_err() {
            msg!("Unexpected error encountered while iterating over instructions");
        }
        let ix = ix?;
        if ix.program_id == crate::id() {
            found_extra_ix = Some(ix);
            break;
        }
    }

    let extra_ix = found_extra_ix.ok_or_else(|| error!(LimoError::FlashIxsNotEnded))?;

    for ix in ix_iterator.by_ref() {
        if ix.is_err() {
            msg!("Unexpected error encountered while iterating over instructions");
        }
        let ix = ix?;
        require!(
            program_id_allowed(ix.program_id),
            LimoError::FlashTxWithUnexpectedIxs
        );
    }

    Ok(extra_ix)
}

fn program_id_allowed(program_id: Pubkey) -> bool {
    program_id == COMPUTE_BUDGET_PUBKEY
        || program_id == spl_token::ID
        || program_id == token_2022::ID
        || program_id == associated_token::ID
}

pub fn ensure_first_ix_match<T>(instruction_sysvar_account_info: &AccountInfo) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info,
    };
    ensure_first_ix_match_internal(&instruction_loader)
}

fn ensure_first_ix_match_internal<T>(
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<T>
where
    T: Discriminator + AnchorDeserialize,
{
    let current_idx = instruction_loader.load_current_index()?.into();
    let first_ix = search_first_ix(current_idx, instruction_loader)?;
    if let Some(discriminator) = first_ix.data.get(..8) {
        if discriminator != T::discriminator() {
            msg!("Extra ix is not the expected one");
            return err!(LimoError::FlashTxWithUnexpectedIxs);
        }
    } else {
        msg!("Extra ix has no valid discriminator");
        return err!(LimoError::FlashTxWithUnexpectedIxs);
    }

    let current_ix = instruction_loader.load_instruction_at(current_idx)?;
    if first_ix.accounts.len() != current_ix.accounts.len() {
        msg!("Number of accounts mismatch between first and second ix of couple");
        return err!(LimoError::FlashIxsAccountMismatch);
    }
    for (idx, (account_curr, account_other)) in current_ix
        .accounts
        .iter()
        .zip(first_ix.accounts.iter())
        .enumerate()
    {
        let account_curr_pk = &account_curr.pubkey;
        let account_other_pk = &account_other.pubkey;
        if account_curr_pk != account_other_pk {
            msg!("Some accounts in flash tx couple differs. index: {idx}, first:{account_curr_pk}, second:{account_other_pk}",);
            return err!(LimoError::FlashIxsAccountMismatch);
        }
    }

    Ok(T::try_from_slice(&first_ix.data[8..])?)
}

fn search_first_ix(
    current_idx: usize,
    instruction_loader: &impl ix_utils::InstructionLoader,
) -> Result<Instruction> {
    let mut ix_iterator =
        ix_utils::IxIterator::new_at(current_idx.checked_add(1).unwrap(), instruction_loader);

    for ix in ix_iterator.by_ref() {
        if ix.is_err() {
            msg!("Unexpected error encountered while iterating over instructions");
        }
        let ix = ix?;
        require!(
            program_id_allowed(ix.program_id),
            LimoError::FlashTxWithUnexpectedIxs
        );
    }

    let mut found_extra_ix = None;

    for idx in 0..current_idx {
        let ix = instruction_loader.load_instruction_at(idx)?;
        if ix.program_id == crate::id() {
            found_extra_ix = Some(ix);
            break;
        } else {
            require!(
                program_id_allowed(ix.program_id),
                LimoError::FlashTxWithUnexpectedIxs
            );
        }
    }

    let extra_ix = found_extra_ix.ok_or_else(|| error!(LimoError::FlashIxsNotStarted))?;

    Ok(extra_ix)
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
