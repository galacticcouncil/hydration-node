#![cfg(test)]

use crate::polkadot_test_net::*;
use cumulus_primitives_core::ParaId;
use frame_support::weights::Weight;
use frame_support::{
	assert_ok,
	pallet_prelude::*,
	sp_runtime::{FixedU128, Permill},
	traits::Contains,
};
use hex_literal::hex;
use orml_traits::currency::MultiCurrency;
use polkadot_xcm::{latest::prelude::*, v3::WeightLimit, VersionedMultiAssets, VersionedXcm};
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, BlakeTwo256, Hash};
use xcm_emulator::TestExt;

use frame_support::dispatch::GetDispatchInfo;

fn craft_exchange_asset_xcm<M: Into<MultiAssets>, RC: Decode + GetDispatchInfo>(give: M, want: M) -> VersionedXcm<RC> {
	use polkadot_runtime::xcm_config::BaseXcmWeight;
	use sp_runtime::traits::ConstU32;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let dest = Parachain(HYDRA_PARA_ID).into();
	let beneficiary = Junction::AccountId32 { id: BOB, network: None }.into();
	let assets = give.into();
	let max_assets = assets.len() as u32;
	let context = GlobalConsensus(NetworkId::Polkadot).into();
	let fees = assets
		.get(0)
		.expect("should have at least 1 asset")
		.clone()
		.reanchored(&dest, context)
		.expect("should reanchor");
	let weight_limit = {
		let fees = fees.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets.clone()),
			ClearOrigin,
			BuyExecution {
				fees,
				weight_limit: Limited(Weight::zero()),
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
	let give = assets.clone().into();
	let want = want.into();
	let xcm = Xcm(vec![
		BuyExecution { fees, weight_limit },
		ExchangeAsset {
			give,
			want,
			maximal: true,
		},
		DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		},
	]);
	let message = Xcm(vec![
		SetFeesMode { jit_withdraw: true },
		TransferReserveAsset { assets, dest, xcm },
	]);
	VersionedXcm::V3(message)
}

#[test]
fn hydra_should_swap_assets_when_receiving_from_relay() {
	//Arrange
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));
	});

	Acala::execute_with(|| {
		let xcm = craft_exchange_asset_xcm::<_, hydradx_runtime::RuntimeCall>(
			MultiAsset::from((Here, 300 * UNITS)),
			MultiAsset::from((Here, 300 * UNITS)),
		);
		//Act
		assert_ok!(hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::MAX,
		));
		// assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
		// 	polkadot_runtime::RuntimeOrigin::signed(ALICE.into()),
		// 	Box::new(Parachain(HYDRA_PARA_ID).into_versioned()),
		// 	Box::new(Junction::AccountId32 { id: BOB, network: None }.into()),
		// 	Box::new((Here, 300 * UNITS).into()),
		// 	0,
		// ));

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&ParaId::from(HYDRA_PARA_ID).into_account_truncating()),
			310 * UNITS
		);
	});

	let fees = 400641025641;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS - fees
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}
