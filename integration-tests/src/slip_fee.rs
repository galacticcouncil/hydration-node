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
		let amount_out = 425_104_485_151_614;
		let hub_amount_in = 512_883_062_661;
		let hub_amount_out = 512_276_309_064;
		let asset_fee_amount = 1_065_424_774_817;
		let protocol_fee_amount = 606_753_597;
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
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out);
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + extra_hub_reserve_amount + protocol_fee_amount
		);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125514164318604);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in);
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);
		// results from the Python implementation
		// 'HDX':
		//     'liquidity': 935904.48351484841472146907766990689679046487440193,
		//     'LRNA': 1125.5141637346374472664443879820567660170692058532,
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
		let protocol_fee_amount = 490_092_806;
		let extra_hub_reserve_amount = 1_281_274_143;

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
		pretty_assertions::assert_eq!(dai_state.reserve, initial_dai_state.reserve - amount_out);
		pretty_assertions::assert_eq!(dai_state.reserve, 49988644541815744431102);
		pretty_assertions::assert_eq!(
			dai_state.hub_reserve,
			initial_dai_state.hub_reserve + hub_amount_out + extra_hub_reserve_amount
		);
		pretty_assertions::assert_eq!(dai_state.hub_reserve, 2250513674243998);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in);
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249487117049839);
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
		let protocol_fee_amount = 1_776_198_934;
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
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve - amount_out);
		pretty_assertions::assert_eq!(dot_state.reserve, 87641658498448);
		pretty_assertions::assert_eq!(
			dot_state.hub_reserve,
			initial_dot_state.hub_reserve + extra_hub_reserve_amount
		);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250005004556944);
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
		let hub_amount_out = 512_276_309_064;
		let asset_fee_amount = 1_065_424_774_817;
		let protocol_fee_amount = 606_753_597;
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
		pretty_assertions::assert_eq!(hdx_state.reserve, 935904483514848386);
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
		let amount_out = 424_234_277_471_896;
		let hub_amount_in = 512_649_294_583;
		let hub_amount_out = 511_693_249_205;
		let asset_fee_amount = 1_063_243_803_188;
		let protocol_fee_amount = 956_045_378;
		let extra_hub_reserve_amount = 1_279_814_701;

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
		pretty_assertions::assert_eq!(hdx_state.reserve, 935480249237376490);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1126028093427888);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 935480.24949084597958342158326875243764451892871524,
		//     'LRNA': 1126.0280916750696011925767048309275477522271454752,
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
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out);
		pretty_assertions::assert_eq!(hdx_state.reserve, 936327588000000000);
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + hub_amount_out + protocol_fee_amount + extra_hub_reserve_amount
		);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125002416245157);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, initial_dot_state.reserve + amount_in);
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, initial_dot_state.hub_reserve - hub_amount_in);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249997589871925);
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
		let protocol_fee_amount = 5_158;
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
		pretty_assertions::assert_eq!(hdx_state.reserve, initial_hdx_state.reserve - amount_out);
		pretty_assertions::assert_eq!(hdx_state.reserve, 936327588000000000);
		pretty_assertions::assert_eq!(
			hdx_state.hub_reserve,
			initial_hdx_state.hub_reserve + extra_hub_reserve_amount
		);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125000006004582);
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

		// results from the Python implementation
		// delta_ra=mpf('428.84205768471479692962474687490919397396885641487056')
		// delta_qi=mpf('-0.51501588455802333909180556431661517625249377639306208')
		// delta_qj=mpf('0.51440388007087671934321319423774487153458148454058335')
		// asset_fee_total=mpf('0.0050125313283208020050125313283208020050125313283199446')
		// lrna_fee_total=mpf('0.0002575079422790116695459027821583075881262468881965311')
		// slip_fee_buy=mpf('0.00011821996365192522075759986082934537361437737134925755')
		// slip_fee_sell=mpf('0.00023627658121568285828886743588265175617166759293100832')
		let amount_in = 427_959_910_942_803;
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124999995540921);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002571543277);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936332.44316894958161330194801002734914044790143969,
		//     'LRNA': 1124.9997370520570728077602259610781054151247386473,
		// 'DOT': {
		//     'liquidity': 8771.9298249999992549419403076171875,
		//     'LRNA': 2250.002829447438674890524668463813081491713089791,
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

		// results from the Python implementation
		// delta_ra=mpf('213.97927592840943636570491092013664187883283014133663')
		// delta_qi=mpf('-0.25727147315567754252048239788941071526754645870203376')
		// delta_qj=mpf('0.25705501110872270545365398505170635112617781758142905')
		// asset_fee_total=mpf('0.0025062656641604010025062656641604010025062656641599723')
		// lrna_fee_total=mpf('0.00012863573657783877126024119894470535763377322935101692')
		// slip_fee_buy=mpf('0.00002923081027463343670463466045586323202747915295086526')
		// slip_fee_sell=mpf('0.00005859550010236485886353697830379555170738873830174605')
		// protocol_fee=mpf('0.00021646204695483706682841283770436414136864112060362844')
		let amount_in = 213_978_916_779_951;
		let amount_out = 10_000_000_000;
		let hub_amount_in = 257_271_180_269;
		let hub_amount_out = 257_055_011_110;
		let asset_fee_amount = 25_062_657;
		let protocol_fee_amount = 216_169_159;
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125257109307494);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249744814771912);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936118.46279077682415783478258082703343234370723207,
		//     'LRNA': 1125.2568922614817697239239055841673553018016593945,
		// 'DOT': {
		//     'liquidity': 8772.9298249999992549419403076171875,
		//     'LRNA': 2249.7450311608638894899685328566928808625899769282,
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

		// results from the Python implementation
		// delta_ra=mpf('857.09628955737522248859267151668876997230445904063994')
		// delta_qi=mpf('-1.0297961768846184859307051487205526268387024711448435')
		// delta_qj=mpf('1.0285726578203751990200618834191573500968883126131935')
		// asset_fee_total=mpf('0.010025062656641604010025062656641604010025062656639889')
		// lrna_fee_total=mpf('0.00051489808844230924296535257436027631341935123557242236')
		// slip_fee_buy=mpf('0.00023585191480310222862882325842922160952151201062870855')
		// slip_fee_sell=mpf('0.00047276906099787543904908946860577881887329528544438868')
		// protocol_fee=mpf('0.0012235190642432869106432653013952767418141585316455199')
		let amount_in = 857_096_805_976_184;
		let amount_out = 40_000_000_000;
		let hub_amount_in = 1_029_797_352_479;
		let hub_amount_out = 1_028_572_657_821;
		let asset_fee_amount = 100_250_627;
		let protocol_fee_amount = 1_224_694_658;
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
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124485591660783);
		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87739298250000 - amount_out);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250518262315085);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936761.57980440578994395767034142358556043717886097,
		//     'LRNA': 1124.484367557752828780513682833336213390230503382,
		// 'DOT': {
		//     'liquidity': 8769.9298249999992549419403076171875,
		//     'LRNA': 2250.5194852439794894220554156806840146661752082027,
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

		// results from the Python implementation
		// delta_ra=mpf('0.0046701787307855095974808448140633381174625216528677702')
		// delta_qi=mpf('-0.0012015038637751654549887612388660100740953324689315007')
		// delta_qj=mpf('0.0012009011703072006334167235748371562672056214547464161')
		// asset_fee_total=mpf('0.000011704708598459923803210137378604857437249427701422994')
		// lrna_fee_total=mpf('0.0000006007519318875827274943806194330050370476662344657512')
		// slip_fee_buy=mpf('0.00000000064546573033892474214834919774982176991380120893291737')
		// slip_fee_sell=mpf('0.0000000012960703468999198011350602230520308934341494100826877')
		// protocol_fee=mpf('0.00000060269346796482157203766402885380688971101418508476737')
		let amount_in = 1_000_000_000_000;
		let amount_out = 46_701_786;
		let hub_amount_in = 1_201_503_863;
		let hub_amount_out = 1_200_901_177;
		let asset_fee_amount = 117_048;
		let protocol_fee_amount = 602_686;
		let extra_hub_reserve_amount = 3_002_254;

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
		pretty_assertions::assert_eq!(hdx_state.reserve, 936328588000000000);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1125001215343980);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.reserve, 87719345514809);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2249998793775356);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936328.5879999999888241291046142578125,
		//     'LRNA': 1125.0012147412812313426973160369159069407203203641,
		// 'DOT': {
		//     'liquidity': 8771.9345514807524932788436083057101305476554141553,
		//     'LRNA': 2249.9987943780408806280507799224573774978481751048,
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

		// results from the Python implementation
		// delta_ra=mpf('0.018680595768106318674636587622958878939714784877056337')
		// delta_qi=mpf('-0.0048060000566600345002392949020552818812992451835723951')
		// delta_qj=mpf('0.0048035817328571563284325488392187591341441583641861894')
		// asset_fee_total=mpf('0.000046818535759664959084302224618944558746152343050266505')
		// lrna_fee_total=mpf('0.0000024030000283300172501196474510276409406496225917862012')
		// slip_fee_buy=mpf('0.0000000051096152918334673324117811683098564100811589950913729')
		// slip_fee_sell=mpf('0.000000010214159256321089294003604326796358027115635424784292')
		// protocol_fee=mpf('0.0000024183238028781718067460628365227471550868193862060766')
		let amount_in = 4_000_000_000_000;
		let amount_out = 186_805_956;
		let hub_amount_in = 4_806_000_056;
		let hub_amount_out = 4_803_581_708;
		let asset_fee_amount = 468_186;
		let protocol_fee_amount = 2_418_348;
		let extra_hub_reserve_amount = 12_008_979;

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
		pretty_assertions::assert_eq!(hdx_state.reserve, 936331588000000000);
		pretty_assertions::assert_eq!(hdx_state.hub_reserve, 1124997612663449);

		let dot_state = Omnipool::load_asset_state(DOT).unwrap();
		pretty_assertions::assert_eq!(dot_state.reserve, 87719392216595 - amount_out);
		pretty_assertions::assert_eq!(dot_state.reserve, 87719205410639);
		pretty_assertions::assert_eq!(dot_state.hub_reserve, 2250002405462612);
		// results from the Python implementation
		// 'HDX': {
		//     'liquidity': 936331.5879999999888241291046142578125,
		//     'LRNA': 1124.9976102450883464736520655032527176689131164514,
		// 'DOT': {
		//     'liquidity': 8771.9205410637151724697664525629012350068331618921,
		//     'LRNA': 2250.0024078809480472863887788436355979901783831907,
	});
}
