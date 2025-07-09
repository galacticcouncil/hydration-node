use crate::assert_reserved_balance;
use crate::polkadot_test_net::*;
use crate::stableswap::GIGADOT;
use frame_support::pallet_prelude::Pays;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::Router;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::Stableswap;
use hydradx_runtime::{
	AssetRegistry, Balances, CircuitBreaker, Currencies, Omnipool, OmnipoolCollectionId, Tokens, Uniques,
};
use primitives::constants::time::DAYS;

use pallet_stableswap::types::BoundedPegSources;

use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_dca::pallet;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_stableswap::types::PegSource;
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::{AccountId, AssetId, Balance};
use sp_runtime::traits::Zero;
use sp_runtime::BoundedVec;
use sp_runtime::Permill;
use sp_runtime::{FixedU128, TransactionOutcome};
use std::sync::Arc;
use test_utils::{assert_balance, assert_eq_approx};
use xcm_emulator::TestExt;

#[test]
fn circuit_breaker_triggered_when_reaches_limit_in_first_run() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		//Act
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);
	});
}

#[test]
fn circuit_breaker_triggered_when_reaches_limit_in_period() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit / 2));

		//Act
		set_relaychain_block_number(5);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, deposit_limit / 2);
	});
}

#[test]
fn circuit_breaker_allows_deposit_when_period_is_over() {
	Hydra::execute_with(|| {
		//Arrange
		set_relaychain_block_number(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		set_relaychain_block_number(DAYS + 3);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit * 2 + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);
	});
}

#[test]
fn circuit_breaker_triggers_when_period_is_over_but_first_deposit_reaches_limit() {
	Hydra::execute_with(|| {
		//Arrange
		set_relaychain_block_number(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 3);

		//Act
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + 5 * UNITS));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit * 2 + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, 6 * UNITS);
	});
}

#[test]
fn circuit_breaker_triggers_when_adding_more_and_more_above_limit() {
	Hydra::execute_with(|| {
		//Arrange
		set_relaychain_block_number(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		set_relaychain_block_number(3);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 5 * UNITS));

		//Act
		set_relaychain_block_number(4);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 5 * UNITS));

		//Act
		set_relaychain_block_number(5);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 5 * UNITS));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, 16 * UNITS);
	});
}

#[test]
fn circuit_breaker_should_not_trigger_for_asset_without_limit_set() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		let amount = 100_000_000_000_000_000;

		//Act
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), amount * 1000));

		//Assert
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			amount * 1000 + ALICE_INITIAL_DAI_BALANCE
		);

		assert_reserved_balance!(&ALICE.into(), DAI, 0);
	});
}

#[test]
fn save_deposit_should_fail_when_in_lockdown() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::AssetInLockdown,
		);
	});
}

#[test]
fn save_deposit_should_payable_when_fails() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		let err = CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS)
			.expect_err("Expected the call to fail");
		assert_eq!(err.post_info.pays_fee, frame_support::dispatch::Pays::Yes);
	});
}

#[test]
fn save_deposit_should_fail_when_in_the_last_block_of_lockdown() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 4);

		//Act
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::AssetInLockdown
		);
	});
}

#[test]
fn save_deposit_should_release_asset_when_lockdown_expires() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		//Act
		assert_ok!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS),
			Pays::No.into()
		);

		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE + UNITS
		);
	});
}

#[test]
fn save_deposit_should_not_work_when_lockedown_triggered_2nd_time() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 2 * UNITS);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::AssetInLockdown
		);

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS * 2);
	});
}

#[test]
fn save_deposit_should_work_when_asset_unclocked() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), UNITS)); //It doesnt trigger circuit breaker, just puts state to unlocked
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_ok!(CircuitBreaker::save_deposit(
			RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI,
			UNITS
		));

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE + 2 * UNITS
		);
	});
}

#[test]
fn save_deposit_should_work_when_accumulated_through_multiple_periods() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + 2 * UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 3 * UNITS);

		set_relaychain_block_number(2 * DAYS + 6);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + 3 * UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 6 * UNITS);

		set_relaychain_block_number(3 * DAYS + 7);

		//Act
		assert_ok!(CircuitBreaker::save_deposit(
			RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI,
			6 * UNITS
		));

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			3 * deposit_limit + ALICE_INITIAL_DAI_BALANCE + 6 * UNITS
		);
	});
}

#[test]
fn save_deposit_should_fail_when_amount_is_more_than_reserved() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS * 99),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn save_deposit_should_fail_when_amount_is_less_than_reserved() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS / 4),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn save_deposit_should_fail_when_amount_is_zero() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		set_relaychain_block_number(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, 0),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn save_deposit_should_fail_when_nothing_is_reserved() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		set_relaychain_block_number(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, 17 * UNITS),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn save_deposit_should_fail_when_no_reserved_asset_for_user() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		set_relaychain_block_number(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::save_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI, UNITS),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn save_deposit_should_work_when_other_user_claims_it() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		set_relaychain_block_number(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		set_relaychain_block_number(DAYS + 5);

		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_ok!(CircuitBreaker::save_deposit(
			RuntimeOrigin::signed(BOB.into()),
			ALICE.into(),
			DAI,
			UNITS
		));

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE + UNITS
		);
	});
}

use frame_support::pallet_prelude::Weight;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use pallet_broadcast::types::Filler::Omnipool as OtherOmnipool;
use polkadot_xcm::opaque::lts::WeightLimit;
use polkadot_xcm::opaque::v3::{
	Junction,
	Junctions::{X1, X2},
	MultiLocation, NetworkId,
};
use primitives::constants::currency::UNITS;

#[test]
fn hydra_should_block_asset_from_other_hain_when_over_limit() {
	// Arrange
	TestNet::reset();
	let deposit_limit = 10000 * UNITS;
	let amount_over_limit = 100 * UNITS;

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
		));

		update_deposit_limit(ACA, deposit_limit).unwrap();
		assert_ok!(update_ed(ACA, 1_000));

		assert_eq!(hydradx_runtime::Currencies::free_balance(ACA, &BOB.into()), 0);
	});

	Acala::execute_with(|| {
		// Act
		assert_ok!(register_aca());

		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			0,
			2 * deposit_limit as i128,
		));

		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			ACA,
			2 * deposit_limit as i128,
		));

		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			deposit_limit + amount_over_limit,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
	});

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());

		//The fee to-be-sent to the treausury was blocked and reserved too as we reached limit
		let fee = 77827795107;
		assert_reserved_balance!(&hydradx_runtime::Treasury::account_id(), ACA, 77827795107);

		// Bob receives the amount equal to deposit limit, the rest is reserved
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(ACA, &BOB.into()),
			deposit_limit
		);
		assert_reserved_balance!(&BOB.into(), ACA, amount_over_limit - fee);
	});
}

#[test]
fn route_execution_should_not_trigger_circuit_breaker() {
	Hydra::execute_with(|| {
		// Arrange
		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100 * UNITS;

		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), deposit_limit * 50));

		update_deposit_limit(HDX, 100 * UNITS).unwrap();
		update_deposit_limit(DAI, 100 * UNITS).unwrap();

		let init_balance = Currencies::free_balance(HDX, &ALICE.into());

		// Act
		let sell_amount = 20 * deposit_limit;
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DAI,
			sell_amount,
			u128::MIN,
			vec![].try_into().unwrap()
		));

		// Assert
		assert_reserved_balance!(&ALICE.into(), HDX, 0);
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_reserved_balance!(&Router::router_account().into(), HDX, 0);
		assert_reserved_balance!(&Router::router_account().into(), DAI, 0);
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		assert_eq!(init_balance - new_balance, sell_amount)
	});
}

//TODO: verify it
#[test]
fn circuit_should_not_be_triggered_for_omnipool() {
	Hydra::execute_with(|| {
		// Arrange
		crate::circuit_breaker::init_omnipool();
		let amount = 100_000_000_000_000_000;
		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), amount * 100));

		update_deposit_limit(LRNA, 100 * UNITS).unwrap();
		// Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DAI,
			amount,
			u128::MIN,
		));

		assert_reserved_balance!(&Omnipool::protocol_account(), LRNA, 0);
	});
}

#[test]
fn add_liquidity_should_work_when_circuit_breaker_triggers_for_lrna() {
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool();
		assert_ok!(Omnipool::set_asset_weight_cap(
			RuntimeOrigin::root(),
			HDX,
			Permill::from_percent(33),
		));

		assert_ok!(Currencies::deposit(LRNA, &ALICE.into(), 100 * UNITS));

		update_deposit_limit(LRNA, 1 * UNITS).unwrap();
		assert_ok!(Currencies::deposit(LRNA, &Omnipool::protocol_account(), 100 * UNITS));

		let hdx_balance = Currencies::free_balance(HDX, &ALICE.into());

		set_relaychain_block_number(10);

		// Act and assert
		assert_ok!(Omnipool::add_liquidity(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			1000000000
		));
	});
}

#[test]
fn remove_liquidity_cannot_burn_more_lrna_when_asset_locked_down() {
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool();
		assert_ok!(Omnipool::set_asset_weight_cap(
			RuntimeOrigin::root(),
			HDX,
			Permill::from_percent(33),
		));

		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), 1000000 * UNITS));
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 3402823669209384634633746074317)); //Mint infinite amount of DAI (because of a hack/exploit or so)
		assert_ok!(Currencies::deposit(LRNA, &ALICE.into(), 100 * UNITS));

		update_deposit_limit(LRNA, 1 * UNITS).unwrap();
		assert_ok!(Currencies::deposit(LRNA, &Omnipool::protocol_account(), 100 * UNITS));

		let hdx_balance = Currencies::free_balance(HDX, &ALICE.into());

		let init_block = 10u32;
		set_relaychain_block_number(init_block);

		let mut positions = vec![];
		let amount = 2000000000 * UNITS;

		for i in 0..100u32 {
			let position_id = hydradx_runtime::Omnipool::next_position_id();

			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				amount
			));
			positions.push(position_id);
			set_relaychain_block_number(init_block + (i + 1u32));
		}

		for i in 0..=36usize {
			let position_id = positions[i];

			assert_ok!(Omnipool::remove_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				position_id,
				amount
			));

			set_relaychain_block_number(init_block + (i as u32) + 100);
		}

		assert_noop!(
			Omnipool::remove_liquidity(RuntimeOrigin::signed(ALICE.into()), positions[37], amount),
			orml_tokens::Error::<hydradx_runtime::Runtime>::BalanceTooLow
		);
	});
}

pub fn update_deposit_limit(asset_id: AssetId, limit: Balance) -> Result<(), ()> {
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::update(
			RawOrigin::Root.into(),
			asset_id,
			None,
			None,
			None,
			Some(limit),
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}

pub fn update_ed(asset_id: AssetId, ed: Balance) -> Result<(), ()> {
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::update(
			RawOrigin::Root.into(),
			asset_id,
			None,
			None,
			Some(ed),
			None,
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}

fn register_aca() -> Result<u32, ()> {
	with_transaction(|| {
		TransactionOutcome::Commit(
			(AssetRegistry::register_sufficient_asset(
				Some(ACA),
				Some(b"ACAL".to_vec().try_into().unwrap()),
				AssetKind::Token,
				2_000_000,
				None,
				None,
				None,
				None,
			)),
		)
	})
	.map_err(|_| ())
}
