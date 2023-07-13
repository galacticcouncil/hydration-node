#![cfg(test)]

use crate::polkadot_test_net::*;
use common_runtime::CORE_ASSET_ID;
use frame_support::weights::Weight;
use frame_support::{assert_ok, pallet_prelude::*};
use orml_traits::currency::MultiCurrency;
use polkadot_xcm::{latest::prelude::*, VersionedXcm};
use pretty_assertions::assert_eq;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

use frame_support::dispatch::GetDispatchInfo;

pub const SELL: bool = true;
pub const BUY: bool = false;

fn craft_exchange_asset_xcm<M: Into<MultiAssets>, RC: Decode + GetDispatchInfo>(
	give: MultiAsset,
	want: M,
	is_sell: bool,
) -> VersionedXcm<RC> {
	use polkadot_runtime::xcm_config::BaseXcmWeight;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let dest = MultiLocation::new(1, Parachain(HYDRA_PARA_ID));
	let beneficiary = Junction::AccountId32 { id: BOB, network: None }.into();
	let assets: MultiAssets = MultiAsset::from((GeneralIndex(0), 100 * UNITS)).into(); // hardcoded
	let max_assets = assets.len() as u32 + 1;
	let context = X2(GlobalConsensus(NetworkId::Polkadot), Parachain(ACALA_PARA_ID));
	let fees = assets
		.get(0)
		.expect("should have at least 1 asset")
		.clone()
		.reanchored(&dest, context)
		.expect("should reanchor");
	let give = give.reanchored(&dest, context).expect("should reanchor give");
	let give: MultiAssetFilter = Definite(give.into());
	let want = want.into();
	let weight_limit = {
		let fees = fees.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets.clone()),
			ClearOrigin,
			BuyExecution {
				fees,
				weight_limit: Limited(Weight::zero()),
			},
			ExchangeAsset {
				give: give.clone(),
				want: want.clone(),
				maximal: is_sell,
			},
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
			},
		]);
		// use local weight for remote message and hope for the best.
		let remote_weight = Weigher::weight(&mut remote_message).expect("weighing should not fail");
		Limited(remote_weight)
	};
	// executed on remote (on hydra)
	let xcm = Xcm(vec![
		BuyExecution { fees, weight_limit },
		ExchangeAsset {
			give,
			want,
			maximal: is_sell,
		},
		DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		},
	]);
	// executed on local (acala)
	let message = Xcm(vec![
		SetFeesMode { jit_withdraw: true },
		TransferReserveAsset { assets, dest, xcm },
	]);
	VersionedXcm::V3(message)
}

#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_sell() {
	//Arrange
	TestNet::reset();

	let aca = 1234;
	let mut price = None;
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"ACA".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(aca),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(aca, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			aca,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
		use hydradx_traits::pools::SpotPriceProvider;
		price = hydradx_runtime::Omnipool::spot_price(CORE_ASSET_ID, aca);
	});

	Acala::execute_with(|| {
		let xcm = craft_exchange_asset_xcm::<_, hydradx_runtime::RuntimeCall>(
			MultiAsset::from((GeneralIndex(0), 50 * UNITS)),
			MultiAsset::from((GeneralIndex(CORE_ASSET_ID.into()), 300 * UNITS)),
			SELL,
		);
		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_ref_time(399_600_000_000),
		);
		assert_ok!(res);

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE_ON_OTHER_PARACHAIN - 100 * UNITS
		);
		// TODO: add utility macro?
		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	let fees = 500801282051;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(aca, &AccountId::from(BOB)),
			50 * UNITS - fees
		);
		// We receive about 39_101 HDX
		let received = 39_101 * UNITS + BOB_INITIAL_NATIVE_BALANCE + 207_131_554_396;
		assert_eq!(hydradx_runtime::Balances::free_balance(&AccountId::from(BOB)), received);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(aca, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}

//TODO: double check if this buy make sense, especially in the end, bob's aca balanced changed more than the fee
#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_buy() {
	//Arrange
	TestNet::reset();

	let aca = 1234;
	let mut price = None;
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"ACA".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(aca),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(aca, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			aca,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
		use hydradx_traits::pools::SpotPriceProvider;
		price = hydradx_runtime::Omnipool::spot_price(CORE_ASSET_ID, aca);
	});

	Acala::execute_with(|| {
		let xcm = craft_exchange_asset_xcm::<_, hydradx_runtime::RuntimeCall>(
			MultiAsset::from((GeneralIndex(0), 50 * UNITS)),
			MultiAsset::from((GeneralIndex(CORE_ASSET_ID.into()), 300 * UNITS)),
			BUY,
		);
		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_ref_time(399_600_000_000),
		);
		assert_ok!(res);

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE_ON_OTHER_PARACHAIN - 100 * UNITS
		);
		// TODO: add utility macro?
		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	let fees = 862495197993;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(aca, &AccountId::from(BOB)),
			100 * UNITS - fees
		);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(aca, &hydradx_runtime::Treasury::account_id()),
			500801282051
		);
	});
}

// TODO test with Acala -> Hydra Swap -> Acala
// TODO: we want to make sure that the different combinations work
// TODO: implement the most complex version: 4hops, 5 chains involved

// Support different transfers of swap results
// send HDX back to Acala
// DepositReserveAsset { assets: hdx_filter, dest: acala, xcm:
// 	Xcm(vec![BuyExecution { fees, weight_limit }, DepositAsset {
// 		assets: Wild(AllCounted(max_assets)),
// 		beneficiary,
// 	}])
// },
// send ACA back to Acala
// InitiateReserveWithdraw { assets: aca_filter, reserve: acala, xcm:
// 	Xcm(vec![BuyExecution { fees, weight_limit }, DepositAsset {
// 		assets: Wild(AllCounted(max_assets)),
// 		beneficiary,
// 	}])
// },
// send BTC back to Acala
// InitiateReserveWithdraw { assets: btc_filter, reserve: interlay, xcm: // Hydra
// 	Xcm(vec![BuyExecution { fees, weight_limit }, DepositReserveAsset { // Interlay
// 		assets: Wild(AllCounted(max_assets)),
// 		dest: acala,
// 		xcm: Xcm(vec![BuyExecution { fees, weight_limit }, DepositAsset { // Acala
// 			assets: Wild(AllCounted(max_assets)),
// 			beneficiary,
// 		}]),
// 	}])
// },
// send HDX to interlay
// send BTC to interlay
// send ACA to interlay
