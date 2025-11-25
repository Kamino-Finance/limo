use anchor_lang::prelude::*;

use crate::{operations, state::Order, GlobalConfig, UpdateOrderMode};

pub fn handler_update_order(ctx: Context<UpdateOrder>, mode: u16, value: &[u8]) -> Result<()> {
    let order = &mut ctx.accounts.order.load_mut()?;

    let mode = UpdateOrderMode::try_from(mode).map_err(|_| ProgramError::InvalidInstructionData)?;

    operations::update_order(order, mode, value)?;

    msg!("Updating order with mode {:?} and value {:?}", mode, &value);

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateOrder<'info> {
    pub maker: Signer<'info>,

    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(mut,
        has_one = maker,
        has_one = global_config)]
    pub order: AccountLoader<'info, Order>,
}
