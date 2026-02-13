#![cfg(test)]

use crate::dynamic_fees::{init_omnipool, set_balance};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydra_dx_math::omnipool::types::slip_fee::HubAssetBlockState;
use hydra_dx_math::omnipool::types::BalanceUpdate::{Decrease, Increase};
use hydradx_runtime::{Omnipool, RuntimeOrigin, System};
use hydradx_traits::router::PoolType;
use pallet_route_executor::TradeExecution;
use xcm_emulator::TestExt;

const ONE: u128 = 1_000_000_000_000;
const ONE_DOT: u128 = 10_000_000_000;

#[test]
fn slip_fee_for_single_sell_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		System::reset_events();

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			10 * ONE_DOT,
			0,
		));

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			5 * ONE_DOT,
			10_000 * ONE,
		));

		let amount_in = 20_000_000_000;
		let amount_out = 425_104_485_150_786;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 513_790_957_735;
		let asset_fee_amount = 1_065_424_774_814;
		let protocol_fee_amount = 373_378_869;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Decrease(512_883_062_661),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(512_509_683_792),
			}
		);
	});
}

#[test]
fn slip_fee_for_single_sell_lrna_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		System::reset_events();

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			LRNA,
			DOT,
			2 * UNITS,
			0,
		));

		let amount_in = 2_000_000_000_000;
		let amount_out = 77_639_751_552;
		let hub_amount_in = 0;
		let hub_amount_out = 0;
		let asset_fee_amount = 194_585_844;
		let protocol_fee_amount = 0;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: LRNA,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Increase(2_000_000_000_000),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn slip_fee_for_two_sells_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE,
			0,
		));

		System::reset_events();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE,
			0,
		));

		let amount_in = 2_000_000_000_000;
		let amount_out = 32_012_738_643_449_134;
		let hub_amount_in = 47_969_044_874_199;
		let hub_amount_out = 45_870_369_897_229;
		let asset_fee_amount = 80_232_427_677_818;
		let protocol_fee_amount = 2_211_372_968_591;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Decrease(98_125_478_194_552),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(94_745_460_329_654),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_in::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_in.hub_reserve_at_block_start
		);
		println!(
			"state_in::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_in.current_delta_hub_reserve
		);
		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn slip_fee_for_single_buy_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		System::reset_events();

		//Act
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		let amount_in = 93_966_595;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 2_410_240_575;
		let hub_amount_out = 2_409_027_715;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_205_120;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Decrease(2_410_240_575),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(2_409_027_715),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_in::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_in.hub_reserve_at_block_start
		);
		println!(
			"state_in::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_in.current_delta_hub_reserve
		);
		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn slip_fee_for_single_buy_for_lrna_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		System::reset_events();

		//Act
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			LRNA,
			2 * ONE,
			u128::MAX,
		));

		let amount_in = 2_409_032_873;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 0;
		let hub_amount_out = 0;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 0;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: LRNA,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(2_409_032_873),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn slip_fee_for_two_buys_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		//Act
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		let amount_in = 93_967_501;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 2_410_258_642;
		let hub_amount_out = 2_415_060_640;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_205_129;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Decrease(4_820_499_217),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(4_818_065_748),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_in::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_in.hub_reserve_at_block_start
		);
		println!(
			"state_in::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_in.current_delta_hub_reserve
		);
		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn slip_fee_for_two_trades_in_opposite_direction_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE,
			0,
		));

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE,
			u128::MAX,
		));

		let amount_in = 40_297_129_859_941_999;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 50_375_348_046_952;
		let hub_amount_out = 50_413_598_219_992;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 25_187_674_023;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Decrease(1_387_559_622_906),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Increase(128_578_775_931),
			}
		);

		println!("amount_in: {:?}", amount_in);
		println!("amount_out: {:?}", amount_out);
		println!("hub_asset_in: {:?}", hub_amount_in);
		println!("hub_asset_out: {:?}", hub_amount_out);
		println!("asset_protocol: {:?}", asset_fee_amount);
		println!("protocol_fee_amount: {:?}", protocol_fee_amount);

		println!(
			"state_in::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_in.hub_reserve_at_block_start
		);
		println!(
			"state_in::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_in.current_delta_hub_reserve
		);
		println!(
			"state_out::hub_reserve_at_block_start: {:?}",
			hub_asset_block_state_out.hub_reserve_at_block_start
		);
		println!(
			"state_out::current_delta_hub_reserve: {:?}",
			hub_asset_block_state_out.current_delta_hub_reserve
		);
	});
}

#[test]
fn route_execution_for_buy_should_provide_same_result_as_buy() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		crate::dca::init_omnipool_with_oracle_for_block_10();

		System::reset_events();

		//Act
		assert_ok!(Omnipool::calculate_in_given_out(
			PoolType::Omnipool,
			HDX,
			DAI,
			100_000_000_000_000,
		));
		let amount_in = 20_000_000_000;
		let amount_out = 425_104_485_150_786;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 513_790_957_735;
		let asset_fee_amount = 1_065_424_774_814;
		let protocol_fee_amount = 373_378_869;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: DAI,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 2_250_000_000_112_500,
				current_delta_hub_reserve: Decrease(512_883_062_661),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(512_509_683_792),
			}
		);
	});
}
