#![cfg(feature = "runtime-benchmarks")]

pub mod currencies;
pub mod dca;
pub mod duster;
pub mod dynamic_evm_fee;
pub mod multi_payment;
pub mod omnipool;
pub mod route_executor;
pub mod tokens;
pub mod vesting;

use crate::{AssetLocation, AssetRegistry, MultiTransactionPayment};
use frame_system::RawOrigin;

use pallet_transaction_multi_payment::Price;
use primitives::{AssetId, Balance};
use sp_std::vec;
use sp_std::vec::Vec;

pub const BSX: Balance = primitives::constants::currency::UNITS;

pub fn register_asset(name: Vec<u8>, deposit: Balance) -> Result<AssetId, ()> {
	AssetRegistry::register_asset(
		AssetRegistry::to_bounded_name(name).map_err(|_| ())?,
		pallet_asset_registry::AssetType::<AssetId>::Token,
		deposit,
		None,
		None,
	)
	.map_err(|_| ())
}

pub fn set_location(asset_id: AssetId, location: AssetLocation) -> Result<(), ()> {
	AssetRegistry::set_location(RawOrigin::Root.into(), asset_id, location).map_err(|_| ())
}

pub fn add_as_accepted_currency(asset_id: AssetId, price: Price) -> Result<(), ()> {
	MultiTransactionPayment::add_currency(RawOrigin::Root.into(), asset_id, price).map_err(|_| ())
}

#[allow(dead_code)]
pub fn update_asset(asset_id: AssetId, name: Vec<u8>, deposit: Balance) -> Result<(), ()> {
	AssetRegistry::update(
		RawOrigin::Root.into(),
		asset_id,
		name,
		pallet_asset_registry::AssetType::<AssetId>::Token,
		Some(deposit),
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
