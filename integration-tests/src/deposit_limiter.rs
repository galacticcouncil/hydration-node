use crate::assert_reserved_balance;
use crate::polkadot_test_net::*;
use frame_support::pallet_prelude::Pays;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::Router;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::{AssetRegistry, CircuitBreaker, Currencies, Omnipool};
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use primitives::constants::time::DAYS;
use primitives::{AssetId, Balance};
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;
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
		go_to_block(5);
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
		go_to_block(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		go_to_block(DAYS + 3);
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
		go_to_block(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 3);

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
		go_to_block(2);

		crate::circuit_breaker::init_omnipool();
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		go_to_block(3);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 5 * UNITS));

		//Act
		go_to_block(4);
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 5 * UNITS));

		//Act
		go_to_block(5);
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
fn release_deposit_should_fail_when_in_lockdown() {
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
			CircuitBreaker::release_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::AssetInLockdown,
		);
	});
}

#[test]
fn release_deposit_should_payable_when_fails() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		let err = CircuitBreaker::release_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI)
			.expect_err("Expected the call to fail");
		assert_eq!(err.post_info.pays_fee, frame_support::dispatch::Pays::Yes);
	});
}

#[test]
fn release_deposit_should_work_when_in_the_last_block_of_lockdown() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 4);

		//Act
		assert_ok!(CircuitBreaker::release_deposit(
			RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI
		));
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
	});
}

#[test]
fn release_deposit_should_release_asset_when_lockdown_expires() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		//Act
		assert_ok!(
			CircuitBreaker::release_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI),
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
fn release_deposit_should_not_work_when_lockedown_triggered_2nd_time() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 2 * UNITS);

		//Act and assert
		assert_noop!(
			CircuitBreaker::release_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::AssetInLockdown
		);

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS * 2);
	});
}

#[test]
fn release_deposit_should_work_when_asset_unclocked() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), UNITS)); //It doesnt trigger circuit breaker, just puts state to unlocked
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_ok!(CircuitBreaker::release_deposit(
			RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI,
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
fn release_deposit_should_work_when_called_by_authority() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), UNITS)); //It doesnt trigger circuit breaker, just puts state to unlocked
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		let authority_origin = hydradx_runtime::OriginCaller::Origins(Origin::OmnipoolAdmin);

		assert_ok!(CircuitBreaker::release_deposit(
			authority_origin.into(),
			ALICE.into(),
			DAI,
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
fn release_deposit_should_work_when_accumulated_through_multiple_periods() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + 2 * UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 3 * UNITS);

		go_to_block(2 * DAYS + 6);

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + 3 * UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, 6 * UNITS);

		go_to_block(3 * DAYS + 7);

		//Act
		assert_ok!(CircuitBreaker::release_deposit(
			RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI,
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
fn release_deposit_should_fail_when_no_reserved_asset_for_user() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		go_to_block(DAYS + 5);

		//Act and assert
		assert_noop!(
			CircuitBreaker::release_deposit(RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

#[test]
fn release_deposit_should_work_when_other_user_claims_it() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_ok!(CircuitBreaker::release_deposit(
			RuntimeOrigin::signed(BOB.into()),
			ALICE.into(),
			DAI
		));

		//Assert
		assert_reserved_balance!(&ALICE.into(), DAI, 0);
		assert_eq!(
			Currencies::free_balance(DAI, &ALICE.into()),
			deposit_limit + ALICE_INITIAL_DAI_BALANCE + UNITS
		);
	});
}

#[test]
fn release_deposit_should_fail_when_called_2nd_time() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();
		go_to_block(4);

		assert_eq!(Currencies::free_balance(DAI, &ALICE.into()), ALICE_INITIAL_DAI_BALANCE);
		let deposit_limit = 100_000_000_000_000_000;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), deposit_limit + UNITS));
		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		go_to_block(DAYS + 5);

		assert_reserved_balance!(&ALICE.into(), DAI, UNITS);

		//Act
		assert_ok!(CircuitBreaker::release_deposit(
			RuntimeOrigin::signed(BOB.into()),
			ALICE.into(),
			DAI
		));

		assert_noop!(
			CircuitBreaker::release_deposit(RuntimeOrigin::signed(BOB.into()), ALICE.into(), DAI),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::InvalidAmount
		);
	});
}

use frame_support::pallet_prelude::Weight;
use hydradx_runtime::origins::Origin;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use polkadot_xcm::opaque::lts::WeightLimit;
use polkadot_xcm::v5::{Junction, Location};
use primitives::constants::currency::UNITS;

#[test]
fn hydra_should_block_asset_from_other_chain_when_over_limit() {
	// Arrange
	TestNet::reset();
	let deposit_limit = 10000 * UNITS;
	let amount_over_limit = 100 * UNITS;

	Hydra::execute_with(|| {
		assert_ok!(AssetRegistry::set_location(
			ACA,
			hydradx_runtime::AssetLocation(Location {
				parents: 1,
				interior: [Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0)].into()
			})
		));

		update_deposit_limit(ACA, deposit_limit).unwrap();
		assert_ok!(update_ed(ACA, 1_000));

		assert_eq!(Currencies::free_balance(ACA, &BOB.into()), 0);
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
			RuntimeOrigin::signed(ALICE.into()),
			0,
			deposit_limit + amount_over_limit,
			Box::new(
				Location {
					parents: 1,
					interior: [
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					]
					.into()
				}
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
	});

	Hydra::execute_with(|| {
		//The fee to-be-sent to the treausury was blocked and reserved too as we reached limit
		let fee = 77048488154;
		assert_reserved_balance!(&Treasury::account_id(), ACA, fee);

		// Bob receives the amount equal to deposit limit, the rest is reserved
		assert_eq!(Currencies::free_balance(ACA, &BOB.into()), deposit_limit);
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
		assert_reserved_balance!(&Router::router_account(), HDX, 0);
		assert_reserved_balance!(&Router::router_account(), DAI, 0);
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		assert_eq!(init_balance - new_balance, sell_amount)
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

		update_deposit_limit(LRNA, UNITS).unwrap();
		assert_ok!(Currencies::deposit(LRNA, &Omnipool::protocol_account(), 100 * UNITS));

		go_to_block(10);

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

		update_deposit_limit(LRNA, UNITS).unwrap();
		assert_ok!(Currencies::deposit(LRNA, &Omnipool::protocol_account(), 100 * UNITS));

		let init_block = 10u32;
		go_to_block(init_block);

		let mut positions = vec![];
		let amount = 800000000 * UNITS;

		for i in 0..250u32 {
			let position_id = Omnipool::next_position_id();

			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				amount
			));
			positions.push(position_id);
			go_to_block(init_block + (i + 1u32));
		}

		for (i, &position_id) in positions.iter().enumerate().take(93) {
			assert_ok!(Omnipool::remove_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				position_id,
				amount
			));

			go_to_block(init_block + (i as u32) + 250);
		}

		assert_noop!(
			Omnipool::remove_liquidity(RuntimeOrigin::signed(ALICE.into()), positions[93], amount),
			orml_tokens::Error::<hydradx_runtime::Runtime>::BalanceTooLow
		);
	});
}

fn set_xcm_location(asset_id: AssetId, general_index: u128) -> Location {
	let location = Location {
		parents: 1,
		interior: [
			Junction::Parachain(ACALA_PARA_ID),
			Junction::GeneralIndex(general_index),
		]
		.into(),
	};
	assert_ok!(AssetRegistry::set_location(
		asset_id,
		hydradx_runtime::AssetLocation(location.clone())
	));
	location
}

fn xcm_deposit(
	asset_location: Location,
	amount: Balance,
	beneficiary: [u8; 32],
) -> Result<(), polkadot_xcm::v5::Error> {
	use xcm_executor::traits::TransactAsset;

	let asset = polkadot_xcm::v5::Asset {
		id: polkadot_xcm::v5::AssetId(asset_location),
		fun: polkadot_xcm::v5::Fungibility::Fungible(amount),
	};
	let beneficiary = Location {
		parents: 0,
		interior: [Junction::AccountId32 {
			id: beneficiary,
			network: None,
		}]
		.into(),
	};

	<hydradx_runtime::LocalAssetTransactor as TransactAsset>::deposit_asset(&asset, &beneficiary, None)
}

#[test]
fn xcm_deposit_to_router_should_mint_once_when_over_deposit_limit() {
	Hydra::execute_with(|| {
		//Arrange
		let asset_location = set_xcm_location(ACA, 0);
		let deposit_limit = 10_000 * UNITS;
		update_deposit_limit(ACA, deposit_limit).unwrap();

		let amount = deposit_limit + 100 * UNITS;
		let router = Router::router_account();
		let alternative = hydradx_runtime::Alternative::get();
		let issuance_before = Currencies::total_issuance(ACA);

		//Act
		assert_ok!(xcm_deposit(asset_location, amount, router.into()));

		//Assert
		assert_eq!(Currencies::total_issuance(ACA), issuance_before + amount);

		// The deposit was accounted for on the beneficiary, so the fallback account stays untouched.
		assert_eq!(Currencies::free_balance(ACA, &alternative), 0);
		assert_reserved_balance!(&alternative, ACA, 0);
	});
}

#[test]
fn xcm_deposit_to_router_should_reserve_excess_when_over_deposit_limit() {
	Hydra::execute_with(|| {
		//Arrange
		let asset_location = set_xcm_location(ACA, 0);
		let deposit_limit = 10_000 * UNITS;
		update_deposit_limit(ACA, deposit_limit).unwrap();

		let amount = deposit_limit + 100 * UNITS;
		let router = Router::router_account();

		//Act
		assert_ok!(xcm_deposit(asset_location, amount, router.clone().into()));

		//Assert
		assert_eq!(Currencies::free_balance(ACA, &router), deposit_limit);
		assert_reserved_balance!(&router, ACA, amount - deposit_limit);
	});
}

#[test]
fn xcm_deposit_to_non_whitelisted_account_should_mint_once_when_over_deposit_limit() {
	Hydra::execute_with(|| {
		//Arrange
		let asset_location = set_xcm_location(ACA, 0);
		let deposit_limit = 10_000 * UNITS;
		update_deposit_limit(ACA, deposit_limit).unwrap();

		let amount = deposit_limit + 100 * UNITS;
		let beneficiary: hydradx_runtime::AccountId = ALICE.into();
		let alternative = hydradx_runtime::Alternative::get();

		let issuance_before = Currencies::total_issuance(ACA);
		let free_before = Currencies::free_balance(ACA, &beneficiary);

		//Act
		assert_ok!(xcm_deposit(asset_location, amount, ALICE));

		//Assert
		assert_eq!(Currencies::total_issuance(ACA), issuance_before + amount);
		assert_eq!(Currencies::free_balance(ACA, &beneficiary), free_before + deposit_limit);
		assert_reserved_balance!(&beneficiary, ACA, amount - deposit_limit);
		assert_eq!(Currencies::free_balance(ACA, &alternative), 0);
	});
}

#[test]
fn route_execution_should_not_release_reserved_deposit_of_router() {
	Hydra::execute_with(|| {
		//Arrange
		crate::circuit_breaker::init_omnipool();

		let asset_location = set_xcm_location(DAI, DAI as u128);
		let deposit_limit = 10_000 * UNITS;
		update_deposit_limit(DAI, deposit_limit).unwrap();

		let trader: hydradx_runtime::AccountId = BOB.into();
		assert_ok!(Currencies::deposit(HDX, &trader, 1_000_000 * UNITS));

		let amount = deposit_limit + 100 * UNITS;
		let router = Router::router_account();
		assert_ok!(xcm_deposit(asset_location, amount, router.clone().into()));

		let reserved = amount - deposit_limit;
		assert_reserved_balance!(&router, DAI, reserved);
		let issuance_before = Currencies::total_issuance(DAI);

		//Act
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(trader),
			HDX,
			DAI,
			2_000 * UNITS,
			0,
			vec![].try_into().unwrap(),
		));

		//Assert: route execution moves only free balance, the reserved deposit stays put.
		assert_reserved_balance!(&router, DAI, reserved);
		assert_eq!(Currencies::total_issuance(DAI), issuance_before);
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
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			Some(ACA),
			Some(b"ACAL".to_vec().try_into().unwrap()),
			AssetKind::Token,
			2_000_000,
			None,
			None,
			None,
			None,
		))
	})
	.map_err(|_| ())
}
