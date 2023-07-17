#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::fungible::Balanced;
use frame_support::weights::Weight;
use frame_support::{assert_ok, pallet_prelude::*};
use orml_traits::currency::MultiCurrency;
use polkadot_xcm::{latest::prelude::*, VersionedXcm};
use pretty_assertions::assert_eq;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

pub const SELL: bool = true;
pub const BUY: bool = false;

fn craft_exchange_asset_xcm2<RC: Decode + GetDispatchInfo>(
	give_asset: MultiAsset,
	want_asset: MultiAsset,
	is_sell: bool,
) -> VersionedXcm<RC> {
	use polkadot_runtime::xcm_config::BaseXcmWeight;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let give_reserve_chain = MultiLocation::new(1, Parachain(MOONBEAM_PARA_ID));
	let want_reserve_chain = MultiLocation::new(1, Parachain(INTERLAY_PARA_ID));
	let swap_chain = MultiLocation::new(1, Parachain(HYDRA_PARA_ID));
	let dest = MultiLocation::new(1, Parachain(ACALA_PARA_ID));
	let beneficiary = Junction::AccountId32 { id: BOB, network: None }.into();
	let assets: MultiAssets = MultiAsset::from((GeneralIndex(0), 100 * UNITS)).into(); // hardcoded
	let max_assets = assets.len() as u32 + 1;
	let origin_context = X2(GlobalConsensus(NetworkId::Polkadot), Parachain(ACALA_PARA_ID));
	let give = give_asset
		.clone()
		.reanchored(&dest, origin_context)
		.expect("should reanchor give");
	let give: MultiAssetFilter = Definite(give.clone().into());
	let want: MultiAssets = want_asset.clone().into();

	let fees = give_asset
		.clone()
		.reanchored(&swap_chain, give_reserve_chain.interior)
		.expect("should reanchor");

	let reserve_fees = want_asset
		.clone()
		.reanchored(&want_reserve_chain, swap_chain.interior)
		.expect("should reanchor");

	let destination_fee = want_asset
		.clone()
		.reanchored(&dest, want_reserve_chain.interior)
		.expect("should reanchor");

	let weight_limit = {
		let fees = fees.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets.clone()),
			ClearOrigin,
			BuyExecution {
				fees: fees.clone(),
				weight_limit: Limited(Weight::zero()),
			},
			ExchangeAsset {
				give: give.clone(),
				want: want.clone(),
				maximal: is_sell,
			},
			InitiateReserveWithdraw {
				assets: want.clone().into(),
				reserve: want_reserve_chain,
				xcm: Xcm(vec![
					BuyExecution {
						fees: reserve_fees.clone(), //reserve fee
						weight_limit: Limited(Weight::zero()),
					},
					DepositReserveAsset {
						assets: Wild(AllCounted(max_assets)),
						dest,
						xcm: Xcm(vec![
							BuyExecution {
								fees: destination_fee.clone(), //destination fee
								weight_limit: Limited(Weight::zero()),
							},
							DepositAsset {
								assets: Wild(AllCounted(max_assets)),
								beneficiary,
							},
						]),
					},
				]),
			},
		]);
		// use local weight for remote message and hope for the best.
		let remote_weight = Weigher::weight(&mut remote_message).expect("weighing should not fail");
		Limited(remote_weight)
	};

	// executed on remote (on hydra)
	let xcm = Xcm(vec![
		BuyExecution {
			fees: half(&fees),
			weight_limit: weight_limit.clone(),
		},
		ExchangeAsset {
			give: give.clone(),
			want: want.clone(),
			maximal: is_sell,
		},
		InitiateReserveWithdraw {
			assets: want.into(),
			reserve: want_reserve_chain,
			xcm: Xcm(vec![
				BuyExecution {
					fees: half(&reserve_fees),
					weight_limit: weight_limit.clone(),
				},
				DepositReserveAsset {
					assets: Wild(AllCounted(max_assets)),
					dest,
					xcm: Xcm(vec![
						BuyExecution {
							fees: half(&destination_fee),
							weight_limit: weight_limit.clone(),
						},
						DepositAsset {
							assets: Wild(AllCounted(max_assets)),
							beneficiary,
						},
					]),
				},
			]),
		},
	]);

	let give_reserve_fees = give_asset
		.clone()
		.reanchored(&give_reserve_chain, origin_context)
		.expect("should reanchor");

	// executed on local (acala)
	let message = Xcm(vec![
		WithdrawAsset(give_asset.clone().into()),
		InitiateReserveWithdraw {
			assets: All.into(),
			reserve: give_reserve_chain,
			xcm: Xcm(vec![
				BuyExecution {
					fees: half(&give_reserve_fees),
					weight_limit: weight_limit.clone(),
				},
				DepositReserveAsset {
					assets: AllCounted(max_assets).into(),
					dest: swap_chain,
					xcm,
				},
			]),
		},
	]);
	VersionedXcm::V3(message)
}

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
			ALICE_INITIAL_NATIVE_BALANCE - 100 * UNITS
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
			ALICE_INITIAL_NATIVE_BALANCE - 100 * UNITS
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

#[test]
fn hydra_should_transfer_and_swap_send_back_to_acala() {
	//Arrange
	TestNet::reset();

	let moon = 4567; //TODO: RENAME TO glmr
	let btc = 7890;
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"MOON".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(moon),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"iBTC".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(btc),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(INTERLAY_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			moon,
			FixedU128::from(1),
		));
		use hydradx_traits::NativePriceOracle;
		// assert_eq!(hydradx_runtime::MultiTransactionPayment::price(moon).unwrap(), FixedU128::from(1));
		// make sure the price is propagated
		hydradx_runtime::MultiTransactionPayment::on_initialize(hydradx_runtime::System::block_number());
		assert_eq!(hydradx_runtime::MultiTransactionPayment::price(moon).unwrap(), FixedU128::from(1));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(moon, &omnipool_account, 3000 * UNITS));
		assert_ok!(hydradx_runtime::Tokens::deposit(btc, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			moon,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			btc,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
	});

	Moonbeam::execute_with(|| {
		use xcm_executor::traits::Convert;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert((Parent, Parachain(ACALA_PARA_ID)).into()).unwrap();
		hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS).expect("Failed to deposit");
	});

	Interlay::execute_with(|| {
		use xcm_executor::traits::Convert;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert((Parent, Parachain(HYDRA_PARA_ID)).into()).unwrap();
		hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS).expect("Failed to deposit");
	});

	Acala::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"MOON".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(moon),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			hydradx_runtime::RuntimeOrigin::root(),
			b"iBTC".to_vec(),
			pallet_asset_registry::AssetType::Token,
			1_000_000,
			Some(btc),
			None,
			Some(hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Parachain(INTERLAY_PARA_ID), GeneralIndex(0))
			))),
			None
		));

		assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			btc,
			FixedU128::from(1),
		));
		// make sure the price is propagated
		hydradx_runtime::MultiTransactionPayment::on_initialize(hydradx_runtime::System::block_number());

		let alice_init_moon_balance = 3000 * UNITS;
		assert_ok!(hydradx_runtime::Tokens::deposit(
			moon,
			&ALICE.into(),
			alice_init_moon_balance
		));

		//Act

		let give_amount = 1000 * UNITS;
		let give = MultiAsset::from((hydradx_runtime::CurrencyIdConvert::convert(moon).unwrap(), give_amount));
		let want = MultiAsset::from((hydradx_runtime::CurrencyIdConvert::convert(btc).unwrap(), 550 * UNITS));

		let xcm = craft_exchange_asset_xcm2::<hydradx_runtime::RuntimeCall>(give, want, SELL);
		assert_ok!(hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_ref_time(399_600_000_000),
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(moon, &AccountId::from(ALICE)),
			alice_init_moon_balance - give_amount
		);
		// TODO: add utility macro?
		/*assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));*/
	});

	//let fees = 500801282051;
	Acala::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(btc, &AccountId::from(BOB)),
			549198717948718
		);
		/*assert_eq!(
			hydradx_runtime::Tokens::free_balance(aca, &hydradx_runtime::Treasury::account_id()),
			fees
		);*/
	});
}
/// Returns amount if `asset` is fungible, or zero.
fn fungible_amount(asset: &MultiAsset) -> u128 {
	if let Fungible(amount) = &asset.fun {
		*amount
	} else {
		Zero::zero()
	}
}

fn half(asset: &MultiAsset) -> MultiAsset {
	let half_amount = fungible_amount(asset)
		.checked_div(2)
		.expect("div 2 can't overflow; qed");
	MultiAsset {
		fun: Fungible(half_amount),
		id: asset.id,
	}
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
