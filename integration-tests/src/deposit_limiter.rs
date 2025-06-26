use crate::assert_reserved_balance;
use crate::polkadot_test_net::*;
use crate::stableswap::GIGADOT;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::Stableswap;
use hydradx_runtime::{
	AssetRegistry, Balances, CircuitBreaker, Currencies, Omnipool, OmnipoolCollectionId, Tokens, Uniques,
};
use pallet_stableswap::types::BoundedPegSources;

use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_stableswap::types::PegSource;
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::{AssetId, Balance};
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
		set_relaychain_block_number(103);
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

		set_relaychain_block_number(103);

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

#[ignore] //TODO: continue later once we have all done
#[test]
fn should_trigger_for_vdot_sent_from_other_chain() {
	let dot_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(1500),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);

	let vdot_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(1500),
			polkadot_xcm::v4::Junction::GeneralIndex(1),
		])),
	);

	let vdot_boxed = Box::new(vdot_location.clone().into_versioned());
	let dot_boxed = Box::new(dot_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::default()
		.register_asset(
			crate::stableswap::DOT,
			b"myDOT",
			crate::stableswap::DOT_DECIMALS,
			Some(dot_location),
		)
		.register_asset(
			crate::stableswap::VDOT,
			b"myvDOT",
			crate::stableswap::VDOT_DECIMALS,
			Some(vdot_location),
		)
		.register_asset(
			crate::stableswap::ADOT,
			b"myaDOT",
			crate::stableswap::ADOT_DECIMALS,
			None,
		)
		.register_asset(
			crate::stableswap::GIGADOT,
			b"myGIGADOT",
			crate::stableswap::GIGADOT_DECIMALS,
			None,
		)
		.update_bifrost_oracle(dot_boxed, vdot_boxed, crate::stableswap::DOT_VDOT_PRICE)
		.new_block()
		.endow_account(
			ALICE.into(),
			crate::stableswap::DOT,
			1_000_000 * 10u128.pow(crate::stableswap::DOT_DECIMALS as u32),
		)
		.endow_account(
			ALICE.into(),
			crate::stableswap::VDOT,
			1_000_000 * 10u128.pow(crate::stableswap::VDOT_DECIMALS as u32),
		)
		.endow_account(
			ALICE.into(),
			crate::stableswap::ADOT,
			1_000_000 * 10u128.pow(crate::stableswap::ADOT_DECIMALS as u32),
		)
		.execute(|| {
			let assets = vec![crate::stableswap::VDOT, crate::stableswap::ADOT];
			let pegs = vec![
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, crate::stableswap::DOT)), // vDOT peg
				PegSource::Value((1, 1)),                                                             // aDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				GIGADOT,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let initial_liquidity = 1_000 * 10u128.pow(crate::stableswap::DOT_DECIMALS as u32);
			let liquidity = vec![
				AssetAmount::new(crate::stableswap::VDOT, initial_liquidity),
				AssetAmount::new(crate::stableswap::ADOT, initial_liquidity),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				BoundedVec::truncate_from(liquidity),
				0,
			));
		});
}

#[test]
fn circuit_should_be_triggered_for_erc20() {}

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
