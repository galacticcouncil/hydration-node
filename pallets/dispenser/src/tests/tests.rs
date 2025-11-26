use crate::{
	tests::{
		new_test_ext,
		utils::{acct, compute_request_id, create_test_receiver_address, create_test_tx_params},
		Currencies, Dispenser, RuntimeEvent, RuntimeOrigin, System, Test, MIN_WEI_BALANCE,
	},
	DispenserConfigData, Error, Event,
};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;

#[test]
fn test_cannot_initialize_twice() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		assert_noop!(
			Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE),
			Error::<Test>::AlreadyInitialized
		);

		assert_eq!(
			Dispenser::dispenser_config(),
			Some(DispenserConfigData {
				init: true,
				paused: false,
			})
		);
	});
}

#[test]
fn test_request_rejected_when_paused() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
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
fn test_invalid_request_id_reverts_balances() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let fee = <Test as crate::Config>::DispenserFee::get();
		let treasury = <Test as crate::Config>::TreasuryAddress::get();
		let pallet_account = Dispenser::account_id();

		let hdx_req_before = Currencies::free_balance(1, &requester);
		let hdx_treas_before = Currencies::free_balance(1, &treasury);
		let weth_treas_before = Currencies::free_balance(2, &treasury);
		let eth_req_before = Currencies::free_balance(2, &requester);
		let eth_pallet_before = Currencies::free_balance(2, &pallet_account);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx
		));

		assert_eq!(Currencies::free_balance(1, &requester), hdx_req_before - fee);
		assert_eq!(Currencies::free_balance(1, &treasury), hdx_treas_before + fee);
		assert_eq!(Currencies::free_balance(2, &treasury), weth_treas_before + amount);
		assert_eq!(Currencies::free_balance(2, &requester), eth_req_before - amount);
		assert_eq!(Currencies::free_balance(2, &pallet_account), eth_pallet_before + 0);
	});
}

#[test]
fn test_pause_unpause_state() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));
		assert_eq!(Dispenser::dispenser_config().unwrap().paused, true);

		assert_ok!(Dispenser::unpause(RuntimeOrigin::root()));
		assert_eq!(Dispenser::dispenser_config().unwrap().paused, false);
	});
}

#[test]
fn test_amount_too_small_and_too_large() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();

		let amt_small = (<Test as crate::Config>::MinimumRequestAmount::get() - 1) as u128;
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

		let amt_big = <Test as crate::Config>::MaxDispenseAmount::get() + 1;
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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let requester = acct(1);
		let receiver_address = create_test_receiver_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		let request_id = compute_request_id(requester.clone(), receiver_address, amount, &tx_params);
		let hdx_balance_before = Currencies::free_balance(1, &requester);
		let eth_balance_before = Currencies::free_balance(2, &requester);

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
					amount_wei: _amt,
				}) if rid == &request_id
					&& req == &requester
					&& to == &receiver_address
					&& amount == amount
			)
		}));

		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Signet(pallet_signet::Event::SignRespondRequested { .. })
			)
		}));

		assert_eq!(
			Currencies::free_balance(1, &requester),
			hdx_balance_before - <Test as crate::Config>::DispenserFee::get()
		);

		assert_eq!(Currencies::free_balance(2, &requester), eth_balance_before - amount);
	});
}

#[test]
fn governance_sets_faucet_balance_and_emits_event() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let old = Dispenser::current_faucet_balance_wei();
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), 42u128));
		assert_eq!(Dispenser::current_faucet_balance_wei(), 42u128);

		let ev = System::events().into_iter().any(|rec| {
			matches!(rec.event,
				RuntimeEvent::Dispenser(Event::FaucetBalanceUpdated {
					old_balance_wei, new_balance_wei
				}) if old_balance_wei == old && new_balance_wei == 42u128
			)
		});
		assert!(ev, "FaucetBalanceUpdated event not found");
	});
}

#[test]
fn non_governance_cannot_set_faucet_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let alice = acct(1);
		assert_noop!(
			Dispenser::set_faucet_balance(RuntimeOrigin::signed(alice), 7u128),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_eq!(Dispenser::current_faucet_balance_wei(), MIN_WEI_BALANCE);
	});
}

#[test]
fn request_rejected_when_balance_below_threshold() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), 10u128));

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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let amount = 101u128;
		let needed = <Test as crate::Config>::MinFaucetEthThreshold::get() + amount;
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), needed));

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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let amount: u128 = 1_000u128;
		let min_threshold = <Test as crate::Config>::MinFaucetEthThreshold::get();
		let initial_balance = min_threshold + amount + 1_000u128;

		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), initial_balance));
		assert_eq!(Dispenser::current_faucet_balance_wei(), initial_balance);

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let hdx_before = Currencies::free_balance(1, &requester);
		let weth_before = Currencies::free_balance(2, &requester);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx
		));

		let expected_balance = initial_balance.saturating_sub(amount);
		assert_eq!(Dispenser::current_faucet_balance_wei(), expected_balance);

		assert_eq!(
			Currencies::free_balance(1, &requester),
			hdx_before - <Test as crate::Config>::DispenserFee::get()
		);
		assert_eq!(Currencies::free_balance(2, &requester), weth_before - amount);
	});
}

#[test]
fn request_fails_before_initialize() {
	new_test_ext().execute_with(|| {
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 1_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::NotFound
		);
	});
}

#[test]
fn request_fails_with_zero_address() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let requester = acct(1);
		let receiver = [0u8; 20];
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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let requester = acct(99);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_eq!(Currencies::free_balance(1, &requester), 0);
		assert_eq!(Currencies::free_balance(2, &requester), 0);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::NotEnoughFunds
		);
	});
}

#[test]
fn request_fails_when_insufficient_faucet_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let requester = acct(55);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let fee = <Test as crate::Config>::DispenserFee::get();

		assert_ok!(Currencies::deposit(1, &requester, fee));

		assert_eq!(Currencies::free_balance(1, &requester), fee);
		assert_eq!(Currencies::free_balance(2, &requester), 0);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::NotEnoughFaucetFunds
		);
	});
}

#[test]
fn request_fails_with_duplicate_request_id() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

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
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let amount = 10_000u128;
		let min_threshold = <Test as crate::Config>::MinFaucetEthThreshold::get();
		let initial_balance = min_threshold + amount + 1_000u128;

		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), initial_balance));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let mut tx = create_test_tx_params();
		tx.gas_limit = 0;

		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amount, req_id, tx),
			Error::<Test>::InvalidOutput
		);
	});
}

#[test]
fn pause_fails_before_initialize() {
	new_test_ext().execute_with(|| {
		let origin = RuntimeOrigin::root();

		assert_noop!(Dispenser::pause(origin), Error::<Test>::NotFound);
	});
}

#[test]
fn unpause_fails_before_initialize() {
	new_test_ext().execute_with(|| {
		let origin = RuntimeOrigin::root();

		assert_noop!(Dispenser::unpause(origin), Error::<Test>::NotFound);
	});
}
