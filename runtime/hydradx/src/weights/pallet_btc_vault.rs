#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_btc_vault::WeightInfo for HydraWeight<T> {
	fn pause() -> Weight {
		Weight::from_parts(7_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn unpause() -> Weight {
		Weight::from_parts(7_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn request_deposit() -> Weight {
		Weight::from_parts(150_000_000, 65536)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	fn claim_deposit() -> Weight {
		Weight::from_parts(60_000_000, 4096)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	fn withdraw_btc() -> Weight {
		Weight::from_parts(150_000_000, 65536)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	fn complete_withdraw_btc() -> Weight {
		Weight::from_parts(60_000_000, 4096)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}
