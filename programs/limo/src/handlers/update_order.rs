use crate::state::Order;
use crate::UpdateOrderMode;
use crate::{operations, GlobalConfig};
use anchor_lang::prelude::*;

pub fn handler_update_order(
    ctx: Context<UpdateOrder>,
    mode: UpdateOrderMode,
    value: &[u8],
) -> Result<()> {
    let order = &mut ctx.accounts.order.load_mut()?;

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
