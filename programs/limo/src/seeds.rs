pub const GLOBAL_AUTH: &[u8] = b"authority";
pub const TIP_VAULT: &[u8] = b"tip_vault";
pub const ESCROW_VAULT: &[u8] = b"escrow_vault";
pub const INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT: &[u8] = b"intermediary";
pub const EVENT_AUTHORITY: &[u8] = b"__event_authority";
pub const REFERRER_SEED: &[u8] = b"referrer";

mod macros {
    #[macro_export]
    macro_rules! global_seeds {
        ($bump: expr, $global_config_key: expr) => {
            &[GLOBAL_AUTH as &[u8], $global_config_key.as_ref(), &[$bump]]
        };
    }
    #[macro_export]
    macro_rules! intermediary_seeds {
        ($bump: expr, $order_key: expr) => {
            &[
                INTERMEDIARY_OUTPUT_TOKEN_ACCOUNT as &[u8],
                $order_key.as_ref(),
                &[$bump],
            ]
        };
    }
}
