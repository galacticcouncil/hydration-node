use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::BoundedVec;
use hydradx_adapters::stableswap_peg_oracle::PegOracle;
use hydradx_runtime::*;
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_stableswap::traits::{Peg, PegOracle as PegOracleTrait, Source};
use pallet_stableswap::types::BoundedPegSources;
use pallet_stableswap::types::PegSource;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;
use std::sync::Arc;
use test_utils::assert_eq_approx;

const DOT: AssetId = 2221;
const VDOT: AssetId = 2222;
const ADOT: AssetId = 2223;
const GIGADOT: AssetId = 69;

const DOT_DECIMALS: u8 = 10;
const VDOT_DECIMALS: u8 = 10;
const ADOT_DECIMALS: u8 = 10;
const GIGADOT_DECIMALS: u8 = 18;

const DOT_VDOT_PRICE: (Balance, Balance) = (85473939039997170, 57767685517430457);

#[test]
fn gigadot_pool_should_work() {
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

	HydrationTestDriver::default()
		.register_asset(DOT, b"myDOT", DOT_DECIMALS, Some(dot_location))
		.register_asset(VDOT, b"myvDOT", VDOT_DECIMALS, Some(vdot_location))
		.register_asset(ADOT, b"myaDOT", ADOT_DECIMALS, None)
		.register_asset(GIGADOT, b"myGIGADOT", GIGADOT_DECIMALS, None)
		.update_bifrost_oracle(dot_boxed, vdot_boxed, DOT_VDOT_PRICE)
		.new_block()
		.endow_account(ALICE.into(), DOT, 1_000_000 * 10u128.pow(DOT_DECIMALS as u32))
		.endow_account(ALICE.into(), VDOT, 1_000_000 * 10u128.pow(VDOT_DECIMALS as u32))
		.endow_account(ALICE.into(), ADOT, 1_000_000 * 10u128.pow(ADOT_DECIMALS as u32))
		.execute(|| {
			let assets = vec![VDOT, ADOT];
			let pegs = vec![
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, DOT)), // vDOT peg
				PegSource::Value((1, 1)),                                          // aDOT peg
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

			let initial_liquidity = 1_000 * 10u128.pow(DOT_DECIMALS as u32);
			let liquidity = vec![
				AssetAmount::new(VDOT, initial_liquidity),
				AssetAmount::new(ADOT, initial_liquidity),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			let initial_alice_vdot_balance = Tokens::free_balance(VDOT, &ALICE.into());
			let initial_alice_adot_balance = Tokens::free_balance(ADOT, &ALICE.into());

			// Sell 1 vdot for adot
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			// Verify balances of ALICE
			let final_alice_vdot_balance = Tokens::free_balance(VDOT, &ALICE.into());
			let final_alice_adot_balance = Tokens::free_balance(ADOT, &ALICE.into());

			let adot_received = final_alice_adot_balance - initial_alice_adot_balance;
			// use vdot adot price to calculate expected adot received
			let expected_adot_received = (10u128.pow(VDOT_DECIMALS as u32)) * DOT_VDOT_PRICE.0 / DOT_VDOT_PRICE.1;
			// ensure that it is approximately equal
			assert_eq_approx!(
				adot_received,
				expected_adot_received,
				100_000_000_000_000_000,
				"Expected adot received is not equal to actual adot received"
			);

			assert!(final_alice_vdot_balance < initial_alice_vdot_balance);
			assert!(final_alice_adot_balance > initial_alice_adot_balance);
		});
}

#[test]
fn peg_oracle_adapter_should_work_when_getting_price_from_dia() {
	TestNet::reset();
	hydra_live_ext("evm-snapshot/router").execute_with(|| {
		const NOW: Moment = (1744142439 + primitives::constants::time::SECS_PER_BLOCK) * 1000; // unix time in milliseconds
		hydradx_runtime::Timestamp::set_timestamp(NOW);
		hydradx_run_to_block(10);

		let peg = PegOracle::<Runtime, evm::Executor<Runtime>>::get(Source::ChainlinkOracle(
			hex!["17711BE5D63B2Fe8A2C379725DE720773158b954"].into(), //NOTE: dia USDC oracle address
		))
		.expect("failed to retrieve peg from contract");

		println!("{:?}, {:?}", peg.val, peg.updated_at);

		let expected_peg = Peg {
			val: (99988686_u128, 100_000_000_u128),
			updated_at: 9_u32,
		};
		assert_eq!(peg, expected_peg)
	});
}
