use crate::{self as pallet_dispenser};
use crate::{
	tests::{
		new_test_ext, test_faucet_address,
		utils::{acct, compute_request_id, create_test_receiver_address, create_test_tx_params},
		Currencies, Dispenser, RuntimeEvent, RuntimeOrigin, System, Test, MIN_WEI_BALANCE, TEST_DISPENSER_FEE,
		TEST_MAX_DISPENSE, TEST_MIN_FAUCET_THRESHOLD, TEST_MIN_REQUEST,
	},
	Error, Event,
};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use sp_runtime::BuildStorage;

#[test]
fn test_request_rejected_when_paused() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 1_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let hdx_before = Currencies::free_balance(1, &requester);
		let eth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester.clone()), receiver, amount, req_id, tx),
			Error::<Test>::Paused
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), eth_before);
	});
}

#[test]
fn test_request_rejected_when_not_configured() {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		frame_system::Pallet::<Test>::set_block_number(1);
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 1_000u128;
		let tx = create_test_tx_params();

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, [0u8; 32], tx),
			Error::<Test>::NotConfigured
		);
	});
}

#[test]
fn test_invalid_request_id_reverts_balances() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 123_456u128;
		let tx = create_test_tx_params();

		let bad_req_id = [9u8; 32];
		let hdx_before = Currencies::free_balance(1, &requester);
		let eth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(
				RuntimeOrigin::signed(requester.clone()),
				receiver,
				amount,
				bad_req_id,
				tx
			),
			Error::<Test>::InvalidRequestId
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), eth_before);
	});
}

#[test]
fn test_fee_and_asset_routing() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let config = Dispenser::dispenser_config().unwrap();
		let fee = config.dispenser_fee;
		let treasury = <Test as crate::Config>::FeeDestination::get();
		let pallet_account = Dispenser::account_id();

		let fee_asset = <Test as pallet_dispenser::Config>::FeeAsset::get();
		let faucet_asset = <Test as pallet_dispenser::Config>::FaucetAsset::get();

		let hdx_req_before = Currencies::free_balance(fee_asset, &requester);
		let hdx_treas_before = Currencies::free_balance(fee_asset, &treasury);
		let weth_treas_before = Currencies::free_balance(faucet_asset, &treasury);
		let eth_req_before = Currencies::free_balance(faucet_asset, &requester);
		let eth_pallet_before = Currencies::free_balance(faucet_asset, &pallet_account);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx
		));

		assert_eq!(Currencies::free_balance(fee_asset, &requester), hdx_req_before - fee);
		assert_eq!(Currencies::free_balance(fee_asset, &treasury), hdx_treas_before + fee);
		assert_eq!(
			Currencies::free_balance(faucet_asset, &treasury),
			weth_treas_before + amount
		);
		assert_eq!(
			Currencies::free_balance(faucet_asset, &requester),
			eth_req_before - amount
		);
		assert_eq!(
			Currencies::free_balance(faucet_asset, &pallet_account),
			eth_pallet_before
		);
	});
}

#[test]
fn test_pause_unpause_state() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));
		assert!(Dispenser::dispenser_config().unwrap().paused);

		assert_ok!(Dispenser::unpause(RuntimeOrigin::root()));
		assert!(!Dispenser::dispenser_config().unwrap().paused);
	});
}

#[test]
fn test_amount_too_small_and_too_large() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();

		let amt_small = TEST_MIN_REQUEST - 1;
		let rid_small = compute_request_id(requester.clone(), receiver, amt_small, &tx);
		assert_noop!(
			Dispenser::request_fund(
				RuntimeOrigin::signed(requester.clone()),
				receiver,
				amt_small,
				rid_small,
				tx.clone()
			),
			Error::<Test>::AmountTooSmall
		);

		let amt_big = TEST_MAX_DISPENSE + 1;
		let rid_big = compute_request_id(requester.clone(), receiver, amt_big, &tx);
		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amt_big, rid_big, tx),
			Error::<Test>::AmountTooLarge
		);
	});
}

#[test]
fn test_deposit_erc20_success() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver_address = create_test_receiver_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		let fee_asset = <Test as pallet_dispenser::Config>::FeeAsset::get();
		let faucet_asset = <Test as pallet_dispenser::Config>::FaucetAsset::get();

		let request_id = compute_request_id(requester.clone(), receiver_address, amount, &tx_params);
		let hdx_balance_before = Currencies::free_balance(fee_asset, &requester);
		let eth_balance_before = Currencies::free_balance(faucet_asset, &requester);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver_address,
			amount,
			request_id,
			tx_params,
		));

		let events = System::events();
		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Dispenser(Event::FundRequested {
					request_id: rid,
					requester: req,
					to,
					amount: _amt,
				}) if rid == &request_id
					&& req == &requester
					&& to == &receiver_address
					&& amount == amount
			)
		}));

		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Signet(pallet_signet::Event::SignBidirectionalRequested { .. })
			)
		}));

		let config = Dispenser::dispenser_config().unwrap();
		assert_eq!(
			Currencies::free_balance(fee_asset, &requester),
			hdx_balance_before - config.dispenser_fee
		);

		assert_eq!(
			Currencies::free_balance(faucet_asset, &requester),
			eth_balance_before - amount
		);
	});
}

#[test]
fn test_set_config_works() {
	new_test_ext().execute_with(|| {
		let new_address = primitives::EvmAddress::from([2u8; 20]);
		assert_ok!(Dispenser::set_config(
			RuntimeOrigin::root(),
			new_address,
			500,
			200,
			2_000_000_000,
			25,
			999,
		));

		let config = Dispenser::dispenser_config().unwrap();
		assert_eq!(config.faucet_address, new_address);
		assert_eq!(config.min_faucet_threshold, 500);
		assert_eq!(config.min_request, 200);
		assert_eq!(config.max_dispense, 2_000_000_000);
		assert_eq!(config.dispenser_fee, 25);
		assert_eq!(config.faucet_balance_wei, 999);
		// paused state preserved from previous set_config (was false)
		assert!(!config.paused);
	});
}

#[test]
fn test_set_config_preserves_paused_state() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));
		assert!(Dispenser::dispenser_config().unwrap().paused);

		assert_ok!(Dispenser::set_config(
			RuntimeOrigin::root(),
			test_faucet_address(),
			1,
			100,
			1_000_000_000,
			10,
			MIN_WEI_BALANCE,
		));

		// paused should still be true
		assert!(Dispenser::dispenser_config().unwrap().paused);
	});
}

#[test]
fn non_governance_cannot_set_config() {
	new_test_ext().execute_with(|| {
		let alice = acct(1);
		assert_noop!(
			Dispenser::set_config(
				RuntimeOrigin::signed(alice),
				test_faucet_address(),
				1,
				100,
				1_000_000_000,
				10,
				MIN_WEI_BALANCE,
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn request_rejected_when_balance_below_threshold() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();

		// Set config with very low faucet balance
		assert_ok!(Dispenser::set_config(
			RuntimeOrigin::root(),
			test_faucet_address(),
			TEST_MIN_FAUCET_THRESHOLD,
			TEST_MIN_REQUEST,
			TEST_MAX_DISPENSE,
			TEST_DISPENSER_FEE,
			100u128, // low balance
		));

		let amount = 100u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let hdx_before = Currencies::free_balance(1, &requester);
		let weth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester.clone()), receiver, amount, req_id, tx),
			Error::<Test>::FaucetBalanceBelowThreshold
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), weth_before);
	});
}

#[test]
fn request_allowed_at_or_above_threshold() {
	new_test_ext().execute_with(|| {
		let amount = 101u128;
		let needed = TEST_MIN_FAUCET_THRESHOLD + amount;

		// Set config with enough balance
		assert_ok!(Dispenser::set_config(
			RuntimeOrigin::root(),
			test_faucet_address(),
			TEST_MIN_FAUCET_THRESHOLD,
			TEST_MIN_REQUEST,
			TEST_MAX_DISPENSE,
			TEST_DISPENSER_FEE,
			needed,
		));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester),
			receiver,
			amount,
			req_id,
			tx
		));
	});
}

#[test]
fn request_reduces_faucet_balance() {
	new_test_ext().execute_with(|| {
		let amount: u128 = 1_000u128;
		let initial_balance = TEST_MIN_FAUCET_THRESHOLD + amount + 1_000u128;

		assert_ok!(Dispenser::set_config(
			RuntimeOrigin::root(),
			test_faucet_address(),
			TEST_MIN_FAUCET_THRESHOLD,
			TEST_MIN_REQUEST,
			TEST_MAX_DISPENSE,
			TEST_DISPENSER_FEE,
			initial_balance,
		));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx
		));

		let config = Dispenser::dispenser_config().unwrap();
		assert_eq!(config.faucet_balance_wei, initial_balance - amount);
	});
}

#[test]
fn request_fails_with_zero_address() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = primitives::EvmAddress::zero();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::InvalidAddress
		);
	});
}

#[test]
fn request_fails_when_insufficient_fee_balance() {
	new_test_ext().execute_with(|| {
		let requester = acct(99);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_eq!(Currencies::free_balance(1, &requester), 0);
		assert_eq!(Currencies::free_balance(2, &requester), 0);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::NotEnoughFeeFunds
		);
	});
}

#[test]
fn request_fails_when_insufficient_faucet_balance() {
	new_test_ext().execute_with(|| {
		let requester = acct(55);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let fee_asset = <Test as pallet_dispenser::Config>::FeeAsset::get();
		let config = Dispenser::dispenser_config().unwrap();
		let fee = config.dispenser_fee;

		let _ = Currencies::deposit(fee_asset, &requester, 1_000_000_000_000_000_000_000);
		assert_ok!(Currencies::deposit(1, &requester, fee));

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::NotEnoughFaucetFunds
		);
	});
}

#[test]
fn request_fails_with_duplicate_request_id() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx.clone()
		));

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::DuplicateRequest
		);
	});
}

#[test]
fn request_fails_with_zero_gas_limit() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let mut tx = create_test_tx_params();
		tx.gas_limit = 0;

		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::InvalidOutput
		);
	});
}
