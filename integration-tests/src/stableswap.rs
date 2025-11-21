use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::BoundedVec;
use hydradx_adapters::stableswap_peg_oracle::PegOracle;
use hydradx_runtime::*;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::RawEntry;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_stableswap::traits::PegRawOracle;
use pallet_stableswap::types::BoundedPegSources;
use pallet_stableswap::types::BoundedPegs;
use pallet_stableswap::types::PegSource;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;
use primitives::{constants::time::SECS_PER_BLOCK, BlockNumber};
use sp_runtime::{Perbill, Permill};
use std::sync::Arc;
use test_utils::assert_eq_approx;

pub const DOT: AssetId = 2221;
pub const VDOT: AssetId = 2222;
pub const ADOT: AssetId = 2223;
pub const GIGADOT: AssetId = 69;

pub const DOT_DECIMALS: u8 = 10;
pub const VDOT_DECIMALS: u8 = 10;
pub const ADOT_DECIMALS: u8 = 10;
pub const GIGADOT_DECIMALS: u8 = 18;

pub const DOT_VDOT_PRICE: (Balance, Balance) = (85473939039997170, 57767685517430457);

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
				Perbill::from_percent(100),
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
				100_000_000,
				"Expected adot received is not equal to actual adot received"
			);

			assert!(final_alice_vdot_balance < initial_alice_vdot_balance);
			assert!(final_alice_adot_balance > initial_alice_adot_balance);
		});
}

#[test]
fn peg_oracle_adapter_should_work_when_getting_price_from_mm_oracle() {
	TestNet::reset();
	hydra_live_ext("evm-snapshot/router").execute_with(|| {
		let current_block: BlockNumber = 50_u32;
		let blocks_diff = 5;
		let now: Moment = (1744142439 + SECS_PER_BLOCK * blocks_diff) * 1000; // unix time in milliseconds
		hydradx_runtime::Timestamp::set_timestamp(now);
		hydradx_run_to_block(current_block);

		let peg = PegOracle::<Runtime, evm::Executor<Runtime>, EmaOracle>::get_raw_entry(
			Default::default(), //NOTE: MMOracle doesn't use this param, only contract's address
			PegSource::MMOracle(
				hex!["17711BE5D63B2Fe8A2C379725DE720773158b954"].into(), //NOTE: dia's USDC oracle
			),
		)
		.expect("failed to retrieve peg from contract");

		let expected_peg = RawEntry {
			price: (99988686_u128, 100_000_000_u128),
			volume: Default::default(),
			liquidity: Default::default(),
			updated_at: current_block - blocks_diff as u32,
			shares_issuance: Default::default(),
		};
		assert_eq!(peg, expected_peg)
	});
}

#[test]
fn peg_oracle_adapter_should_not_work_when_mm_oracle_price_was_updated_in_current_block() {
	TestNet::reset();
	hydra_live_ext("evm-snapshot/router").execute_with(|| {
		let current_block: BlockNumber = 50_u32;
		let blocks_diff = 0;
		let now: Moment = (1744142439 + SECS_PER_BLOCK * blocks_diff) * 1000; // unix time in milliseconds
		hydradx_runtime::Timestamp::set_timestamp(now);
		hydradx_run_to_block(current_block);

		let peg = PegOracle::<Runtime, evm::Executor<Runtime>, EmaOracle>::get_raw_entry(
			Default::default(), //NOTE: MMOracle doesn't use this param, only contract's address
			PegSource::MMOracle(
				hex!["17711BE5D63B2Fe8A2C379725DE720773158b954"].into(), //NOTE: dia's USDC oracle
			),
		)
		.expect("failed to retrieve peg from contract");

		let expected_peg = RawEntry {
			price: (99988686_u128, 100_000_000_u128),
			volume: Default::default(),
			liquidity: Default::default(),
			updated_at: current_block,
			shares_issuance: Default::default(),
		};
		assert_eq!(peg, expected_peg)
	});
}

mod circuit_breaker {
	use super::*;
	use crate::assert_reserved_balance;

	#[test]
	fn ciruit_breaker_is_triggered_when_deposit_limit_reached_for_sharetoken() {
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
					Perbill::from_percent(100),
				));

				let initial_liquidity = 1_000 * 10u128.pow(DOT_DECIMALS as u32);
				let liquidity = vec![
					AssetAmount::new(VDOT, initial_liquidity),
					AssetAmount::new(ADOT, initial_liquidity),
				];

				//Act
				crate::deposit_limiter::update_deposit_limit(GIGADOT, 2000000000000000000000).unwrap();
				assert_ok!(Stableswap::add_assets_liquidity(
					RuntimeOrigin::signed(ALICE.into()),
					GIGADOT,
					BoundedVec::truncate_from(liquidity),
					0,
				));

				//Assert
				assert_reserved_balance!(&ALICE.into(), GIGADOT, 479138260494833187243);
			});
	}
}

#[test]
fn pool_with_pegs_should_update_pegs_only_once_per_block() {
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
		//~1.479615087126985602
		.update_bifrost_oracle(dot_boxed.clone(), vdot_boxed.clone(), DOT_VDOT_PRICE)
		.new_block()
		.endow_account(ALICE.into(), DOT, 1_000_000 * 10u128.pow(DOT_DECIMALS as u32))
		.endow_account(ALICE.into(), VDOT, 1_000_000 * 10u128.pow(VDOT_DECIMALS as u32))
		.endow_account(ALICE.into(), ADOT, 1_000_000 * 10u128.pow(ADOT_DECIMALS as u32))
		.execute(|| {
			let precission = FixedU128::from_inner(1_000);
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
				Permill::from_parts(600), //0.06%
				BoundedPegSources::truncate_from(pegs),
				Perbill::from_parts(1_000_000), //0.1%
			));

			assert_eq!(
				Stableswap::pool_peg_info(GIGADOT)
					.expect("GIGADOT pool to exists")
					.updated_at,
				System::block_number()
			);

			let initial_liquidity = 1_000 * 10u128.pow(DOT_DECIMALS as u32);
			let liquidity = vec![
				AssetAmount::new(VDOT, initial_liquidity),
				AssetAmount::new(ADOT, initial_liquidity),
			];

			let pegs_0 = Stableswap::pool_peg_info(GIGADOT).expect("pegs should exists");

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			//pegs should not change, it's same block
			assert_eq!(Stableswap::pool_peg_info(GIGADOT).unwrap(), pegs_0);

			//initial liq. should update block fees
			let block_fee_0 = Stableswap::block_fee(GIGADOT);
			assert!(block_fee_0.is_some());

			// Sell 1 vdot for adot
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			//neither pegs nor block fees should change, it's same block
			assert_eq!(Stableswap::pool_peg_info(GIGADOT).unwrap(), pegs_0);
			assert_eq!(Stableswap::block_fee(GIGADOT), block_fee_0);

			//NOTE: I. set new oracle's price and move by 10 blocks
			//new price = 1.576357467046855425
			let dot_vdot_price: (Balance, Balance) = (189574745532334, 120261266556172);
			assert_ok!(EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(bifrost_account()),
				dot_boxed.clone(),
				vdot_boxed.clone(),
				dot_vdot_price
			));

			for _ in 0..10 {
				hydradx_run_to_next_block();
			}

			assert!(Stableswap::block_fee(GIGADOT).is_none());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			// 1.479615087126985602 + (1.479615087126985602 * 10 * 0.001) = 1.49441123799825545802
			// (1.49441123799825545802 / 1.479615087126985602) - 1 = 0.01 => 1[%] == 0.1[%]*10[blocks])
			let expected_pegs = FixedU128::from_float(1.49441123799825545802_f64);
			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				expected_pegs,
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 12);

			let block_fee_1 = Stableswap::block_fee(GIGADOT);
			assert_eq!(block_fee_1, Some(Permill::from_parts(1999)));

			//second trade in same block, pegs should not change
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));
			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				expected_pegs,
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 12);

			assert_eq!(Stableswap::block_fee(GIGADOT), block_fee_1);

			//NOTE: II. move 1 block and check change
			hydradx_run_to_next_block();

			assert!(Stableswap::block_fee(GIGADOT).is_none());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			// 1.49441123799825545802 + (1.49441123799825545802 * 1 * 0.001) = 1.49590564923625371347802
			// (1.49590564923625371347802 / 1.49441123799825545802) - 1 = 0.001 => 0.1[%] == 0.1[%] * 1[block])
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				FixedU128::from_float(1.49590564923625371347802_f64),
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 13);

			assert_eq!(Stableswap::block_fee(GIGADOT), Some(Permill::from_parts(1999)));

			//NOTE: III. run to 1 block before peg should reach oracle's price
			// ((oracle_price - current_price)/(current_prie * max_change))
			// (1.57635 - 1.495905649)/(1.495905649 * 0.001) = 53.776 => 53
			// pegs should be equal 54 blocks from now
			for _ in 0..53 {
				hydradx_run_to_next_block();
			}
			assert!(Stableswap::block_fee(GIGADOT).is_none());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			// 1.49590564923625371347802 + (1.49590564923625371347802 * 53 * 0.001) = 1.57518864864577516029235506
			// (1.57518864864577516029235506 /  1.49590564923625371347802) - 1 = 0.053 => 5.3[%] == 0.1[%]*53[blocks])
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				FixedU128::from_float(1.57518864864577516029235506_f64),
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 66);

			assert_eq!(Stableswap::block_fee(GIGADOT), Some(Permill::from_parts(1999)));

			//NOTE: run to block when stableswap's peg should reach oracle's price
			hydradx_run_to_next_block();
			assert!(Stableswap::block_fee(GIGADOT).is_none());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			// 1.57518864864577516029235506 + (1.57518864864577516029235506 * 1 * 0.001) = 1.57676383729442093545264741506
			// 1.57676383729442093545264741506 > 1.57635(oracle's price) => new peg == 1.576357467046855425
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				FixedU128::from_float(1.576357467046855425_f64),
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 67);

			assert_eq!(Stableswap::block_fee(GIGADOT), Some(Permill::from_parts(1484)));

			//NOTE: run multiple blocks, pegs value should not change as it already reached oracle's
			//price
			for _ in 0..20 {
				hydradx_run_to_next_block();
			}
			assert!(Stableswap::block_fee(GIGADOT).is_none());

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				GIGADOT,
				VDOT,
				ADOT,
				10u128.pow(VDOT_DECIMALS as u32),
				0,
			));

			//Asserts
			let peg_info_1 = Stableswap::pool_peg_info(GIGADOT).unwrap();
			assert_eq_approx!(
				FixedU128::from_rational(peg_info_1.current[0].0, peg_info_1.current[0].1),
				FixedU128::from_float(1.576357467046855425_f64),
				precission,
				"Updated pegs doesn't match expected value"
			);
			assert_eq!(peg_info_1.current[1], (1_u128, 1_u128));
			assert_eq!(peg_info_1.updated_at, 87);

			assert_eq!(Stableswap::block_fee(GIGADOT), Some(Permill::from_parts(600)));
		});
}
