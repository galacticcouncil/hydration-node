#![cfg(test)]

use crate::dynamic_fees::{init_omnipool, set_balance};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydra_dx_math::omnipool::types::slip_fee::HubAssetBlockState;
use hydra_dx_math::omnipool::types::BalanceUpdate::{Decrease, Increase};
use hydradx_runtime::{FixedU128, Omnipool, Permill, Runtime, RuntimeOrigin, System, Tokens};
use orml_traits::MultiCurrency;
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
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + extra_hub_reserve_amount + protocol_fee_amount
		); // 1125.514164318604
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in); // 8773.9298250000
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		// 2249.487117049839
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
		pretty_assertions::assert_eq!(
			dot_state.hub_reserve,
			initial_dot_state.hub_reserve + amount_in + extra_hub_reserve_amount
		); // 2252.005004556944
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
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125514164318604);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);

		System::reset_events();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE_DOT,
			0,
		));

		let amount_in = 20_000_000_000;
		let amount_out = 424_234_189_586_334;
		let hub_amount_in = 512_649_294_583;
		let hub_amount_out = 512_159_201_858;
		let asset_fee_amount = 1_063_243_582_924;
		let protocol_fee_amount = 490_092_725;
		let extra_hub_reserve_amount = 1_279_814_436;

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
				current_delta_hub_reserve: Decrease(512_883_062_661 + hub_amount_in),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(512_509_683_792 + hub_amount_out),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87759298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2248974467755256);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935480249325262880);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1126028093427623);
		// 'HDX': {
		//     'liquidity': 935480.24949084597958342158326875243764451892871524,
		//     'LRNA': 1126.0265282941166111230950843235114898638691192951,
		// 'DOT': {
		//     'liquidity': 8775.9298249999992549419403076171875,
		//     'LRNA': 2248.9744677552548274652352162143836503455208178988,
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
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + protocol_fee_amount + extra_hub_reserve_amount
		); // 1125.002416237417
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in); // 8771.9392216595
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		// 2249.997589871925
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
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + amount_in + extra_hub_reserve_amount
		); // 1125.002415037455
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
			2_000_000_000_000,
			u128::MAX,
		));

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
		let amount_out = 2_000_000_000_000;
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
				current_delta_hub_reserve: Decrease(2_410_240_575 + hub_amount_in),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: 1_124_999_999_982_000,
				current_delta_hub_reserve: Increase(2_409_027_715 + hub_amount_out),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719486184096);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249995179613281);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 936325588000000000);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125004832503188);
	});
}

#[test]
fn slip_fee_for_two_trades_in_opposite_direction_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);
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

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(512_883_062_661),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(512_509_683_792),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125514164318604);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE_DOT,
			u128::MAX,
		));

		let amount_in = 427_959_822_038_641;
		let amount_out = 20_000_000_000;
		let hub_amount_in = 514_427_161_946;
		let hub_amount_out = 514_168_777_683;
		let asset_fee_amount = 50_125_314;
		let protocol_fee_amount = 257_213_580;
		let extra_hub_reserve_amount = 1_285_715_755;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Increase(1285715022),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Decrease(1917478154),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124999994370238);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002571543277);
	});
}

#[test]
fn slip_fee_for_sell_and_smaller_buy_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);
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

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(512_883_062_661),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(512_509_683_792),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125514164318604);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			1 * ONE_DOT,
			u128::MAX,
		));

		let amount_in = 213978961180937;
		let amount_out = 10_000_000_000;
		let hub_amount_in = 257271233641;
		let hub_amount_out = 257055011110;
		let asset_fee_amount = 25062657;
		let protocol_fee_amount = 128635616;
		let extra_hub_reserve_amount = 642710963;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(255828051551),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(255238450151),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125257021720579);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249744814771912);
	});
}

#[test]
fn slip_fee_for_sell_and_larger_buy_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);
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

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(512_883_062_661),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(512_509_683_792),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125514164318604);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			4 * ONE_DOT,
			u128::MAX,
		));

		let amount_in = 857096627515000;
		let amount_out = 40_000_000_000;
		let hub_amount_in = 1029797138255;
		let hub_amount_out = 1028572657821;
		let asset_fee_amount = 100250627;
		let protocol_fee_amount = 514898569;
		let extra_hub_reserve_amount = 2572607425;

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Increase(515689595160),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Decrease(517287454463),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514849214 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124484882078918);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250518262315085);
	});
}

#[test]
fn slip_fee_for_buy_and_smaller_sell_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);
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
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out);
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + protocol_fee_amount + extra_hub_reserve_amount
		);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);

		System::reset_events();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			1 * ONE,
			0,
		));

		let amount_in = 1_000_000_000_000;
		let amount_out = 46701786;
		let hub_amount_in = 1201503863;
		let hub_amount_out = 1200901822;
		let asset_fee_amount = 117048;
		let protocol_fee_amount = 602041;
		let extra_hub_reserve_amount = 3002254;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Decrease(1209338753),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Increase(1207523852),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 936327588000000000 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125001215335595);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249998793776001);
	});
}

#[test]
fn slip_fee_for_buy_and_larger_sell_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		set_balance(ALICE.into(), HDX, 1_000_000 * UNITS as i128);
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
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out);
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + protocol_fee_amount + extra_hub_reserve_amount
		);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);

		System::reset_events();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			4 * ONE,
			0,
		));

		let amount_in = 4_000_000_000_000;
		let amount_out = 186805956;
		let hub_amount_in = 4806000056;
		let hub_amount_out = 4803586817;
		let asset_fee_amount = 468186;
		let protocol_fee_amount = 2413239;
		let extra_hub_reserve_amount = 12008979;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(HDX).unwrap();
		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		pretty_assertions::assert_eq!(
			hub_asset_block_state_in,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_dot_state.hub_reserve,
				current_delta_hub_reserve: Increase(2393346242),
			}
		);
		pretty_assertions::assert_eq!(
			hub_asset_block_state_out,
			HubAssetBlockState::<pallet_omnipool::types::Balance> {
				hub_reserve_at_block_start: initial_hdx_state.hub_reserve,
				current_delta_hub_reserve: Decrease(2396972341),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 936327588000000000 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124997612650600);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002405467721);
	});
}

#[test]
fn hub_reserve_invariant_should_hold_after_multiple_hdx_trades() {
	TestNet::reset();

	fn assert_hub_asset_invariant() {
		let mut total_balance = 0;
		for asset in Omnipool::list_assets() {
			total_balance += Omnipool::load_asset_state(asset).unwrap().hub_reserve;
		}
		assert_eq!(
			Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
			total_balance,
			"Hub liquidity incorrect\n"
		);
	}

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_hub_token_supply = Tokens::total_issuance(LRNA);

		//Act & Assert
		for _ in 0..3 {
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				5 * ONE,
				0
			));
			assert_hub_asset_invariant();
		}

		for _ in 0..3 {
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				5 * ONE,
				500 * ONE
			));
			assert_hub_asset_invariant();
		}

		for _ in 0..3 {
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				DOT,
				HDX,
				5 * ONE,
				0
			));
			assert_hub_asset_invariant();
		}

		let final_hub_token_supply = Tokens::total_issuance(LRNA);
		pretty_assertions::assert_eq!(initial_hub_token_supply, final_hub_token_supply);
	});
}
