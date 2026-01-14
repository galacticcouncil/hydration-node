use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use sp_core::U256;
use sp_runtime::traits::CheckedConversion;
use sp_runtime::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type AssetId = u32;
pub type Balance = u128;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type IntentId = u128;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum CallbackType {
	OnSuccess,
	OnFailure,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum IntentKind {
	Swap(SwapData),
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent {
	pub kind: IntentKind,
	pub deadline: Moment,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
}

impl Intent {
	pub fn is_partial(&self) -> bool {
		match &self.kind {
			IntentKind::Swap(s) => s.partial,
		}
	}

	pub fn asset_in(&self) -> AssetId {
		match &self.kind {
			IntentKind::Swap(s) => s.asset_in,
		}
	}

	pub fn asset_out(&self) -> AssetId {
		match &self.kind {
			IntentKind::Swap(s) => s.asset_out,
		}
	}

	pub fn amount_in(&self) -> Balance {
		match &self.kind {
			IntentKind::Swap(s) => s.amount_in,
		}
	}

	pub fn amount_out(&self) -> Balance {
		match &self.kind {
			IntentKind::Swap(s) => s.amount_out,
		}
	}

	/// Function calculates surplus amount from `resolved` intent.
	///
	/// Surplus must be >= zero
	pub fn surplus(&self, resolved: &Intent) -> Option<Balance> {
		match &self.kind {
			IntentKind::Swap(s) => match s.swap_type {
				SwapType::ExactIn => {
					let amt = if s.partial {
						self.pro_rata(&resolved)?
					} else {
						s.amount_out
					};

					resolved.amount_out().checked_sub(amt)
				}
				SwapType::ExactOut => {
					let amt = if s.partial {
						self.pro_rata(&resolved)?
					} else {
						s.amount_in
					};

					amt.checked_sub(resolved.amount_in())
				}
			},
		}
	}

	// Function calculates pro rata amount based on `resolved` intent.
	pub fn pro_rata(&self, resolved: &Intent) -> Option<Balance> {
		match &self.kind {
			IntentKind::Swap(s) => match s.swap_type {
				SwapType::ExactIn => U256::from(resolved.amount_in())
					.checked_mul(U256::from(s.amount_out))?
					.checked_div(U256::from(s.amount_in))?
					.checked_into(),

				SwapType::ExactOut => U256::from(resolved.amount_out())
					.checked_mul(U256::from(s.amount_in))?
					.checked_div(U256::from(s.amount_out))?
					.checked_into(),
			},
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct SwapData {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub swap_type: SwapType,
	pub partial: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SwapType {
	ExactIn,
	ExactOut,
}
