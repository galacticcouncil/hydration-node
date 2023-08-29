#![cfg(feature = "runtime-benchmarks")]

pub mod currencies;
pub mod duster;
pub mod multi_payment;
pub mod omnipool;
pub mod route_executor;
pub mod tokens;
pub mod vesting;

use crate::AssetRegistry;
use frame_system::RawOrigin;

use frame_support::dispatch::DispatchError;
use hydradx_traits::Registry;
use primitives::{AssetId, Balance};
use sp_std::vec;
use sp_std::vec::Vec;

pub const BSX: Balance = primitives::constants::currency::UNITS;

pub fn register_asset(name: Vec<u8>, deposit: Balance) -> Result<AssetId, ()> {
	<AssetRegistry as Registry<AssetId, Vec<u8>, Balance, DispatchError>>::create_asset(&name, deposit, false)
		.map_err(|_| ())
}

#[allow(dead_code)]
pub fn update_asset(asset_id: AssetId, name: Vec<u8>, deposit: Balance) -> Result<(), ()> {
	AssetRegistry::update(
		RawOrigin::Root.into(),
		asset_id,
		Some(name),
		Some(pallet_asset_registry::AssetType::<AssetId>::Token),
		Some(deposit),
		None,
		None,
		None,
		None,
	)
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
