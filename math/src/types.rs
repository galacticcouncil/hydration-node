use sp_arithmetic::FixedU128;

pub use crate::ratio::Ratio;

pub type Balance = u128;
pub type Price = FixedU128;
pub type Fraction = fixed::types::U1F127;
pub type LBPWeight = u32;

pub const HYDRA_ONE: u128 = 1_000_000_000_000u128;
pub const BASILISK_ONE: u128 = 1_000_000_000_000u128;
