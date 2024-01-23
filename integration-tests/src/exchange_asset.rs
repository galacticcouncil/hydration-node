#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::fungible::Balanced;
use frame_support::traits::tokens::Precision;
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

pub const ACA: u32 = 1234;
pub const GLMR: u32 = 4567;
pub const IBTC: u32 = 7890;

//TODO: Unignore these tests when we have the AssetExchange feature configured in hydra.
//For now they are ignored as first we want to try out AssetExchange in basilisk

#[ignore]
#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_sell() {
	//Arrange
	TestNet::reset();

	let mut price = None;
	Hydra::execute_with(|| {
		register_aca();

		add_currency_price(ACA, FixedU128::from(1));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(ACA, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
		use hydradx_traits::pools::SpotPriceProvider;
		price = hydradx_runtime::Omnipool::spot_price(CORE_ASSET_ID, ACA);
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
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 100 * UNITS
		);

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
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			50 * UNITS - fees
		);
		// We receive about 39_101 HDX (HDX is super cheap in our test)
		let received = 39_101 * UNITS + BOB_INITIAL_NATIVE_BALANCE + 207_131_554_396;
		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)), received);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}

#[ignore]
#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_buy() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		register_aca();

		add_currency_price(ACA, FixedU128::from(1));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(ACA, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
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
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 100 * UNITS
		);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	let fees = 500801282051;
	let swapped = 361693915942; // HDX is super cheap in our setup
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			100 * UNITS - swapped - fees
		);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}

//We swap GLMR for iBTC, sent from ACALA and executed on Hydradx, resultin in 4 hops
#[ignore]
#[test]
fn transfer_and_swap_should_work_with_4_hops() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		register_glmr();
		register_ibtc();

		add_currency_price(GLMR, FixedU128::from(1));

		init_omnipool();
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

		let token_price = FixedU128::from_float(1.0);
		assert_ok!(hydradx_runtime::Tokens::deposit(GLMR, &omnipool_account, 3000 * UNITS));
		assert_ok!(hydradx_runtime::Tokens::deposit(IBTC, &omnipool_account, 3000 * UNITS));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			GLMR,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			IBTC,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));
	});

	Moonbeam::execute_with(|| {
		use xcm_executor::traits::ConvertLocation;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(ACALA_PARA_ID)).into()).unwrap();
		let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
			.expect("Failed to deposit");
	});

	Interlay::execute_with(|| {
		use xcm_executor::traits::ConvertLocation;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(HYDRA_PARA_ID)).into()).unwrap();
		let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
			.expect("Failed to deposit");
	});

	Acala::execute_with(|| {
		register_glmr();
		register_ibtc();

		add_currency_price(IBTC, FixedU128::from(1));

		let alice_init_moon_balance = 3000 * UNITS;
		assert_ok!(hydradx_runtime::Tokens::deposit(
			GLMR,
			&ALICE.into(),
			alice_init_moon_balance
		));

		//Act
		let give_amount = 1000 * UNITS;
		let give = MultiAsset::from((hydradx_runtime::CurrencyIdConvert::convert(GLMR).unwrap(), give_amount));
		let want = MultiAsset::from((hydradx_runtime::CurrencyIdConvert::convert(IBTC).unwrap(), 550 * UNITS));

		let xcm = craft_transfer_and_swap_xcm_with_4_hops::<hydradx_runtime::RuntimeCall>(give, want, SELL);
		assert_ok!(hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_parts(399_600_000_000, 0),
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(GLMR, &AccountId::from(ALICE)),
			alice_init_moon_balance - give_amount
		);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	let fees = 400641025641;
	Acala::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(IBTC, &AccountId::from(BOB)),
			549198717948718
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(IBTC, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}

fn register_glmr() {
	assert_ok!(hydradx_runtime::AssetRegistry::register(
		hydradx_runtime::RuntimeOrigin::root(),
		b"GLRM".to_vec(),
		pallet_asset_registry::AssetType::Token,
		1_000_000,
		Some(GLMR),
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(0))
		))),
		None
	));
}

fn register_aca() {
	assert_ok!(hydradx_runtime::AssetRegistry::register(
		hydradx_runtime::RuntimeOrigin::root(),
		b"ACAL".to_vec(),
		pallet_asset_registry::AssetType::Token,
		1_000_000,
		Some(ACA),
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))
		))),
		None
	));
}

fn register_ibtc() {
	assert_ok!(hydradx_runtime::AssetRegistry::register(
		hydradx_runtime::RuntimeOrigin::root(),
		b"iBTC".to_vec(),
		pallet_asset_registry::AssetType::Token,
		1_000_000,
		Some(IBTC),
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Parachain(INTERLAY_PARA_ID), GeneralIndex(0))
		))),
		None
	));
}

fn add_currency_price(asset_id: u32, price: FixedU128) {
	assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
		hydradx_runtime::RuntimeOrigin::root(),
		asset_id,
		price,
	));

	// make sure the price is propagated
	hydradx_runtime::MultiTransactionPayment::on_initialize(hydradx_runtime::System::block_number());
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

fn craft_transfer_and_swap_xcm_with_4_hops<RC: Decode + GetDispatchInfo>(
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
	let give: MultiAssetFilter = Definite(give.into());
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
		.reanchored(&dest, want_reserve_chain.interior)
		.expect("should reanchor");

	let weight_limit = {
		let fees = fees.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets),
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
			give,
			want: want.clone(),
			maximal: is_sell,
		},
		InitiateReserveWithdraw {
			assets: want.into(),
			reserve: want_reserve_chain,
			xcm: Xcm(vec![
				//Executed on interlay
				BuyExecution {
					fees: half(&reserve_fees),
					weight_limit: weight_limit.clone(),
				},
				DepositReserveAsset {
					assets: Wild(AllCounted(max_assets)),
					dest,
					xcm: Xcm(vec![
						//Executed on acala
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
		WithdrawAsset(give_asset.into()),
		InitiateReserveWithdraw {
			assets: All.into(),
			reserve: give_reserve_chain,
			xcm: Xcm(vec![
				//Executed on moonbeam
				BuyExecution {
					fees: half(&give_reserve_fees),
					weight_limit,
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
