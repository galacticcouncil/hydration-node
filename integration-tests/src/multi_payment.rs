#![cfg(test)]

use crate::erc20::{bind_erc20, deploy_token_contract};
use crate::{assert_balance, polkadot_test_net::*};
use frame_support::dispatch::GetDispatchInfo;
use frame_support::dispatch::PostDispatchInfo;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::Router;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::{AssetRegistry, TreasuryAccount};
use hydradx_runtime::{FixedU128, Omnipool};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_transaction_payment::ChargeTransactionPayment;
use primitives::constants::currency::UNITS;
use sp_core::Get;
use sp_runtime::traits::{DispatchTransaction, TransactionExtension};
use sp_runtime::DispatchResult;
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;
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

			go_to_block(11);

			let alice_init_insuff_balance = 10 * UNITS;
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				insufficient_asset,
				alice_init_insuff_balance as i128,
			));

			let fee_currency = insufficient_asset;

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
				.validate_and_prepare(Some(AccountId::from(ALICE)).into(), &omni_sell, &info, info_len, 0);
			assert_ok!(&pre);
			let (pre_data, _origin) = pre.unwrap();
			assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
				pre_data,
				&info,
				&mut PostDispatchInfo::default(),
				info_len,
				&Ok(())
			));

			//Assert
			let alice_new_insuff_balance = hydradx_runtime::Currencies::free_balance(insufficient_asset, &ALICE.into());
			assert!(alice_new_insuff_balance < alice_init_insuff_balance);

			let treasury_insuff_balance =
				hydradx_runtime::Currencies::free_balance(insufficient_asset, &TreasuryAccount::get());
			assert_eq!(
				treasury_insuff_balance, 0,
				"Treasury should not have accumulated insuff asset"
			);

			let treasury_dot_balance = hydradx_runtime::Currencies::free_balance(DOT, &TreasuryAccount::get());
			assert!(
				treasury_dot_balance > 0,
				"Treasury should have received DOT swapped from insuff asset"
			);

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

#[test]
fn sufficient_but_not_accepted_asset_can_be_used_as_fee_currency() {
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
			let sufficient_but_not_accepted_asset = AssetRegistry::register_sufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				1_000,
				None,
				None,
				None,
				None,
			)
			.unwrap();
			create_xyk_pool(sufficient_but_not_accepted_asset, 1000000 * UNITS, DOT, 3000000 * UNITS);

			go_to_block(11);

			let alice_init_suff_balance = 10 * UNITS;
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				sufficient_but_not_accepted_asset,
				alice_init_suff_balance as i128,
			));

			let fee_currency = sufficient_but_not_accepted_asset;

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
				.validate_and_prepare(Some(AccountId::from(ALICE)).into(), &omni_sell, &info, info_len, 0);
			assert_ok!(&pre);
			let (pre_data, _origin) = pre.unwrap();
			assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
				pre_data,
				&info,
				&mut PostDispatchInfo::default(),
				info_len,
				&Ok(())
			));

			//Assert
			let alice_new_suff_balance =
				hydradx_runtime::Currencies::free_balance(sufficient_but_not_accepted_asset, &ALICE.into());
			assert!(alice_new_suff_balance < alice_init_suff_balance);

			let treasury_suff_balance =
				hydradx_runtime::Currencies::free_balance(sufficient_but_not_accepted_asset, &TreasuryAccount::get());
			assert_eq!(
				treasury_suff_balance, 0,
				"Treasury should not have accumulated non accepted suff asset"
			);

			let treasury_dot_balance = hydradx_runtime::Currencies::free_balance(DOT, &TreasuryAccount::get());
			assert!(
				treasury_dot_balance > 0,
				"Treasury should have received DOT swapped from non accepted suff asset"
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn erc20_can_be_used_as_fee_currency() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let fee_currency = bind_erc20(contract);
		let _ = with_transaction(|| {
			//Arrange
			let init_balance = hydradx_runtime::Currencies::free_balance(fee_currency, &ALICE.into());
			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				fee_currency,
				FixedU128::from_rational(1, 100000),
			));
			hydradx_run_to_next_block();
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

			assert_balance!(&Treasury::account_id(), fee_currency, 0);

			//Act
			let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.pre_dispatch(&AccountId::from(ALICE), &omni_sell, &info, info_len);
			assert_ok!(&pre);
			assert_ok!(ChargeTransactionPayment::<hydradx_runtime::Runtime>::post_dispatch(
				Some(pre.unwrap()),
				&info,
				&PostDispatchInfo::default(),
				info_len,
				&Ok(())
			));

			//Assert
			let after_balance = hydradx_runtime::Currencies::free_balance(fee_currency, &ALICE.into());
			assert!(after_balance < init_balance);
			assert_eq!(
				hydradx_runtime::Currencies::free_balance(fee_currency, &TreasuryAccount::get()),
				init_balance - after_balance
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn set_currency_in_batch_should_fail_for_unaccepted_asset_with_oracle_price() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			let dot = DOT;
			crate::dca::init_omnipool_with_oracle_for_block_10();
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				dot,
				1000 * UNITS as i128,
			));
			assert_ok!(Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				dot,
				FixedU128::from_rational(1, 100),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(dot, DOT_ASSET_LOCATION));
			set_ed(dot, 100000);

			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				dot,
				1 * UNITS,
				u128::MIN,
				vec![].try_into().unwrap(),
			));
			hydradx_run_to_next_block();

			//create_xyk_pool(dot, 1000000 * UNITS, DAI, 3000000 * UNITS);

			// Ensure DAI is removed from accepted currencies.
			assert_ok!(hydradx_runtime::MultiTransactionPayment::remove_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DAI,
			));

			// Verify set_currency fails individually
			assert_noop!(
				hydradx_runtime::MultiTransactionPayment::set_currency(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					DAI,
				),
				pallet_transaction_multi_payment::Error::<hydradx_runtime::Runtime>::UnsupportedCurrency
			);

			// Act: Batch with set_currency as first item
			let set_currency_call =
				hydradx_runtime::RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::<
					hydradx_runtime::Runtime,
				>::set_currency {
					currency: DAI,
				});

			let batch =
				hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::<hydradx_runtime::Runtime>::batch {
					calls: vec![set_currency_call],
				});

			let info = batch.get_dispatch_info();
			let info_len = 146;

			let alice_init_dai_balance = hydradx_runtime::Currencies::free_balance(DAI, &ALICE.into());

			let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.pre_dispatch(&AccountId::from(ALICE), &batch, &info, info_len);

			let alice_dai_balance_after = hydradx_runtime::Currencies::free_balance(DAI, &ALICE.into());
			let dai_fee_charged = alice_init_dai_balance - alice_dai_balance_after;
			assert_eq!(dai_fee_charged, 0, "No fee should be charged if pre-dispatch fails");

			assert!(pre.is_err(), "Should fail to pay fee with unaccepted currency in batch");

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

fn set_ed(asset_id: AssetId, ed: u128) {
	AssetRegistry::update(
		hydradx_runtime::RuntimeOrigin::root(),
		asset_id,
		None,
		None,
		Some(ed),
		None,
		None,
		None,
		None,
		None,
	)
	.unwrap();
}
