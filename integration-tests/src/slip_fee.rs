#![cfg(test)]

use crate::dynamic_fees::{init_omnipool, set_balance};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydra_dx_math::omnipool::types::slip_fee::HubAssetBlockState;
use hydra_dx_math::omnipool::types::BalanceUpdate::{Decrease, Increase};
use hydradx_runtime::{Omnipool, RuntimeOrigin, System};
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

		let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(initial_hdx_state.reserve, 936329588000000000);
		pretty_assertions::assert_eq!(initial_hdx_state.hub_reserve, 1124999999982000);
		let initial_dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(initial_dot_state.reserve, 87719298250000);
		pretty_assertions::assert_eq!(initial_dot_state.hub_reserve, 2250000000112500);

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE_DOT,
			0,
		));

		// results from the Python implementation
		// delta_ra=mpf('425.10448515157410266002694435091570953512559806778283')
		// delta_qi=mpf('-0.51288306266171900644296117704645027370135884899976081')
		// delta_qj=mpf('0.51227630906395000763066977688365571365222163350224179')
		// asset_fee_total=mpf('1.0654247748159751946366590083982849863035729274881772')
		// lrna_fee_total=mpf('0.00025644153133085950322148058852322513685067942449988082')
		// slip_fee_buy=mpf('0.0002333747281571142325827135540903374280109443826342537')
		// slip_fee_sell=mpf('0.00011693733828102507648720602018099748427559169038469382')
		let amount_in = 20_000_000_000;
		let amount_out = 425_104_485_150_786;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 512_509_683_792;
		let asset_fee_amount = 1_065_424_774_814;
		let protocol_fee_amount = 373_378_869;
		let extra_hub_reserve_amount = 1_281_273_943;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(hub_amount_in),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(hub_amount_out),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out); // 935904.483514849214
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, initial_hdx_state.hub_reserve + hub_amount_out + extra_hub_reserve_amount + protocol_fee_amount); // 1125.514164318604
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in); // 8773.9298250000
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in); // 2249.487117049839
		// results from the Python implementation
		// 'HDX':
		//     'liquidity': 935904.48351484841472146907766990689679046487440193,
		//     'LRNA': 1125.5135569810396782676320965818939714570200686377,
		// 'DOT':
		//     'liquidity': 8773.9298249999992549419403076171875,
		//     'LRNA': 2249.487117049838228129359397211854306562236141151,
	});
}

#[test]
fn slip_fee_for_single_sell_lrna_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(initial_dot_state.reserve, 87719298250000);
		pretty_assertions::assert_eq!(initial_dot_state.hub_reserve, 2250000000112500);

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			LRNA,
			DOT,
			2 * UNITS,
			0,
		));

		// results from the Python implementation
		// delta_ra=mpf('7.7639751552801915346390711887043757385102148728396952')
		// delta_qi=0.0
		// delta_qj=mpf('1.9982238010658080916013318197988055676012781459299828')
		// asset_fee_total=mpf('0.01945858434907316174095005310452224495867221772641527')
		// lrna_fee_total=0.0
		// slip_fee_buy=mpf('0.0017761989341919083986681802011944323987218540700185338')
		// slip_fee_sell=0.0
		let amount_in = 2_000_000_000_000;
		let amount_out = 77_639_751_552;
		let hub_amount_in = 0;
		let hub_amount_out = 0;
		let asset_fee_amount = 194_585_844;
		let protocol_fee_amount = 0;
		let extra_hub_reserve_amount = 5_004_444_444;

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
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Increase(amount_in),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve - amount_out); // 8764.1658498448
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve + amount_in + extra_hub_reserve_amount); // 2252.005004556944
		// results from the Python implementation
		// 'DOT': {
		//     'liquidity': 8764.1658498447190634073012364284831242614897851272,
		//     'LRNA': 2252.0032194612411152209616668126535007887691892918,
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

		let amount_in = 2_000_000_000_000;
		let amount_out = 37_412_066_789_861_635;
		let hub_amount_in = 50_156_433_320_353;
		let hub_amount_out = 49_110_044_651_337;
		let asset_fee_amount = 93_764_578_420_706;
		let protocol_fee_amount = 1_168_644_896_307;

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
				current_delta_hub_reserve: Decrease(50_156_433_320_353),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(48_987_788_424_046),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 89_719_298_250_000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2_199_843_566_792_147);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 898_917_521_210_138_365);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1_175_278_689_529_644);

		System::reset_events();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE,
			0,
		));

		let amount_in = 2_000_000_000_000;
		let amount_out = 31_982_041_970_335_103;
		let hub_amount_in = 47_969_044_874_199;
		let hub_amount_out = 45_870_365_896_418;
		let asset_fee_amount = 80_155_493_659_988;
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

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 91_719_298_250_000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2_151_874_521_917_948);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 866_935_479_239_803_262);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1_223_360_428_394_653);
	});
}

#[test]
fn slip_fee_for_single_buy_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(initial_hdx_state.reserve, 936329588000000000);
		pretty_assertions::assert_eq!(initial_hdx_state.hub_reserve, 1124999999982000);
		let initial_dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(initial_dot_state.reserve, 87719298250000);
		pretty_assertions::assert_eq!(initial_dot_state.hub_reserve, 2250000000112500);

		//Act
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		// results from the Python implementation
		// delta_ra=mpf('0.0093966594840238465007815333366938857728766769893578331')
		// delta_qi=mpf('-0.0024102405757561808993746609079017228297585398765218622')
		// delta_qj=mpf('0.0024090277149706257736363698232416045948288234720429954')
		// asset_fee_total=mpf('0.0050125313283208020050125313283208020050125313283199446')
		// lrna_fee_total=mpf('0.0000012051202878780904496873304539508614148792699382609338')
		// slip_fee_buy=mpf('0.0000000051586017411591240395712743313581216264233695892076958')
		// slip_fee_sell=mpf('0.0000000025818959358761645641829318360153932107111710163343582')
		let amount_in = 93_966_595;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 2_410_240_575;
		let hub_amount_out = 2_409_027_715;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_205_120;
		let extra_hub_reserve_amount = 6_022_582;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(hub_amount_in),
			}
		);
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(hub_amount_out),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out); // 936327.588000000000
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, initial_hdx_state.hub_reserve + hub_amount_out + protocol_fee_amount + extra_hub_reserve_amount); // 1125.002416237417
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in); // 8771.9392216595
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in); // 2249.997589871925
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936327.5879999999888241291046142578125,
		//     'LRNA': 1125.0024150322842209530265665070701128325594859802,
		// 'DOT': {
		//     'liquidity': 8771.939221659483278788441089150524193885772876677,
		//     'LRNA': 2249.9975898719241909549029837279928551131077414601,
	});
}

#[test]
fn slip_fee_for_single_buy_for_lrna_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(initial_hdx_state.reserve, 936329588000000000);
		pretty_assertions::assert_eq!(initial_hdx_state.hub_reserve, 1124999999982000);

		//Act
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			LRNA,
			2 * ONE,
			u128::MAX,
		));

		// results from the Python implementation
		// delta_ra=mpf('0.0024090328735723669327604093945159359529504498954125827')
		// delta_qi=0.0
		// delta_qj=mpf('0.0024090277149706257736363698232416045948288234720429954')
		// asset_fee_total=mpf('0.0050125313283208020050125313283208020050125313283199446')
		// lrna_fee_total=0.0
		// slip_fee_buy=mpf('0.0000000051586017411591240395712743313581216264233695892076958')
		// slip_fee_sell=0.0
		let amount_in = 2_409_032_873;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 0;
		let hub_amount_out = 0;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 0;
		let extra_hub_reserve_amount = 6_022_582;

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
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(amount_in),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out); // 936327.588000000000
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, initial_hdx_state.hub_reserve + amount_in + extra_hub_reserve_amount); // 1125.002415037455
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936327.5879999999888241291046142578125,
		//     'LRNA': 1125.0024150322842209530265665070701128325594859802,
	});
}

#[test]
fn slip_fee_for_two_buys_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		//Act
		let buy_amount = 2 * ONE;
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			buy_amount,
			u128::MAX,
		));

		let amount_in = 93_966_595;
		let amount_out = buy_amount;
		let hub_amount_in = 2_410_240_575;
		let hub_amount_out = 2_409_027_715;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_205_120;
		let extra_hub_reserve_amount = 6_022_582;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
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
				current_delta_hub_reserve: Decrease(hub_amount_in),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(hub_amount_out),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87_719_392_216_595);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2_249_997_589_871_925);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 936_327_588_000_000_000);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1_125_002_416_237_417);

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		let amount_in = 93_967_501;
		let amount_out = buy_amount;
		let hub_amount_in = 2_410_258_644;
		let hub_amount_out = 2_409_038_035;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_205_129;
		let extra_hub_reserve_amount = 6_022_607;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
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
				current_delta_hub_reserve: Decrease(4_820_499_219),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(4_818_065_750),
			}
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

		let amount_in = 40255265787350153;
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
	});
}
