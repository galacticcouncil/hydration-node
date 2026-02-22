#![cfg(test)]

use crate::dynamic_fees::{init_omnipool, set_balance};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydra_dx_math::omnipool::types::slip_fee::HubAssetBlockState;
use hydra_dx_math::omnipool::types::BalanceUpdate::{Decrease, Increase};
use hydradx_runtime::{Omnipool, RuntimeOrigin, System, Tokens};
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
		let amount_out = 425_104_485_151_614;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 512_276_309_064;
		let asset_fee_amount = 1_065_424_774_817;
		let protocol_fee_amount = 373_378_869;
		let extra_hub_reserve_amount = 1_280_689_975;

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
fn slip_fee_for_single_sell_without_hdx_should_provide_correct_results() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(initial_dot_state.reserve, 87719298250000);
		pretty_assertions::assert_eq!(initial_dot_state.hub_reserve, 2250000000112500);
		let initial_dai_state = Omnipool::load_asset_state(DAI).unwrap();
		pretty_assertions::assert_eq!(initial_dai_state.reserve, 50000000000000000000000);
		pretty_assertions::assert_eq!(initial_dai_state.hub_reserve, 2250000000000000);

		//Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			DAI,
			2 * ONE_DOT,
			0,
		));

		let amount_in = 20_000_000_000;
		let amount_out = 11_355_458_184_255_568_898;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 512_392_969_855;
		let asset_fee_amount = 28_459_794_948_008_945;
		let protocol_fee_amount = 373_378_869;
		let extra_hub_reserve_amount = 1_280_982_225;

		expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
			who: ALICE.into(),
			asset_in: DOT,
			asset_out: DAI,
			amount_in,
			amount_out,
			hub_amount_in,
			hub_amount_out: hub_amount_out + extra_hub_reserve_amount,
			asset_fee_amount,
			protocol_fee_amount,
		}
		.into()]);

		let hub_asset_block_state_in = Omnipool::hub_asset_block_state(DOT).unwrap();
		let hub_asset_block_state_out = Omnipool::hub_asset_block_state(DAI).unwrap();
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
				hub_reserve_at_block_start: initial_dai_state.hub_reserve,
				current_delta_hub_reserve: Increase(hub_amount_out),
			}
		);

		let dai_state = Omnipool::load_asset_state(DAI).unwrap();
		pretty_assertions::assert_eq!(dai_state.reserve, initial_dai_state.reserve - amount_out); // 49988644541.815744431102
		pretty_assertions::assert_eq!(
			dai_state.hub_reserve,
			initial_dai_state.hub_reserve + hub_amount_out + extra_hub_reserve_amount
		); // 2250.513674243998
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in); // 8773.9298250000
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		// 2249.487117049839
		// results from the Python implementation
		// 'DOT':
		//     'liquidity': 8773.9298249999992549419403076171875,
		//     'LRNA': 2249.487117049838228129359397211854306562236141151,
		// 'DAI':
		//     'liquidity': 49988644541.815749025431469746734355850581929762778,
		//     'LRNA': 2250.5136739520801000899913790026661378105390440333,,
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
		let amount_out = 425_104_485_151_614;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 512276309064;
		let asset_fee_amount = 1065424774817;
		let protocol_fee_amount = 373_378_869;
		let extra_hub_reserve_amount = 1280689975;

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
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125513930359908);
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
		let amount_out = 424234365616691;
		let hub_amount_in = 512_649_294_583;
		let hub_amount_out = 511693249205;
		let asset_fee_amount = 1063244024102;
		let protocol_fee_amount = 490_092_725;
		let extra_hub_reserve_amount = 1278648761;

		// results from the Python implementation
		// delta_ra=mpf('424.23402400243513804749440115445914594594568668965051')
		// delta_qi=mpf('-0.51264929458340066412418099747065621671532325215935861')
		// delta_qj=mpf('0.51169266722786664215104953541223011098239671877856259')
		// asset_fee_total=mpf('1.0632431679259026016227929853495216690374578613775697')
		// lrna_fee_total=mpf('0.00025632464729170033206209049873532810835766162607967942')
		// slip_fee_buy=mpf('0.00046653462992397932228919198389672063853327491431479485')
		// slip_fee_sell=mpf('0.0002337680783183423187801795757940569860355968404017346')
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
				current_delta_hub_reserve: Increase(512_276_309_064 + hub_amount_out),
			}
		);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87759298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2248974467755256);
		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935480249149231695);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1126027392350599);
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
		let protocol_fee_amount = 1_212_860;
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
		let protocol_fee_amount = 1_212_860;
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1_125_002_416_245_157);

		System::reset_events();

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			2 * ONE,
			u128::MAX,
		));

		// results from the Python implementation
		// delta_ra=mpf('0.0093967501025340413925238442852480902904933930551039794')
		// delta_qi=mpf('-0.0024102586555236411330982850358753297512809864377820228')
		// delta_qj=mpf('0.0024090380321740528595243310994567412197428448855997889')
		// asset_fee_total=mpf('0.0050125313283208020050125313283208020050125313283199446')
		// lrna_fee_total=mpf('0.0000012051293277618205665491425179376648756404932188910135')
		// slip_fee_buy=mpf('0.000000010330166320711757691659480821795053306020274480507411')
		// slip_fee_sell=mpf('0.0000000051638555057412497131344198290716091950386888602560828')
		let amount_in = 93_967_501;
		let amount_out = 2_000_000_000_000;
		let hub_amount_in = 2_410_258_644;
		let hub_amount_out = 2_409_038_035;
		let asset_fee_amount = 5_012_531_329;
		let protocol_fee_amount = 1_220_609;
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125004832526408);
		// results from the Python implementation
		// 'HDX':
		//     'liquidity': 936325.5879999999888241291046142578125,
		//     'LRNA': 1125.0048300928985464741295812857080084205011302247,
		// 'DOT':
		//     'liquidity': 8771.9486184095858128298336129948094419760633700701,
		//     'LRNA': 2249.9951796132686673137698854429569797833564604737,
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
				current_delta_hub_reserve: Increase(512276309064),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125513930359908);
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

		// results from the Python implementation
		// delta_ra=mpf('428.84205768471479692962474687490919397396885641487056')
		// delta_qi=mpf('-0.51501588455802333909180556431661517625249377639306208')
		// delta_qj=mpf('0.51440388007087671934321319423774487153458148454058335')
		// asset_fee_total=mpf('0.0050125313283208020050125313283208020050125313283199446')
		// lrna_fee_total=mpf('0.0002575079422790116695459027821583075881262468881965311')
		// slip_fee_buy=mpf('0.00011821996365192522075759986082934537361437737134925755')
		// slip_fee_sell=mpf('0.00023627658121568285828886743588265175617166759293100832')
		let amount_in = 427_959_999_942_791;
		let amount_out = 20_000_000_000;
		let hub_amount_in = 514_427_268_764;
		let hub_amount_out = 514_168_777_683;
		let asset_fee_amount = 50_125_314;
		let protocol_fee_amount = 258_491_081;
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
				current_delta_hub_reserve: Decrease(2150959700),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124999761582225);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002571543277);
		// results from the Python implementation
		// 'HDX':
		//     'liquidity': 936761.28522663429641023157275690225833442187029611,
		//     'LRNA': 1124.4841144139012804698561289965986956788231076554,
		// 'DOT':
		//     'liquidity': 8769.9298249999992549419403076171875,
		//     'LRNA': 2250.5182614306387260303931719295121233113215857116,
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
				current_delta_hub_reserve: Increase(512_276_309_064),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125513930359908);
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

		// results from the Python implementation
		let amount_in = 213978961269560;
		let amount_out = 10_000_000_000;
		let hub_amount_in = 257271180269;
		let hub_amount_out = 257_055_011_110;
		let asset_fee_amount = 25_062_657;
		let protocol_fee_amount = 216169159;
		let extra_hub_reserve_amount = 642_710_963;

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
				current_delta_hub_reserve: Increase(255005128795),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125256875348798);
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
				current_delta_hub_reserve: Increase(512_276_309_064),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125513930359908);
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

		let amount_in = 857096984302623;
		let amount_out = 40_000_000_000;
		let hub_amount_in = 1029797352479;
		let hub_amount_out = 1028572657821;
		let asset_fee_amount = 100_250_627;
		let protocol_fee_amount = 1224694658;
		let extra_hub_reserve_amount = 2_572_607_425;

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
				current_delta_hub_reserve: Decrease(517521043415),
			}
		);

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386 + amount_in);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124485357702087);
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
		let protocol_fee_amount = 1_212_860;
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
		let hub_amount_out = 1200901177;
		let asset_fee_amount = 117048;
		let protocol_fee_amount = 602041;
		let extra_hub_reserve_amount = 3002252;

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
				current_delta_hub_reserve: Decrease(1209339398),
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125001215343335);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249998793775354);
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
		let protocol_fee_amount = 1_212_860;
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
		let hub_amount_out = 4803581708;
		let asset_fee_amount = 468186;
		let protocol_fee_amount = 2413239;
		let extra_hub_reserve_amount = 12008967;

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
				current_delta_hub_reserve: Increase(2393341133),
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124997612658340);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002405462600);
	});
}

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

#[test]
fn test_me() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		System::reset_events();

		let initial_hub_token_supply = Tokens::total_issuance(LRNA);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			DOT,
			HDX,
			2 * ONE_DOT,
			0
		));
		assert_hub_asset_invariant();

		let final_hub_token_supply = Tokens::total_issuance(LRNA);
		pretty_assertions::assert_eq!(initial_hub_token_supply, final_hub_token_supply);
		// 23007_607_932_702_426 23007_608_980_017_673
		// 1_047_315_247
	});
}

#[test]
fn hub_reserve_invariant_should_hold_after_multiple_hdx_trades() {
	TestNet::reset();
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

#[test]
fn hub_reserve_invariant_should_hold_after_multiple_non_hdx_trades() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		let initial_hub_token_supply = Tokens::total_issuance(LRNA);

		//Act & Assert
		for _ in 0..3 {
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				DOT,
				5 * ONE,
				0
			));
			assert_hub_asset_invariant();
		}

		for _ in 0..3 {
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
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
				DAI,
				5 * ONE,
				0
			));
			assert_hub_asset_invariant();
		}

		let final_hub_token_supply = Tokens::total_issuance(LRNA);
		pretty_assertions::assert_eq!(initial_hub_token_supply, final_hub_token_supply);
	});
}
