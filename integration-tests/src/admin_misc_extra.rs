// Integration coverage for admin / utility extrinsics that had no
// integration-test coverage:
//   - `pallet-dispatcher::dispatch_as_treasury`
//   - `pallet-transaction-multi-payment::reset_payment_currency`
//   - `pallet-gigahdx::set_pool_contract`
//   - the `pallet-dispenser` surface: `set_config`, `pause`, `unpause`,
//     and `request_fund` guard rejections.
//
// These are origin-gated administrative calls, so the assertions focus on the
// authority gate, the specific error variants, and the observable storage /
// balance effect.

#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use primitives::EvmAddress;
use xcm_emulator::TestExt;

// ------------------------- pallet-dispatcher -------------------------

#[test]
fn dispatch_as_treasury_should_transfer_from_treasury_when_root() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let treasury = TreasuryAccount::get();

		// Fund the treasury so the inner transfer has something to move.
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			treasury.clone(),
			HDX,
			1_000 * UNITS as i128,
		));

		let treasury_before = Currencies::free_balance(HDX, &treasury);
		let bob_before = Currencies::free_balance(HDX, &AccountId::from(BOB));

		let inner = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: HDX,
			amount: 100 * UNITS,
		});

		assert_ok!(Dispatcher::dispatch_as_treasury(RuntimeOrigin::root(), Box::new(inner),));

		// The inner call runs as a Signed origin from the treasury account.
		assert_eq!(
			Currencies::free_balance(HDX, &AccountId::from(BOB)),
			bob_before + 100 * UNITS
		);
		assert_eq!(Currencies::free_balance(HDX, &treasury), treasury_before - 100 * UNITS);
	});
}

#[test]
fn dispatch_as_treasury_should_fail_when_origin_not_authorized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let inner = RuntimeCall::System(frame_system::Call::remark { remark: vec![1, 2, 3] });

		assert_noop!(
			Dispatcher::dispatch_as_treasury(RuntimeOrigin::signed(ALICE.into()), Box::new(inner)),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ------------------- pallet-transaction-multi-payment -------------------

#[test]
fn reset_payment_currency_should_clear_account_currency_when_root() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let alice: AccountId = ALICE.into();

		// DAI is an accepted currency at genesis, so a non-EVM account can set it.
		assert_ok!(MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(alice.clone()),
			DAI,
		));
		assert_eq!(MultiTransactionPayment::get_currency(&alice), Some(DAI));

		// Reset of a non-EVM account removes the override entirely (defaults to HDX).
		assert_ok!(MultiTransactionPayment::reset_payment_currency(
			RuntimeOrigin::root(),
			alice.clone(),
		));
		assert_eq!(MultiTransactionPayment::get_currency(&alice), None);
	});
}

#[test]
fn reset_payment_currency_should_fail_when_origin_not_authorized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			MultiTransactionPayment::reset_payment_currency(RuntimeOrigin::signed(ALICE.into()), BOB.into()),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ------------------------- pallet-gigahdx -------------------------

fn pool_addr_a() -> EvmAddress {
	hex!["1111111111111111111111111111111111111111"].into()
}

fn pool_addr_b() -> EvmAddress {
	hex!["2222222222222222222222222222222222222222"].into()
}

#[test]
fn set_pool_contract_should_update_stored_address_when_root_and_no_stake() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// No stHDX has been minted on the vanilla test net, so the pool is settable.
		assert_eq!(GigaHdx::total_gigahdx_supply(), 0);

		assert_ok!(GigaHdx::set_pool_contract(RuntimeOrigin::root(), pool_addr_a()));
		assert_eq!(
			pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get(),
			Some(pool_addr_a())
		);

		// Idempotent-friendly: a second set with a different address overwrites it.
		assert_ok!(GigaHdx::set_pool_contract(RuntimeOrigin::root(), pool_addr_b()));
		assert_eq!(
			pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get(),
			Some(pool_addr_b())
		);
	});
}

#[test]
fn set_pool_contract_should_fail_when_stake_outstanding() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sthdx: AssetId = <Runtime as pallet_gigahdx::Config>::StHdxAssetId::get();

		// Simulate outstanding stHDX supply — the pool must not be swapped while
		// aTokens are in circulation.
		orml_tokens::TotalIssuance::<Runtime>::insert(sthdx, 1_000 * UNITS);
		assert_eq!(GigaHdx::total_gigahdx_supply(), 1_000 * UNITS);

		assert_noop!(
			GigaHdx::set_pool_contract(RuntimeOrigin::root(), pool_addr_a()),
			pallet_gigahdx::Error::<Runtime>::OutstandingStake
		);
	});
}

#[test]
fn set_pool_contract_should_fail_when_origin_not_authority() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			GigaHdx::set_pool_contract(RuntimeOrigin::signed(ALICE.into()), pool_addr_a()),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

// ------------------------- pallet-dispenser -------------------------

fn faucet_addr() -> EvmAddress {
	hex!["00000000000000000000000000000000000000ff"].into()
}

fn configure_dispenser() {
	assert_ok!(EthDispenser::set_config(
		RuntimeOrigin::root(),
		faucet_addr(),
		1_000,      // min_faucet_threshold
		10,         // min_request
		1_000_000,  // max_dispense
		5 * UNITS,  // dispenser_fee
		10_000_000, // faucet_balance_wei
	));
}

fn dummy_tx() -> pallet_dispenser::EvmTransactionParams {
	pallet_dispenser::EvmTransactionParams {
		value: 0,
		gas_limit: 21_000,
		max_fee_per_gas: 1_000_000_000,
		max_priority_fee_per_gas: 1_000_000_000,
		nonce: 0,
		chain_id: 1,
	}
}

#[test]
fn set_config_should_store_config_when_update_origin() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert!(EthDispenser::dispenser_config().is_none());

		configure_dispenser();

		let cfg = EthDispenser::dispenser_config().expect("dispenser must be configured");
		assert_eq!(cfg.faucet_address, faucet_addr());
		assert_eq!(cfg.min_faucet_threshold, 1_000);
		assert_eq!(cfg.min_request, 10);
		assert_eq!(cfg.max_dispense, 1_000_000);
		assert_eq!(cfg.dispenser_fee, 5 * UNITS);
		assert_eq!(cfg.faucet_balance_wei, 10_000_000);
		// First configuration starts unpaused.
		assert!(!cfg.paused);
	});
}

#[test]
fn set_config_should_fail_when_origin_not_update_origin() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			EthDispenser::set_config(
				RuntimeOrigin::signed(ALICE.into()),
				faucet_addr(),
				1_000,
				10,
				1_000_000,
				5 * UNITS,
				10_000_000,
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_config_should_fail_when_faucet_address_is_zero() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			EthDispenser::set_config(
				RuntimeOrigin::root(),
				EvmAddress::zero(),
				1_000,
				10,
				1_000_000,
				5 * UNITS,
				10_000_000,
			),
			pallet_dispenser::Error::<Runtime>::InvalidAddress
		);
	});
}

#[test]
fn set_config_should_fail_when_max_dispense_is_zero() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			EthDispenser::set_config(
				RuntimeOrigin::root(),
				faucet_addr(),
				1_000,
				0,
				0, // max_dispense == 0
				5 * UNITS,
				10_000_000,
			),
			pallet_dispenser::Error::<Runtime>::InvalidConfig
		);
	});
}

#[test]
fn pause_should_set_paused_flag_when_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_dispenser();
		assert!(!EthDispenser::dispenser_config().unwrap().paused);

		assert_ok!(EthDispenser::pause(RuntimeOrigin::root()));

		assert!(EthDispenser::dispenser_config().unwrap().paused);
	});
}

#[test]
fn pause_should_fail_when_not_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			EthDispenser::pause(RuntimeOrigin::root()),
			pallet_dispenser::Error::<Runtime>::NotConfigured
		);
	});
}

#[test]
fn pause_should_fail_when_origin_not_update_origin() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Origin is checked before the configured-state check.
		assert_noop!(
			EthDispenser::pause(RuntimeOrigin::signed(ALICE.into())),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn unpause_should_clear_paused_flag_when_paused() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_dispenser();
		assert_ok!(EthDispenser::pause(RuntimeOrigin::root()));
		assert!(EthDispenser::dispenser_config().unwrap().paused);

		assert_ok!(EthDispenser::unpause(RuntimeOrigin::root()));

		assert!(!EthDispenser::dispenser_config().unwrap().paused);
	});
}

#[test]
fn unpause_should_fail_when_not_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_noop!(
			EthDispenser::unpause(RuntimeOrigin::root()),
			pallet_dispenser::Error::<Runtime>::NotConfigured
		);
	});
}

#[test]
fn request_fund_should_fail_when_not_configured() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let to: EvmAddress = hex!["0000000000000000000000000000000000000001"].into();

		assert_noop!(
			EthDispenser::request_fund(RuntimeOrigin::signed(ALICE.into()), to, 100, [0u8; 32], dummy_tx()),
			pallet_dispenser::Error::<Runtime>::NotConfigured
		);
	});
}

#[test]
fn request_fund_should_fail_when_paused() {
	TestNet::reset();
	Hydra::execute_with(|| {
		configure_dispenser();
		assert_ok!(EthDispenser::pause(RuntimeOrigin::root()));

		let to: EvmAddress = hex!["0000000000000000000000000000000000000001"].into();

		// Paused is enforced before any parameter / request-id validation.
		assert_noop!(
			EthDispenser::request_fund(RuntimeOrigin::signed(ALICE.into()), to, 100, [0u8; 32], dummy_tx()),
			pallet_dispenser::Error::<Runtime>::Paused
		);
	});
}
