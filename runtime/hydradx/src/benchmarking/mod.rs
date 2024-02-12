#![cfg(feature = "runtime-benchmarks")]

pub mod currencies;
pub mod dca;
pub mod duster;
pub mod multi_payment;
pub mod omnipool;
pub mod route_executor;
pub mod tokens;
pub mod vesting;
pub mod xyk;

use crate::AssetRegistry;
use frame_system::RawOrigin;

use hydradx_traits::{registry::Create, AssetKind};
use primitives::{AssetId, Balance};
use sp_runtime::traits::One;
use sp_std::vec;
use sp_std::vec::Vec;

pub const BSX: Balance = primitives::constants::currency::UNITS;

use frame_support::storage::with_transaction;
use sp_runtime::TransactionOutcome;

pub fn register_asset(name: Vec<u8>, deposit: Balance) -> Result<AssetId, ()> {
	let n = name.try_into().map_err(|_| ())?;
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(n),
			AssetKind::Token,
			deposit,
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}

pub fn register_external_asset(name: Vec<u8>) -> Result<AssetId, ()> {
	let n = name.try_into().map_err(|_| ())?;
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_insufficient_asset(
			None,
			Some(n),
			AssetKind::External,
			Some(Balance::one()),
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}

#[allow(dead_code)]
pub fn update_asset(asset_id: AssetId, name: Option<Vec<u8>>, deposit: Balance) -> Result<(), ()> {
	let nm = if let Some(n) = name {
		Some(n.try_into().map_err(|_| ())?)
	} else {
		None
	};

	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::update(
			RawOrigin::Root.into(),
			asset_id,
			nm,
			None,
			Some(deposit),
			None,
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}

// TODO: uncomment once AMM pool is available
// pub fn create_pool(who: AccountId, asset_a: AssetId, asset_b: AssetId, amount: Balance, price: Price) {
// 	assert_ok!(XYK::create_pool(
// 		RawOrigin::Signed(who).into(),
// 		asset_a,
// 		asset_b,
// 		amount,
// 		price
// 	));
// }
