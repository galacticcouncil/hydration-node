use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{
	AssetRegistry, Balances, Currencies, MultiTransactionPayment, Omnipool, Runtime, RuntimeOrigin, Stableswap, Tokens,
	Treasury,
};
use hydradx_traits::registry::{AssetKind, Create};
use hydradx_traits::stableswap::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use sp_runtime::{BoundedVec, DispatchError, FixedU128, Permill};

const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

/// HDX + DAI omnipool with a funded treasury and zero referral rewards.
pub fn init_omnipol() {
	let native_price = FixedU128::from_float(0.5);
	let stable_price = FixedU128::from_float(0.7);
	let acc = Omnipool::protocol_account();

	let stable_amount: Balance = 5_000_000_000_000_000_000_000u128;
	let native_amount: Balance = 5_000_000_000_000_000_000_000u128;
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DAI,
		stable_amount,
		0
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		acc,
		HDX,
		native_amount as i128,
	));

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		Omnipool::protocol_account(),
	));

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(60),
		Omnipool::protocol_account(),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));

	set_zero_reward_for_referrals(HDX);
	set_zero_reward_for_referrals(DAI);
}

pub fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	go_to_block(10);
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}

pub fn add_dot_as_payment_currency() {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		Omnipool::protocol_account(),
		DOT,
		(1000 * UNITS) as i128,
	));

	assert_ok!(MultiTransactionPayment::add_currency(
		RuntimeOrigin::root(),
		DOT,
		FixedU128::from_rational(1, 100000),
	));

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DOT,
		FixedU128::from_rational(10, 100),
		Permill::from_percent(100),
		AccountId::from(BOB),
	));
}

/// A `MAX_ASSETS_IN_POOL`-asset stableswap pool funded by BOB. Returns `(pool_id, asset_in, asset_out)`.
pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.try_into().unwrap()),
			AssetKind::Token,
			1u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(18u8),
			None,
			None,
		)?;

		asset_ids.push(asset_id);
		Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
	}
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(
		RuntimeOrigin::root(),
		pool_id,
		BoundedVec::truncate_from(asset_ids),
		amplification,
		fee,
	)?;

	Stableswap::add_assets_liquidity(
		RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(initial),
		Balance::zero(),
	)?;

	Ok((pool_id, asset_in, asset_out))
}
