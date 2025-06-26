use anchor_lang::prelude::{Pubkey, *};
use derivative::Derivative;
use num_enum::TryFromPrimitive;

use crate::{utils::consts::UPDATE_GLOBAL_CONFIG_BYTE_SIZE, LimoError};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderStatus {
    Active = 0,
    Filled = 1,
    Cancelled = 2,
}

impl From<OrderStatus> for u8 {
    fn from(val: OrderStatus) -> Self {
        match val {
            OrderStatus::Active => 0,
            OrderStatus::Filled => 1,
            OrderStatus::Cancelled => 2,
        }
    }
}

impl From<u8> for OrderStatus {
    fn from(val: u8) -> Self {
        match val {
            0 => OrderStatus::Active,
            1 => OrderStatus::Filled,
            2 => OrderStatus::Cancelled,
            _ => panic!("Invalid OrderStatus"),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OrderType {
    Vanilla = 0,
}

impl From<OrderType> for u8 {
    fn from(val: OrderType) -> Self {
        match val {
            OrderType::Vanilla => 0,
        }
    }
}

impl TryFrom<u8> for OrderType {
    type Error = LimoError;
    fn try_from(val: u8) -> core::result::Result<Self, LimoError> {
        match val {
            0 => Ok(OrderType::Vanilla),
            _ => Err(LimoError::OrderTypeInvalid),
        }
    }
}

#[derive(PartialEq, Derivative, Default)]
#[derivative(Debug)]
#[account(zero_copy)]
pub struct Order {
    pub global_config: Pubkey,
    pub maker: Pubkey,

    pub input_mint: Pubkey,
    pub input_mint_program_id: Pubkey,
    pub output_mint: Pubkey,
    pub output_mint_program_id: Pubkey,

    pub initial_input_amount: u64,
    pub expected_output_amount: u64,
    pub remaining_input_amount: u64,
    pub filled_output_amount: u64,
    pub tip_amount: u64,
    pub number_of_fills: u64,

    pub order_type: u8,
    pub status: u8,
    pub in_vault_bump: u8,
    pub flash_ix_lock: u8,

    pub permissionless: u8,

    pub padding0: [u8; 3],

    pub last_updated_timestamp: u64,

    pub flash_start_taker_output_balance: u64,

    pub counterparty: Pubkey,

    pub padding: [u64; 15],
}

#[event]
pub struct OrderDisplay {
    pub initial_input_amount: u64,
    pub expected_output_amount: u64,
    pub remaining_input_amount: u64,
    pub filled_output_amount: u64,
    pub tip_amount: u64,
    pub number_of_fills: u64,

    pub on_event_output_amount_filled: u64,
    pub on_event_tip_amount: u64,

    pub order_type: u8,
    pub status: u8,

    pub last_updated_timestamp: u64,
}

#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
pub struct UserSwapBalancesState {
    pub user_lamports: u64,
    pub input_ta_balance: u64,
    pub output_ta_balance: u64,
}

#[event]
pub struct UserSwapBalanceDiffs {
    pub user_lamports_before: u64,
    pub input_ta_balance_before: u64,
    pub output_ta_balance_before: u64,
    pub user_lamports_after: u64,
    pub input_ta_balance_after: u64,
    pub output_ta_balance_after: u64,
    pub swap_program: Pubkey,
    pub simulated_swap_amount_out: u64,
    pub simulated_ts: u64,
    pub minimum_amount_out: u64,
    pub swap_amount_in: u64,
    pub simulated_amount_out_next_best: u64,
    pub aggregator: u8,
    pub next_best_aggregator: u8,
}

#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
pub struct GlobalConfig {
    pub emergency_mode: u8,
    pub flash_take_order_blocked: u8,
    pub new_orders_blocked: u8,
    pub orders_taking_blocked: u8,

    pub host_fee_bps: u16,

    pub padding0: [u8; 2],
    pub order_close_delay_seconds: u64,
    pub padding1: [u64; 9],

    pub pda_authority_previous_lamports_balance: u64,
    pub total_tip_amount: u64,
    pub host_tip_amount: u64,

    pub pda_authority: Pubkey,
    pub pda_authority_bump: u64,
    pub admin_authority: Pubkey,
    pub admin_authority_cached: Pubkey,
    pub txn_fee_cost: u64,
    pub ata_creation_cost: u64,

    pub padding2: [u64; 241],
}

impl Default for GlobalConfig {
    #[cfg(not(any(feature = "test-bpf", test)))]
    fn default() -> Self {
        unimplemented!()
    }

    #[cfg(any(test, feature = "test-bpf"))]
    #[inline(never)]
    fn default() -> GlobalConfig {
        GlobalConfig {
            flash_take_order_blocked: 0,
            new_orders_blocked: 0,
            orders_taking_blocked: 0,
            host_fee_bps: 0,
            order_close_delay_seconds: 0,
            pda_authority_previous_lamports_balance: 0,
            total_tip_amount: 0,
            host_tip_amount: 0,
            pda_authority: Pubkey::default(),
            pda_authority_bump: 0,
            admin_authority: Pubkey::default(),
            admin_authority_cached: Pubkey::default(),
            emergency_mode: 0,
            ata_creation_cost: 0,
            txn_fee_cost: 0,
            padding0: [0; 2],
            padding1: [0; 9],
            padding2: [0; 241],
        }
    }
}

pub struct TakeOrderEffects {
    pub input_to_send_to_taker: u64,
    pub output_to_send_to_maker: u64,
}

pub struct TipCalcs {
    pub host_tip: u64,
    pub maker_tip: u64,
}

#[derive(TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(u16)]
pub enum UpdateGlobalConfigMode {
    UpdateEmergencyMode = 0,
    UpdateFlashTakeOrderBlocked = 1,
    UpdateBlockNewOrders = 2,
    UpdateBlockOrderTaking = 3,
    UpdateHostFeeBps = 4,
    UpdateAdminAuthorityCached = 5,
    UpdateOrderTakingPermissionless = 6,
    UpdateOrderCloseDelaySeconds = 7,
    UpdateTxnFeeCost = 8,
    UpdateAtaCreationCost = 9,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum UpdateGlobalConfigValue {
    Bool(bool),
    U16(u16),
    U64(u64),
    Pubkey(Pubkey),
}

impl UpdateGlobalConfigValue {
    pub fn to_raw_bytes_array(&self) -> [u8; UPDATE_GLOBAL_CONFIG_BYTE_SIZE] {
        let mut raw_bytes_array = [0u8; UPDATE_GLOBAL_CONFIG_BYTE_SIZE];
        match self {
            UpdateGlobalConfigValue::Bool(v) => {
                let raw_bytes = vec![*v as u8];
                raw_bytes_array[..1].copy_from_slice(&raw_bytes);
            }
            UpdateGlobalConfigValue::U16(v) => {
                raw_bytes_array[..2].copy_from_slice(&v.to_le_bytes());
            }
            UpdateGlobalConfigValue::U64(v) => {
                raw_bytes_array[..8].copy_from_slice(&v.to_le_bytes());
            }
            UpdateGlobalConfigValue::Pubkey(v) => {
                raw_bytes_array[..32].copy_from_slice(v.as_ref());
            }
        }
        raw_bytes_array
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetBalancesCheckedResult {
    pub lamports_balance: u64,
    pub input_balance: u64,
    pub output_balance: u64,
}

#[derive(
    TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug, AnchorSerialize, AnchorDeserialize,
)]
#[repr(u16)]
pub enum UpdateOrderMode {
    UpdatePermissionless = 0,
    UpdateCounterparty = 1,
}
