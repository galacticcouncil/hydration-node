
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

pub trait WeightInfo {
	fn pause() -> Weight;
	fn unpause() -> Weight;
	fn request_deposit() -> Weight;
	fn claim_deposit() -> Weight;
	fn withdraw_btc() -> Weight;
	fn complete_withdraw_btc() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn pause() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(Weight::from_parts(0, 1486))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn unpause() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(Weight::from_parts(0, 1486))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn request_deposit() -> Weight {
		Weight::from_parts(150_000_000, 65536)
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn claim_deposit() -> Weight {
		Weight::from_parts(60_000_000, 4096)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn withdraw_btc() -> Weight {
		Weight::from_parts(150_000_000, 65536)
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	fn complete_withdraw_btc() -> Weight {
		Weight::from_parts(60_000_000, 4096)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
}

impl WeightInfo for () {
	fn pause() -> Weight {
		Weight::from_parts(9_000_000, 0)
	}
	fn unpause() -> Weight {
		Weight::from_parts(9_000_000, 0)
	}
	fn request_deposit() -> Weight {
		Weight::from_parts(150_000_000, 0)
	}
	fn claim_deposit() -> Weight {
		Weight::from_parts(60_000_000, 0)
	}
	fn withdraw_btc() -> Weight {
		Weight::from_parts(150_000_000, 0)
	}
	fn complete_withdraw_btc() -> Weight {
		Weight::from_parts(60_000_000, 0)
	}
}
