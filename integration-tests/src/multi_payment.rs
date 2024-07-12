#![cfg(test)]

use crate::{assert_balance, oracle::hydradx_run_to_block, polkadot_test_net::*};
use frame_support::dispatch::GetDispatchInfo;
use frame_support::dispatch::{DispatchClass, PostDispatchInfo};
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Omnipool;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::Tokens;
use hydradx_runtime::TransactionPayment;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::EVM;
use hydradx_runtime::XYK;
use hydradx_traits::router::PoolType;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::FeeCalculator;
use pallet_transaction_payment::ChargeTransactionPayment;
use primitives::constants::currency::UNITS;
use primitives::constants::time::HOURS;
use sp_core::Encode;
use sp_core::U256;
use sp_runtime::traits::SignedExtension;
use sp_runtime::DispatchResult;
use sp_runtime::{FixedU128, Permill, TransactionOutcome};
use test_utils::assert_eq_approx;
use xcm_emulator::TestExt;

#[test]
fn insufficient_asset_can_be_used_as_fee_currency() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

			//Arrange
			crate::dca::init_omnipool_with_oracle_for_block_10();
			crate::dca::add_dot_as_payment_currency();
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				DOT,
				HDX,
				10 * UNITS,
				u128::MIN
			));

			let name = b"INSUF1".to_vec();
			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();
			create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 3000000 * UNITS);

			set_relaychain_block_number(11);

			let alice_init_insuff_balance = 10 * UNITS;
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				insufficient_asset,
				alice_init_insuff_balance as i128,
			));

			let fee_currency = insufficient_asset;

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				fee_currency,
				FixedU128::from_rational(88, 100),
			));
			assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				fee_currency,
			));

			let omni_sell =
				hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
					asset_in: DOT,
					asset_out: 2,
					amount: UNITS,
					min_buy_amount: 0,
				});
			let info = omni_sell.get_dispatch_info();
			let info_len = 146;

			assert_balance!(&Treasury::account_id(), DOT, 0);

			//Act
			let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.pre_dispatch(&AccountId::from(ALICE), &omni_sell, &info, info_len);
			assert_ok!(&pre);
			assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
				Some(pre.unwrap()),
				&info,
				&default_post_info(),
				info_len,
				&Ok(())
			));

			//Assert
			let alice_new_insuff_balance = hydradx_runtime::Currencies::free_balance(insufficient_asset, &ALICE.into());
			assert!(alice_new_insuff_balance < alice_init_insuff_balance);

			let treasury_dot_balance = hydradx_runtime::Currencies::free_balance(DOT, &ALICE.into());
			assert!(treasury_dot_balance > 0, "Treasury is rugged");

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn insufficient_asset_should_not_be_set_as_currency_when_pool_doesnt_exist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

			//Arrange
			crate::dca::init_omnipool_with_oracle_for_block_10();
			//crate::dca::add_dot_as_payment_currency();

			let name = b"INSUF1".to_vec();
			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();
			//create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 3000000 * UNITS);

			assert_noop!(
				hydradx_runtime::MultiTransactionPayment::set_currency(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					insufficient_asset,
				),
				pallet_transaction_multi_payment::Error::<hydradx_runtime::Runtime>::UnsupportedCurrency
			);
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

fn default_post_info() -> PostDispatchInfo {
	PostDispatchInfo {
		actual_weight: None,
		pays_fee: Default::default(),
	}
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));
}

fn create_xyk_pool(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}
