use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use hydradx_runtime::{AssetRegistry, Balances, Currencies, Stableswap, Router};
use hydradx_runtime::{Omnipool, RuntimeOrigin, Tokens};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use primitives::{AssetId, Balance};
use sp_runtime::Permill;
use sp_runtime::{DispatchError, FixedU128, TransactionOutcome};
use hydradx_traits::router::PoolType;
use hydradx_traits::router::Trade;
pub fn setup_omnipool() {
	initialize_omnipol();
	for _ in 0..10 {
		hydradx_run_to_next_block();
		do_trade_to_populate_oracle(DOT, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(DAI, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
	}
}

pub fn setup_omnipool_with_stable_subpool() -> (AssetId, Vec<AssetId>) {
	setup_omnipool();
	let (pool_id, asset_a, asset_b) = with_transaction(
		|| -> TransactionOutcome<Result<(AssetId, AssetId, AssetId), DispatchError>> {
			let r = initialize_stableswap();
			TransactionOutcome::Commit(Ok(r))
		},
	)
	.unwrap();

	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		Omnipool::protocol_account(),
		pool_id,
		(3000 * UNITS * 1_000_000u128) as i128,
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		pool_id,
		FixedU128::from_inner(25_650_000_000_000_000),
		Permill::from_percent(1),
		AccountId::from(BOB),
	));
	for _ in 0..10 {
		hydradx_run_to_next_block();
		do_trade_to_populate_stable_oracle(pool_id,asset_a, 1_000_000_000_000);
		do_trade_to_populate_stable_oracle(pool_id, asset_b, 1_000_000_000_000);
	}

	(pool_id, vec![asset_a, asset_b])
}
fn do_trade_to_populate_stable_oracle(pool_id: AssetId, asset_id: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));
	let route = vec![
		Trade {
			pool: PoolType::Omnipool,
			asset_in: LRNA,
			asset_out: pool_id,
		},
		Trade {
			pool: PoolType::Stableswap(pool_id),
			asset_in: pool_id,
			asset_out: asset_id,
		},
	];


	assert_ok!(Router::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_id,
		amount,
		Balance::MIN,
		route,
	));

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

fn initialize_omnipol() {
	let native_price = FixedU128::from_rational(29903049701668757, 73927734532192294158);
	let dot_price = FixedU128::from_rational(103158291366950047, 4566210555614178);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let acc = hydradx_runtime::Omnipool::protocol_account();

	let stable_amount = 50_000_000 * UNITS * 1_000_000;
	let dot_amount: Balance = 4566210555614178u128;
	let native_amount: Balance = 73927734532192294158u128;
	let weth_amount: Balance = 1074271742496220564487u128;
	let weth_price = FixedU128::from_rational(67852651072676287, 1074271742496220564487);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DOT,
		dot_amount,
		0
	));
	Balances::set_balance(&acc, native_amount);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		WETH,
		weth_amount,
		0
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		WETH,
		weth_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), acc, DAI, stable_amount, 0));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));
}

fn initialize_stableswap() -> (AssetId, AssetId, AssetId) {
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	initialize_stableswap_with_details(initial_liquidity, liquidity_added, 18)
}

fn initialize_stableswap_with_details(
	initial_liquidity: Balance,
	liquidity_added: Balance,
	decimals: u8,
) -> (AssetId, AssetId, AssetId) {
	let mut initial: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> =
		vec![];

	let mut asset_ids: Vec<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let result = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.clone().try_into().unwrap()),
			AssetKind::Token,
			1000u128,
			Some(name.try_into().unwrap()),
			Some(decimals),
			None,
			None,
		);
		assert_ok!(result);
		let asset_id = result.unwrap();
		asset_ids.push(asset_id);

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		));
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(CHARLIE),
			asset_id,
			initial_liquidity as i128,
		));

		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let result = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	);

	assert_ok!(result);

	let pool_id = result.unwrap();

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	assert_ok!(Stableswap::create_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		pool_id,
		asset_ids,
		amplification,
		fee,
	));

	assert_ok!(Stableswap::add_liquidity(
		hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
		pool_id,
		initial
	));

	(pool_id, asset_in, asset_out)
}
