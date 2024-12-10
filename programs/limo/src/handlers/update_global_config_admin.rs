use anchor_lang::{prelude::*, Accounts};

use crate::GlobalConfig;

pub fn handler_update_global_config_admin(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    msg!(
        "Updated Global Config admin_authority, previous: {}, new: {}",
        global_config.admin_authority,
        global_config.admin_authority_cached
    );

    global_config.admin_authority = global_config.admin_authority_cached;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGlobalConfigAdmin<'info> {
    admin_authority_cached: Signer<'info>,

    #[account(mut, has_one = admin_authority_cached)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
}
