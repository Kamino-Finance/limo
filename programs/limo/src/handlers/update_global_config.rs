use anchor_lang::{prelude::*, Accounts};

use crate::{
    operations,
    state::{GlobalConfig, UpdateGlobalConfigMode},
    utils::consts::UPDATE_GLOBAL_CONFIG_BYTE_SIZE,
};

pub fn handler_update_global_config(
    ctx: Context<UpdateGlobalConfig>,
    mode: u16,
    value: &[u8; UPDATE_GLOBAL_CONFIG_BYTE_SIZE],
) -> Result<()> {
    let ts = Clock::get()?.unix_timestamp;
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    let mode =
        UpdateGlobalConfigMode::try_from(mode).map_err(|_| ProgramError::InvalidInstructionData)?;

    operations::update_global_config(global_config, mode, value, ts.try_into().unwrap())?;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGlobalConfig<'info> {
    #[account(mut)]
    pub admin_authority: Signer<'info>,

    #[account(mut,
        has_one = admin_authority,)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
}
