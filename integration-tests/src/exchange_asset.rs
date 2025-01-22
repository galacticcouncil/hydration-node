#![cfg(test)]

use crate::assert_operation_stack;
use crate::polkadot_test_net::*;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Balanced;
use frame_support::traits::tokens::Precision;
use frame_support::weights::Weight;
use frame_support::{assert_ok, pallet_prelude::*};
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Router;
use hydradx_runtime::RuntimeOrigin;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::currency::MultiCurrency;
use polkadot_xcm::opaque::v3::{Junction, Junctions::X2, MultiLocation};
use polkadot_xcm::{v4::prelude::*, VersionedXcm};
use pretty_assertions::assert_eq;
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::AccountId;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::DispatchResult;
use sp_runtime::{FixedU128, Permill, TransactionOutcome};
use sp_std::sync::Arc;
use xcm_emulator::TestExt;

pub const SELL: bool = true;
pub const BUY: bool = false;

pub const ACA: u32 = 1234;
pub const GLMR: u32 = 4567;
pub const IBTC: u32 = 7890;
pub const ZTG: u32 = 5001;

pub const HDX_ON_OTHER_PARACHAIN: u32 = 5002;

#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_sell() {
	//Arrange
	TestNet::reset();

	let mut price = None;
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
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

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	Acala::execute_with(|| {
		let give = Asset::from((
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X2(Arc::new([
					cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				])),
			),
			50 * UNITS,
		));

		let want = Asset::from((
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X2(Arc::new([
					cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				])),
			),
			300 * UNITS,
		));

		let xcm = craft_exchange_asset_xcm::<hydradx_runtime::RuntimeCall>(give, want, SELL);
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

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "treasury should have received fees");
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			50 * UNITS - fee
		);
		// We receive about 39_101 HDX (HDX is super cheap in our test)
		let received = 39_101 * UNITS + BOB_INITIAL_NATIVE_BALANCE + 207_131_554_396;
		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)), received);

		let last_swapped_events: Vec<pallet_broadcast::Event<hydradx_runtime::Runtime>> = get_last_swapped_events();
		let last_two_swapped_events = &last_swapped_events[last_swapped_events.len() - 2..];

		let event1 = &last_two_swapped_events[0];
		assert_operation_stack!(
			event1,
			[
				ExecutionType::Xcm(_, 0),
				ExecutionType::XcmExchange(1),
				ExecutionType::Router(2),
				ExecutionType::Omnipool(3)
			]
		);

		let event2 = &last_two_swapped_events[0];
		assert_operation_stack!(
			event2,
			[
				ExecutionType::Xcm(_, 0),
				ExecutionType::XcmExchange(1),
				ExecutionType::Router(2),
				ExecutionType::Omnipool(3)
			]
		);

		//We assert that another trade doesnt have the xcm exchange type on stack
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			ACA,
			UNITS,
			0,
			vec![],
		));

		let last_swapped_events: Vec<pallet_broadcast::Event<hydradx_runtime::Runtime>> = get_last_swapped_events();
		let last_two_swapped_events = &last_swapped_events[last_swapped_events.len() - 2..];

		let event1 = &last_two_swapped_events[0];
		assert_operation_stack!(event1, [ExecutionType::Router(4), ExecutionType::Omnipool(5)]);

		let event2 = &last_two_swapped_events[0];
		assert_operation_stack!(event2, [ExecutionType::Router(4), ExecutionType::Omnipool(5)]);
	});
}

#[test]
fn hydra_should_swap_assets_when_receiving_from_acala_with_buy() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
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

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	let amount_out = 300 * UNITS;
	Acala::execute_with(|| {
		let give = Asset::from((
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X2(Arc::new([
					cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				])),
			),
			50 * UNITS,
		));

		let want = Asset::from((
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X2(Arc::new([
					cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				])),
			),
			amount_out,
		));

		let xcm = craft_exchange_asset_xcm::<hydradx_runtime::RuntimeCall>(give, want, BUY);
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

	Hydra::execute_with(|| {
		let fees = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fees > 0, "treasury should have received fees");
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + amount_out
		);
	});
}

//We swap GLMR for iBTC, sent from ACALA and executed on Hydradx, resultin in 4 hops
#[test]
fn transfer_and_swap_should_work_with_4_hops() {
	//Arrange
	TestNet::reset();

	let bob_init_ibtc_balance = 0;

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
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
			set_zero_reward_for_referrals(GLMR);
			set_zero_reward_for_referrals(IBTC);
			set_zero_reward_for_referrals(ACA);
			hydradx_run_to_block(3);

			assert_eq!(
				hydradx_runtime::Currencies::free_balance(IBTC, &AccountId::from(BOB)),
				bob_init_ibtc_balance
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	Moonbeam::execute_with(|| {
		set_zero_reward_for_referrals(ACA);

		use xcm_executor::traits::ConvertLocation;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(ACALA_PARA_ID)).into()).unwrap();
		let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
			.expect("Failed to deposit");
	});

	Interlay::execute_with(|| {
		set_zero_reward_for_referrals(IBTC);

		use xcm_executor::traits::ConvertLocation;
		let para_account =
			hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(HYDRA_PARA_ID)).into()).unwrap();
		let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
			.expect("Failed to deposit");
	});

	Acala::execute_with(|| {
		let _ = with_transaction(|| {
			register_glmr();
			register_ibtc();
			set_zero_reward_for_referrals(GLMR);
			set_zero_reward_for_referrals(IBTC);
			set_zero_reward_for_referrals(ACA);

			add_currency_price(IBTC, FixedU128::from(1));

			let alice_init_moon_balance = 3000 * UNITS;
			assert_ok!(hydradx_runtime::Tokens::deposit(
				GLMR,
				&ALICE.into(),
				alice_init_moon_balance
			));

			//Act
			let give_amount = 1000 * UNITS;
			let give = Asset::from((hydradx_runtime::CurrencyIdConvert::convert(GLMR).unwrap(), give_amount));
			let want = Asset::from((hydradx_runtime::CurrencyIdConvert::convert(IBTC).unwrap(), 550 * UNITS));

			let xcm = craft_transfer_and_swap_xcm_with_4_hops::<hydradx_runtime::RuntimeCall>(give, want, SELL);
			assert_ok!(hydradx_runtime::PolkadotXcm::execute(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				Box::new(xcm),
				Weight::from_parts(899_600_000_000, 0),
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
			hydradx_run_to_block(4);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	//We need these executions to trigger the processing of horizontal messages of each parachain
	Moonbeam::execute_with(|| {});
	Hydra::execute_with(|| {});
	Interlay::execute_with(|| {});

	Acala::execute_with(|| {
		let bob_new_ibtc_balance = hydradx_runtime::Currencies::free_balance(IBTC, &AccountId::from(BOB));

		assert!(
			bob_new_ibtc_balance > bob_init_ibtc_balance,
			"Bob should have received iBTC"
		);

		let fee = hydradx_runtime::Tokens::free_balance(IBTC, &hydradx_runtime::Treasury::account_id());

		assert!(fee > 0, "treasury should have received fees, but it didn't");
	});
}

pub mod zeitgeist_use_cases {
	use super::*;
	use frame_support::traits::tokens::Precision;
	use polkadot_xcm::latest::{NetworkId, Parent};
	use polkadot_xcm::prelude::Parachain;
	use std::sync::Arc;

	use primitives::constants::chain::CORE_ASSET_ID;

	#[test]
	fn remote_swap_sell_native_ztg_for_native_hdx_on_hydra() {
		//Register tokens and init omnipool on hydra
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_ztg();
				crate::exchange_asset::add_currency_price(crate::exchange_asset::ZTG, FixedU128::from(1));

				init_omnipool();
				let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

				let token_price = FixedU128::from_float(1.0);
				assert_ok!(hydradx_runtime::Tokens::deposit(
					ZTG,
					&omnipool_account,
					1000000 * UNITS
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					ZTG,
					token_price,
					Permill::from_percent(100),
					AccountId::from(BOB),
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		let alice_init_hxd_balance_on_zeitgeist = 0;

		//Construct and send XCM zeitgeist -> hydra
		Zeitgeist::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_hdx_in_sibling_chain();
				crate::exchange_asset::add_currency_price(HDX_ON_OTHER_PARACHAIN, FixedU128::from(1));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});

			pretty_assertions::assert_eq!(
				hydradx_runtime::Tokens::free_balance(HDX_ON_OTHER_PARACHAIN, &AccountId::from(ALICE)),
				alice_init_hxd_balance_on_zeitgeist
			);

			let give_reserve_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					ZEITGEIST_PARA_ID,
				)])),
			);
			let swap_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID,
				)])),
			);
			let want_reserve_chain = swap_chain.clone();
			let dest = give_reserve_chain.clone();

			let beneficiary = Location::new(
				0,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
					id: ALICE,
					network: None,
				}])),
			);
			let assets: Assets = Asset {
				id: cumulus_primitives_core::AssetId(Location::new(
					0,
					cumulus_primitives_core::Junctions::X1(Arc::new([
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				)),
				fun: Fungible(100 * UNITS),
			}
			.into();
			let max_assets = assets.len() as u32 + 1;

			let give_amount = 10 * UNITS;
			let give_asset = Asset::from((hydradx_runtime::CurrencyIdConvert::convert(0).unwrap(), give_amount));
			let want_asset = Asset::from((
				Location::new(
					1,
					cumulus_primitives_core::Junctions::X2(Arc::new([
						cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				),
				100 * UNITS,
			));

			let want: Assets = want_asset.clone().into();

			let fees = give_asset
				.clone()
				.reanchored(&swap_chain, &give_reserve_chain.interior)
				.expect("should reanchor");

			let destination_fee = want_asset
				.reanchored(&dest, &want_reserve_chain.interior)
				.expect("should reanchor");

			let weight_limit = Limited(Weight::from_parts(u64::MAX, u64::MAX));

			// executed on local (zeitgeist)
			let message = Xcm(vec![
				WithdrawAsset(give_asset.clone().into()),
				DepositReserveAsset {
					assets: AllCounted(max_assets).into(),
					dest: swap_chain,
					// executed on remote (on hydra)
					xcm: Xcm(vec![
						BuyExecution {
							fees: crate::exchange_asset::half(&fees),
							weight_limit: weight_limit.clone(),
						},
						ExchangeAsset {
							give: give_asset.into(),
							want: want.clone(),
							maximal: true,
						},
						DepositReserveAsset {
							assets: Wild(AllCounted(max_assets)),
							dest,
							xcm: Xcm(vec![
								//Executed on Zeitgeist
								BuyExecution {
									fees: crate::exchange_asset::half(&destination_fee),
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
			let xcm = VersionedXcm::from(message);

			assert_ok!(hydradx_runtime::PolkadotXcm::execute(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				Box::new(xcm),
				Weight::from_parts(899_600_000_000, 0),
			));

			//Assert
			pretty_assertions::assert_eq!(
				hydradx_runtime::Currencies::free_balance(CORE_ASSET_ID, &AccountId::from(ALICE)),
				1000 * UNITS - give_amount
			);

			assert!(matches!(
				last_hydra_events(2).first(),
				Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
					cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
				))
			));
		});

		//Trigger the processing of horizontal xcm messages
		Hydra::execute_with(|| {});

		//Assert that swap amount out is sent back to Zeitgeist
		Zeitgeist::execute_with(|| {
			let alice_new_hxd_balance_on_zeitgeist =
				hydradx_runtime::Tokens::free_balance(HDX_ON_OTHER_PARACHAIN, &AccountId::from(ALICE));
			assert!(
				alice_new_hxd_balance_on_zeitgeist > alice_init_hxd_balance_on_zeitgeist,
				"Alice should have received HDX"
			);
		});
	}

	#[test]
	fn remote_swap_sell_native_ztg_for_nonnative_ibtc_on_hydra() {
		//Register tokens and init omnipool on hydra
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_ztg();
				register_ibtc();
				crate::exchange_asset::add_currency_price(crate::exchange_asset::ZTG, FixedU128::from(1));

				init_omnipool();
				let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

				let token_price = FixedU128::from_float(1.0);
				assert_ok!(hydradx_runtime::Tokens::deposit(ZTG, &omnipool_account, 100000 * UNITS));
				assert_ok!(hydradx_runtime::Tokens::deposit(
					IBTC,
					&omnipool_account,
					100000 * UNITS
				));
				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					IBTC,
					token_price,
					Permill::from_percent(100),
					AccountId::from(BOB),
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					ZTG,
					token_price,
					Permill::from_percent(100),
					AccountId::from(BOB),
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Deposit IBTC reserve for hydra
		Interlay::execute_with(|| {
			//set_zero_reward_for_referrals(IBTC);
			use xcm_executor::traits::ConvertLocation;
			let para_account =
				hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(HYDRA_PARA_ID)).into())
					.unwrap();
			let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
				.expect("Failed to deposit");
		});

		let alice_init_ibtc_balance_on_zeitgeist = 0;
		//Construct and send XCM zeitgeist -> hydra
		Zeitgeist::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_hdx_in_sibling_chain();
				register_ibtc();
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});

			crate::exchange_asset::add_currency_price(HDX_ON_OTHER_PARACHAIN, FixedU128::from(1));
			crate::exchange_asset::add_currency_price(IBTC, FixedU128::from(1));

			pretty_assertions::assert_eq!(
				hydradx_runtime::Tokens::free_balance(IBTC, &AccountId::from(ALICE)),
				alice_init_ibtc_balance_on_zeitgeist
			);

			let give_reserve_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					ZEITGEIST_PARA_ID,
				)])),
			);
			let swap_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID,
				)])),
			);
			let want_reserve_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					INTERLAY_PARA_ID,
				)])),
			);
			let dest = give_reserve_chain.clone();

			let beneficiary = Location::new(
				0,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
					id: ALICE,
					network: None,
				}])),
			);
			let assets: Assets = Asset {
				id: cumulus_primitives_core::AssetId(Location::new(
					0,
					cumulus_primitives_core::Junctions::X1(Arc::new([
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				)),
				fun: Fungible(10 * UNITS),
			}
			.into();
			let max_assets = assets.len() as u32 + 1;

			let give_amount = 100 * UNITS;
			let give_asset = Asset::from((hydradx_runtime::CurrencyIdConvert::convert(0).unwrap(), give_amount));
			let want_asset = Asset::from((
				Location::new(
					1,
					cumulus_primitives_core::Junctions::X2(Arc::new([
						cumulus_primitives_core::Junction::Parachain(INTERLAY_PARA_ID),
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				),
				10 * UNITS,
			));

			let want: Assets = want_asset.clone().into();

			let fees = give_asset
				.clone()
				.reanchored(&swap_chain, &give_reserve_chain.interior)
				.expect("should reanchor");

			let destination_fee = want_asset
				.clone()
				.reanchored(&dest, &want_reserve_chain.interior)
				.expect("should reanchor");

			let reserve_fees = want_asset
				.clone()
				.reanchored(&want_reserve_chain, &swap_chain.interior)
				.expect("should reanchor");

			let weight_limit = Limited(Weight::from_parts(u64::MAX, u64::MAX));

			// executed on local (zeitgeist)
			let message = Xcm(vec![
				WithdrawAsset(give_asset.clone().into()),
				DepositReserveAsset {
					assets: AllCounted(max_assets).into(),
					dest: swap_chain,
					// executed on remote (on hydra)
					xcm: Xcm(vec![
						BuyExecution {
							fees: crate::exchange_asset::half(&fees),
							weight_limit: weight_limit.clone(),
						},
						ExchangeAsset {
							give: give_asset.into(),
							want: want.clone(),
							maximal: true,
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
					]),
				},
			]);
			let xcm = VersionedXcm::from(message);

			assert_ok!(hydradx_runtime::PolkadotXcm::execute(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				Box::new(xcm),
				Weight::from_parts(899_600_000_000, 0),
			));

			//Assert
			pretty_assertions::assert_eq!(
				hydradx_runtime::Currencies::free_balance(CORE_ASSET_ID, &AccountId::from(ALICE)),
				1000 * UNITS - give_amount
			);

			assert!(matches!(
				last_hydra_events(2).first(),
				Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
					cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
				))
			));
		});

		//Trigger the processing of horizontal xcm messages
		Hydra::execute_with(|| {});
		Interlay::execute_with(|| {});

		//Assert that swap amount out of IBTC is sent back to Zeitgeist
		Zeitgeist::execute_with(|| {
			let alice_new_ibtc_balance_on_zeitgeist =
				hydradx_runtime::Tokens::free_balance(IBTC, &AccountId::from(ALICE));
			assert!(
				alice_new_ibtc_balance_on_zeitgeist > alice_init_ibtc_balance_on_zeitgeist,
				"Alice should have received iBTC"
			);
		});
	}

	#[test]
	fn remote_swap_sell_nonnative_glmr_for_nonnative_ibtc_on_hydra() {
		let alice_init_ibtc_balance_on_zeitgeist = 0;
		//Register tokens and init omnipool on hydra
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_ztg();

				register_ibtc();
				crate::exchange_asset::add_currency_price(crate::exchange_asset::ZTG, FixedU128::from(1));

				register_glmr();
				crate::exchange_asset::add_currency_price(crate::exchange_asset::GLMR, FixedU128::from(1));

				init_omnipool();
				let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

				let token_price = FixedU128::from_float(1.0);
				assert_ok!(hydradx_runtime::Tokens::deposit(
					GLMR,
					&omnipool_account,
					100000 * UNITS
				));
				assert_ok!(hydradx_runtime::Tokens::deposit(
					IBTC,
					&omnipool_account,
					100000 * UNITS
				));
				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					IBTC,
					token_price,
					Permill::from_percent(100),
					AccountId::from(BOB),
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					GLMR,
					token_price,
					Permill::from_percent(100),
					AccountId::from(BOB),
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Deposit IBTC reserve for hydra
		Interlay::execute_with(|| {
			//set_zero_reward_for_referrals(IBTC);
			use xcm_executor::traits::ConvertLocation;
			let para_account =
				hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(HYDRA_PARA_ID)).into())
					.unwrap();
			let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
				.expect("Failed to deposit");
		});

		//Deposit GLMR reserve for Zeitgeist
		Moonbeam::execute_with(|| {
			use xcm_executor::traits::ConvertLocation;
			let para_account =
				hydradx_runtime::LocationToAccountId::convert_location(&(Parent, Parachain(ZEITGEIST_PARA_ID)).into())
					.unwrap();
			let _ = hydradx_runtime::Balances::deposit(&para_account, 1000 * UNITS, Precision::Exact)
				.expect("Failed to deposit");
		});

		//Construct and send XCM zeitgeist -> hydra
		Zeitgeist::execute_with(|| {
			let _ = with_transaction(|| {
				crate::exchange_asset::register_hdx_in_sibling_chain();
				register_ibtc();
				register_glmr();
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});

			crate::exchange_asset::add_currency_price(IBTC, FixedU128::from(1));
			crate::exchange_asset::add_currency_price(GLMR, FixedU128::from(1));
			let alice_init_glmr_balance = 3000 * UNITS;
			assert_ok!(hydradx_runtime::Tokens::deposit(
				GLMR,
				&ALICE.into(),
				alice_init_glmr_balance
			));

			let give_reserve_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					MOONBEAM_PARA_ID,
				)])),
			);
			let swap_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID,
				)])),
			);
			let want_reserve_chain = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					INTERLAY_PARA_ID,
				)])),
			);
			let dest = Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					ZEITGEIST_PARA_ID,
				)])),
			);

			let beneficiary = Location::new(
				0,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
					id: ALICE,
					network: None,
				}])),
			);
			let assets: Assets = Asset {
				id: cumulus_primitives_core::AssetId(Location::new(
					0,
					cumulus_primitives_core::Junctions::X1(Arc::new([
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				)),
				fun: Fungible(10 * UNITS),
			}
			.into();
			let max_assets = assets.len() as u32 + 1;

			let give_amount = 100 * UNITS;
			let give_asset = Asset::from((
				Location::new(
					1,
					cumulus_primitives_core::Junctions::X2(Arc::new([
						cumulus_primitives_core::Junction::Parachain(MOONBEAM_PARA_ID),
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				),
				give_amount,
			));
			let want_asset = Asset::from((
				Location::new(
					1,
					cumulus_primitives_core::Junctions::X2(Arc::new([
						cumulus_primitives_core::Junction::Parachain(INTERLAY_PARA_ID),
						cumulus_primitives_core::Junction::GeneralIndex(0),
					])),
				),
				10 * UNITS,
			));

			let want: Assets = want_asset.clone().into();

			let fees = give_asset
				.clone()
				.reanchored(&swap_chain, &give_reserve_chain.interior)
				.expect("should reanchor");

			let destination_fee = want_asset
				.clone()
				.reanchored(&dest, &want_reserve_chain.interior)
				.expect("should reanchor");

			let origin_context = cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::GlobalConsensus(NetworkId::Polkadot),
				cumulus_primitives_core::Junction::Parachain(ZEITGEIST_PARA_ID),
			]));
			let give_reserve_fees = give_asset
				.clone()
				.reanchored(&give_reserve_chain, &origin_context)
				.expect("should reanchor");

			let reserve_fees = want_asset
				.clone()
				.reanchored(&want_reserve_chain, &swap_chain.interior)
				.expect("should reanchor");

			let weight_limit = Limited(Weight::from_parts(u64::MAX, u64::MAX));

			// executed on local (zeitgeist)
			let message = Xcm(vec![
				WithdrawAsset(give_asset.clone().into()),
				InitiateReserveWithdraw {
					assets: All.into(),
					reserve: give_reserve_chain,
					xcm: Xcm(vec![
						//Executed on moonbeam
						BuyExecution {
							fees: half(&give_reserve_fees),
							weight_limit: weight_limit.clone(),
						},
						DepositReserveAsset {
							assets: AllCounted(max_assets).into(),
							dest: swap_chain,
							// executed on remote (on hydra)
							xcm: Xcm(vec![
								BuyExecution {
									fees: crate::exchange_asset::half(&fees),
									weight_limit: weight_limit.clone(),
								},
								ExchangeAsset {
									give: give_asset.into(),
									want: want.clone(),
									maximal: true,
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
												//Executed on zetigeist
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
							]),
						},
					]),
				},
			]);
			let xcm = VersionedXcm::from(message);

			assert_ok!(hydradx_runtime::PolkadotXcm::execute(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				Box::new(xcm),
				Weight::from_parts(899_600_000_000, 0),
			));

			//Assert
			pretty_assertions::assert_eq!(
				hydradx_runtime::Currencies::free_balance(GLMR, &AccountId::from(ALICE)),
				alice_init_glmr_balance - give_amount
			);

			assert!(matches!(
				last_hydra_events(2).first(),
				Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
					cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
				))
			));

			pretty_assertions::assert_eq!(
				hydradx_runtime::Tokens::free_balance(IBTC, &AccountId::from(ALICE)),
				alice_init_ibtc_balance_on_zeitgeist
			);
		});

		//Trigger the processing of horizontal xcm messages
		Moonbeam::execute_with(|| {});
		Hydra::execute_with(|| {});
		Interlay::execute_with(|| {});

		//Assert that swap amount out of IBTC is sent back to Zeitgeist
		Zeitgeist::execute_with(|| {
			let alice_new_ibtc_balance = hydradx_runtime::Tokens::free_balance(IBTC, &AccountId::from(ALICE));
			assert!(
				alice_new_ibtc_balance > alice_init_ibtc_balance_on_zeitgeist,
				"Alice should have received iBTC"
			);
		});
	}
}

fn register_glmr() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(GLMR),
		Some(b"GLRM".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Junction::Parachain(MOONBEAM_PARA_ID), Junction::GeneralIndex(0))
		))),
		None,
	));
}

fn register_aca() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(ACA),
		Some(b"ACAL".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
		))),
		None,
	));
}

fn register_ibtc() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(IBTC),
		Some(b"iBTC".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Junction::Parachain(INTERLAY_PARA_ID), Junction::GeneralIndex(0))
		))),
		None,
	));
}

fn register_ztg() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(ZTG),
		Some(b"ZTG".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Junction::Parachain(ZEITGEIST_PARA_ID), Junction::GeneralIndex(0))
		))),
		None,
	));
}

fn register_hdx_in_sibling_chain() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(HDX_ON_OTHER_PARACHAIN),
		Some(b"vHDX".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			X2(Junction::Parachain(HYDRA_PARA_ID), Junction::GeneralIndex(0))
		))),
		None,
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
fn fungible_amount(asset: &Asset) -> u128 {
	if let Fungible(amount) = &asset.fun {
		*amount
	} else {
		Zero::zero()
	}
}

fn half(asset: &Asset) -> Asset {
	let half_amount = fungible_amount(asset)
		.checked_div(2)
		.expect("div 2 can't overflow; qed");
	Asset {
		fun: Fungible(half_amount),
		id: asset.clone().id,
	}
}
use pallet_broadcast::types::ExecutionType;
use rococo_runtime::xcm_config::BaseXcmWeight;
use xcm_builder::FixedWeightBounds;
use xcm_executor::traits::WeightBounds;

fn craft_transfer_and_swap_xcm_with_4_hops<RC: Decode + GetDispatchInfo>(
	give_asset: Asset,
	want_asset: Asset,
	is_sell: bool,
) -> VersionedXcm<RC> {
	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let give_reserve_chain = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
			MOONBEAM_PARA_ID,
		)])),
	);
	let want_reserve_chain = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
			INTERLAY_PARA_ID,
		)])),
	);
	let swap_chain = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)])),
	);
	let dest = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID)])),
	);
	let beneficiary = Location::new(
		0,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
			id: BOB,
			network: None,
		}])),
	);
	let assets: Assets = Asset {
		id: cumulus_primitives_core::AssetId(Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GeneralIndex(0)])),
		)),
		fun: Fungible(100 * UNITS),
	}
	.into();
	let max_assets = assets.len() as u32 + 1;
	let origin_context = cumulus_primitives_core::Junctions::X2(Arc::new([
		cumulus_primitives_core::Junction::GlobalConsensus(NetworkId::Polkadot),
		cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
	]));
	let give = give_asset
		.clone()
		.reanchored(&dest, &origin_context)
		.expect("should reanchor give");
	let give: AssetFilter = Definite(give.into());
	let want: Assets = want_asset.clone().into();

	let fees = give_asset
		.clone()
		.reanchored(&swap_chain, &give_reserve_chain.interior)
		.expect("should reanchor");

	let reserve_fees = want_asset
		.clone()
		.reanchored(&want_reserve_chain, &swap_chain.interior)
		.expect("should reanchor");

	let destination_fee = want_asset
		.reanchored(&dest, &want_reserve_chain.interior)
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
				reserve: want_reserve_chain.clone(),
				xcm: Xcm(vec![
					BuyExecution {
						fees: reserve_fees.clone(), //reserve fee
						weight_limit: Limited(Weight::zero()),
					},
					DepositReserveAsset {
						assets: Wild(AllCounted(max_assets)),
						dest: dest.clone(),
						xcm: Xcm(vec![
							BuyExecution {
								fees: destination_fee.clone(), //destination fee
								weight_limit: Limited(Weight::zero()),
							},
							DepositAsset {
								assets: Wild(AllCounted(max_assets)),
								beneficiary: beneficiary.clone(),
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
		.reanchored(&give_reserve_chain, &origin_context)
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
	VersionedXcm::from(message)
}

fn craft_exchange_asset_xcm<RC: Decode + GetDispatchInfo>(give: Asset, want: Asset, is_sell: bool) -> VersionedXcm<RC> {
	let dest = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)])),
	);
	let beneficiary = Location::new(
		0,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
			id: BOB,
			network: None,
		}])),
	);
	let assets: Assets = Asset {
		id: cumulus_primitives_core::AssetId(Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GeneralIndex(0)])),
		)),
		fun: Fungible(100 * UNITS),
	}
	.into();
	let max_assets = assets.len() as u32 + 1;
	let context = cumulus_primitives_core::Junctions::X2(Arc::new([
		cumulus_primitives_core::Junction::GlobalConsensus(NetworkId::Polkadot),
		cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
	]));
	let fees = assets
		.get(0)
		.expect("should have at least 1 asset")
		.clone()
		.reanchored(&dest, &context)
		.expect("should reanchor");
	let give: AssetFilter = Definite(give.into());
	let want = want.into();
	let weight_limit = Limited(Weight::from_parts(u64::MAX, u64::MAX));

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
	VersionedXcm::from(message)
}
