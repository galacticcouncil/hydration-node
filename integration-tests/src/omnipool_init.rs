#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};

use orml_traits::currency::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::FixedPointNumber;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const PATH_TO_SNAPSHOT: &str = "omnipool-snapshot/SNAPSHOT";

#[test]
fn omnipool_launch_init_params_should_be_correct() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();
		let stable_amount = 50_000 * UNITS * 1_000_000;
		let native_amount = 936_329_588_000_000_000;
		let dot_amount = 87_719_298_250_000_u128;
		let eth_amount = 63_750_000_000_000_000_000u128;
		let btc_amount = 1_000_000_000u128;

		init_omnipool();

		let hdx_balance = hydradx_runtime::Balances::free_balance(&omnipool_account);
		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &omnipool_account);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_account);

		assert_eq!(lrna_balance, 3374999999982000);
		assert_eq!(dai_balance, stable_amount);
		assert_eq!(hdx_balance, native_amount);

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		let hdx_balance = hydradx_runtime::Balances::free_balance(&omnipool_account);
		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &omnipool_account);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_account);
		let dot_balance = hydradx_runtime::Tokens::free_balance(DOT, &omnipool_account);

		assert_eq!(lrna_balance, 5625000000094500);
		assert_eq!(dai_balance, stable_amount);
		assert_eq!(hdx_balance, native_amount);
		assert_eq!(dot_balance, dot_amount);

		let token_price = FixedU128::from_inner(71_145_071_145_071);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			ETH,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		let hdx_balance = hydradx_runtime::Balances::free_balance(&omnipool_account);
		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &omnipool_account);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_account);
		let dot_balance = hydradx_runtime::Tokens::free_balance(DOT, &omnipool_account);
		let eth_balance = hydradx_runtime::Tokens::free_balance(ETH, &omnipool_account);

		assert_eq!(lrna_balance, 10160498285592776);
		assert_eq!(dai_balance, stable_amount);
		assert_eq!(hdx_balance, native_amount);
		assert_eq!(dot_balance, dot_amount);
		assert_eq!(eth_balance, eth_amount);

		let charlie_dai_orig = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(CHARLIE));

		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
			ETH,
			DAI,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let expected = 1664733011875663575256u128;

		let charlie_dai = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(CHARLIE));

		let paid = charlie_dai_orig - charlie_dai;
		assert_eq!(paid, expected);

		let btc_price = FixedU128::from_inner(9_647_109_647_109_650_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			BTC,
			btc_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		let btc_balance = hydradx_runtime::Tokens::free_balance(BTC, &omnipool_account);

		assert_eq!(btc_balance, btc_amount);

		let charlie_dai_orig = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(CHARLIE));

		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
			BTC,
			DAI,
			100_000_000 / 10,
			u128::MAX,
		));

		let expected = 2_428_053_975_026_574_531_220u128;

		let charlie_dai = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(CHARLIE));

		let paid = charlie_dai_orig - charlie_dai;
		assert_eq!(paid, expected);
	});
}

pub fn hydra_run_to_block(to: BlockNumber) {
	use frame_support::traits::{OnFinalize, OnInitialize};
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);
		hydradx_runtime::EmaOracle::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);
		hydradx_runtime::EmaOracle::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

#[ignore]
#[test]
fn add_liquidity_should_fail_when_price_changes() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let acc = AccountId::from(ALICE);
		let eth_precision = 1_000_000_000_000_000_000u128;

		assert_eq!(hydradx_runtime::System::block_number(), 2131225);

		orml_tokens::Pallet::<hydradx_runtime::Runtime>::update_balance(ETH, &acc, 1000 * eth_precision as i128)
			.unwrap();
		orml_tokens::Pallet::<hydradx_runtime::Runtime>::update_balance(DAI, &acc, 115_000 * eth_precision as i128)
			.unwrap();

		// first do a trade to populate the oracle
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ETH,
			DAI,
			eth_precision,
			0,
		));

		hydra_run_to_block(2131226);

		// then do a trade that moves the price
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ETH,
			DAI,
			100 * eth_precision,
			0,
		));

		hydra_run_to_block(2131227);

		assert_noop!(
			hydradx_runtime::Omnipool::add_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				DAI,
				11_500_000_000_000_000_000_000u128,
			),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::PriceDifferenceTooHigh,
		);
	});
}

#[ignore]
#[test]
fn add_liquidity_should_fail_when_price_changes_across_multiple_block() {
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let acc = AccountId::from(ALICE);
		let eth_precision = 1_000_000_000_000_000_000u128;

		orml_tokens::Pallet::<hydradx_runtime::Runtime>::update_balance(ETH, &acc, 1000 * eth_precision as i128)
			.unwrap();
		orml_tokens::Pallet::<hydradx_runtime::Runtime>::update_balance(DAI, &acc, 115_000 * eth_precision as i128)
			.unwrap();

		assert_eq!(hydradx_runtime::System::block_number(), 2131225);

		for idx in 1..10 {
			assert_ok!(hydradx_runtime::Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				ETH,
				DAI,
				10 * eth_precision,
				0,
			));

			hydra_run_to_block(2131225 + idx as u32);
		}

		assert_noop!(
			hydradx_runtime::Omnipool::add_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				DAI,
				11_500_000_000_000_000_000_000u128,
			),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::PriceDifferenceTooHigh,
		);
	});
}

#[test]
fn withdraw_pool_liquidity_should_work_when_withdrawing_all() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		let bob_account = AccountId::from(UNKNOWN);
		let stable_price = FixedU128::from_inner(45_000_000_000);
		let state = pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_asset_state(DAI).unwrap();
		let dai_reserve = state.reserve;

		assert_ok!(hydradx_runtime::Omnipool::withdraw_protocol_liquidity(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			state.protocol_shares,
			(stable_price.into_inner(), FixedU128::DIV),
			bob_account.clone()
		));

		let state = pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_asset_state(DAI).unwrap();
		assert_eq!(state.reserve, 0);
		assert_eq!(state.hub_reserve, 0);
		assert_eq!(state.shares, 0);
		assert_eq!(state.protocol_shares, 0);

		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &bob_account);
		assert!(dai_balance <= dai_reserve);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &bob_account);
		assert_eq!(lrna_balance, 0);
	});
}

use pallet_omnipool::types::Tradability;
#[test]
fn removing_token_should_work_when_no_shares_remaining() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		let bob_account = AccountId::from(UNKNOWN);
		let dot_amount = 87_719_298_250_000_u128;

		let position_id = hydradx_runtime::Omnipool::next_position_id();

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			bob_account.clone(),
		));

		hydra_run_to_block(10);

		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
		));

		let position =
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, bob_account.clone())
				.unwrap();
		assert_ok!(hydradx_runtime::Omnipool::remove_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(UNKNOWN.into()),
			position_id,
			position.shares,
		));

		let dot_balance = hydradx_runtime::Tokens::free_balance(DOT, &bob_account);
		assert!(dot_balance <= dot_amount);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &bob_account);
		assert_eq!(lrna_balance, 0);

		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Tradability::FROZEN
		));
		assert_ok!(hydradx_runtime::Omnipool::remove_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			bob_account.clone(),
		));
		let dot_balance = hydradx_runtime::Tokens::free_balance(DOT, &bob_account);
		assert!(dot_balance == dot_amount);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &bob_account);
		assert_eq!(lrna_balance, 0);
	});
}
