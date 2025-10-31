#![cfg(test)]

use crate::dca::create_schedule;
use crate::dca::schedule_fake_with_sell_order;
use crate::liquidation::supply;
use crate::polkadot_test_net::*;
use crate::utils::accounts::*;
use frame_support::assert_ok;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::pallet_prelude::ValidateUnsigned;
use frame_support::storage::with_transaction;
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, BoundedVec};
use hex_literal::hex;
use hydradx_runtime::evm::aave_trade_executor::AaveTradeExecutor;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::evm::precompiles::{CALLPERMIT, DISPATCH_ADDR};
use hydradx_runtime::evm::Erc20Currency;
use hydradx_runtime::{
	AssetId, Block, Currencies, EVMAccounts, Liquidation, MultiTransactionPayment, Omnipool, OriginCaller, Router,
	Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Treasury,
};
use hydradx_runtime::{AssetRegistry, Stableswap};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::router::ExecutorError;
use hydradx_traits::router::PoolType::{Aave, XYK};
use hydradx_traits::router::RouteProvider;
use hydradx_traits::router::Trade;
use hydradx_traits::router::{AssetPair, PoolType};
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use libsecp256k1::{sign, Message, SecretKey};
use orml_traits::MultiCurrency;
use pallet_asset_registry::Assets;
use pallet_broadcast::types::{Asset, ExecutionType};
use pallet_liquidation::BorrowingContract;
use pallet_route_executor::TradeExecution;
use pallet_transaction_multi_payment::EVMPermit;
use primitives::constants::currency::UNITS;
use primitives::Balance;
use sp_core::{H256, U256};
use sp_runtime::traits::Zero;
use sp_runtime::transaction_validity::{TransactionSource, ValidTransaction};
use sp_runtime::DispatchError;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;

pub const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT";
const RUNTIME_API_CALLER: EvmAddress = sp_core::H160(hex!("82db570265c37be24caf5bc943428a6848c3e9a6"));

pub fn with_aave(execution: impl FnOnce()) {
	with_aave_of_transaction_outcome(execution, TransactionOutcome::Commit(Ok::<(), DispatchError>(())))
}

// We need this for invariant tests, where we set up the base once (as it takes time to load snapshot),
// then not sharing state between prop test runs
pub fn with_aave_rollback(execution: impl FnOnce()) {
	with_aave_of_transaction_outcome(execution, TransactionOutcome::Rollback(Ok::<(), DispatchError>(())))
}

pub fn with_aave_of_transaction_outcome<T, U>(execution: impl FnOnce(), outcome: TransactionOutcome<Result<T, U>>)
where
	U: From<DispatchError>,
{
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		let b = hydradx_runtime::System::block_number();
		let hash = hydradx_runtime::System::block_hash(b);

		let pool_contract = liquidation_worker_support::MoneyMarketData::<
			Block,
			OriginCaller,
			RuntimeCall,
			RuntimeEvent,
		>::fetch_pool::<crate::liquidation::ApiProvider<Runtime>>(
			&crate::liquidation::ApiProvider::<Runtime>(Runtime),
			hash,
			pap_contract,
			RUNTIME_API_CALLER,
		)
		.unwrap();
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), 10 * BAG));

		let _ = with_transaction(|| {
			execution();
			outcome
		});
	});
}

#[test]
fn transfer_all() {
	with_stablepool(|pool| {
		// Get some ADOT to run the POC because we have 0 right now
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			10000,
			10000 + 2,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));

		// Starting with only 10000 weis of ADOT (it can be any amount as long as it is > ed)
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 10000);

		// Deposit these 10000 ADOT and get back any amount of shares you want for free
		assert_eq!(
			Stableswap::add_liquidity_shares(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				pool,
				100000 * BAG,
				// aTOKEN
				ADOT,
				//max_asset_amount
				u128::MAX - 1u128,
			),
			Err(Other(
				"evm:0x4e487b710000000000000000000000000000000000000000000000000000000000000011"
			))
		);
	});
}

pub fn with_atoken(execution: impl FnOnce()) {
	with_aave(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			BAG,
			BAG + 2, //Tiny we charge due token-atoken is not always 1:1,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		execution();
	})
}

pub fn with_atoken_rollback(execution: impl FnOnce()) {
	with_aave_rollback(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			BAG,
			BAG + 2, //Tiny we charge due token-atoken is not always 1:1,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		execution();
	})
}

fn with_stablepool(execution: impl FnOnce(AssetId)) {
	with_atoken(|| {
		let pool = AssetRegistry::register_sufficient_asset(
			None,
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetKind::StableSwap,
			Zero::zero(),
			None,
			None,
			None,
			None,
		)
		.unwrap();

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool,
			BoundedVec::truncate_from([DOT, ADOT].to_vec()),
			amplification,
			fee,
		));

		assert_ok!(Stableswap::add_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pool,
			BoundedVec::truncate_from(vec![
				AssetAmount {
					asset_id: DOT,
					amount: BAG,
				},
				AssetAmount {
					asset_id: ADOT,
					amount: BAG,
				},
			]),
		));

		execution(pool);
	});
}

const HDX: AssetId = 0;
const DAI: AssetId = 1;
pub const DOT: AssetId = 5;
pub const ADOT: AssetId = 1_000_037;
const ONE: u128 = 10_u128.pow(10);
pub const BAG: u128 = 100000 * ONE;

#[test]
fn nice_borrowing_contract_is_used() {
	with_aave(|| {
		let pool_address = EvmAddress::from_slice(hex!("f550bcd9b766843d72fc4c809a839633fd09b643").as_slice());
		assert_eq!(<BorrowingContract<Runtime>>::get(), pool_address)
	})
}

#[test]
fn adot_is_registered() {
	with_aave(|| assert!(<Assets<Runtime>>::get(ADOT).is_some()))
}

#[test]
fn alice_can_supply() {
	with_aave(|| {
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));
		supply(
			EvmAddress::from_slice(hex!("f550bcd9b766843d72fc4c809a839633fd09b643").as_slice()),
			EVMAccounts::evm_address(&AccountId::from(ALICE)),
			HydraErc20Mapping::encode_evm_address(DOT),
			100 * 10_u128.pow(10),
		);
	})
}

#[test]
fn sell_dot() {
	with_aave(|| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), ONE);
	})
}

#[test]
fn buy_adot() {
	with_aave(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			ONE + 2, // Small fee we apply for buys,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), ONE);
	})
}

#[test]
fn sell_adot() {
	with_atoken(|| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ADOT,
			DOT,
			ONE,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: ADOT,
				asset_out: DOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), BAG - ONE);
	})
}
#[test]
fn buy_dot() {
	with_atoken(|| {
		hydradx_run_to_next_block();

		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ADOT,
			DOT,
			ONE,
			ONE + 2,
			vec![Trade {
				pool: Aave,
				asset_in: ADOT,
				asset_out: DOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), BAG - ONE - 2);

		let atoken = HydraErc20Mapping::asset_address(ADOT);
		let filler = pallet_evm_accounts::Pallet::<Runtime>::truncated_account_id(atoken);

		pretty_assertions::assert_eq!(
			*get_last_swapped_events().last().unwrap(),
			pallet_broadcast::Event::<Runtime>::Swapped3 {
				swapper: ALICE.into(),
				filler,
				filler_type: pallet_broadcast::types::Filler::AAVE,
				operation: pallet_broadcast::types::TradeOperation::ExactOut,
				inputs: vec![Asset::new(ADOT, ONE)],
				outputs: vec![Asset::new(DOT, ONE)],
				fees: vec![],
				operation_stack: vec![ExecutionType::Router(1)],
			}
		);
	})
}

#[test]
fn sell_adot_should_work_when_less_spent_due_to_aave_rounding() {
	with_atoken(|| {
		//State needs to be set up so rounding happens in aave contract
		hydradx_run_to_next_block();
		assert_ok!(Currencies::deposit(DOT, &BOB.into(), 6 * BAG));
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			DOT,
			ADOT,
			2 * BAG,
			2 * BAG + 2,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));

		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			BAG / 2,
			BAG / 2 + 2,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));

		hydradx_run_to_next_block();

		let amount = 384586145866073;
		let balance = Currencies::free_balance(ADOT, &ALICE.into());
		let dots = Currencies::free_balance(DOT, &ALICE.into());
		//Act and assert
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ADOT,
			DOT,
			amount,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: ADOT,
				asset_out: DOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), balance - amount + 1);
		assert_eq!(Currencies::free_balance(DOT, &ALICE.into()), dots + amount + 6);

		let atoken = HydraErc20Mapping::asset_address(ADOT);
		let filler = pallet_evm_accounts::Pallet::<Runtime>::truncated_account_id(atoken);

		pretty_assertions::assert_eq!(
			*get_last_swapped_events().last().unwrap(),
			pallet_broadcast::Event::<Runtime>::Swapped3 {
				swapper: ALICE.into(),
				filler,
				filler_type: pallet_broadcast::types::Filler::AAVE,
				operation: pallet_broadcast::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(ADOT, amount)],
				outputs: vec![Asset::new(DOT, amount)],
				fees: vec![],
				operation_stack: vec![ExecutionType::Router(3)],
			}
		);
	})
}

#[test]
fn sell_dot_should_work_when_more_asset_out_received_due_aave_contract_rounding() {
	with_aave(|| {
		let amount = 55108183363806;
		let balance = Currencies::free_balance(ADOT, &ALICE.into());
		let dots = Currencies::free_balance(DOT, &ALICE.into());
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			amount,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(DOT, &ALICE.into()), dots - amount);
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), amount + balance + 1);
	})
}

#[test]
fn not_always_rounding_shall_be_in_your_favor() {
	with_atoken(|| {
		let amount = 55108186;
		let balance = Currencies::free_balance(ADOT, &ALICE.into());
		let dots = Currencies::free_balance(DOT, &ALICE.into());
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			amount,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		assert_eq!(Currencies::free_balance(DOT, &ALICE.into()), dots - amount);
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), amount + balance + 1);
	})
}

#[test]
fn second_hop_should_have_enough_funds_to_swap() {
	with_atoken(|| {
		assert_ok!(Currencies::deposit(
			HDX,
			&hydradx_runtime::Treasury::account_id(),
			2 * BAG
		));
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 2 * BAG));
		assert_ok!(hydradx_runtime::XYK::create_pool(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			BAG,
			ADOT,
			BAG,
		));

		let amount = 55108186;

		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			DAI,
			amount,
			0,
			vec![
				Trade {
					pool: Aave,
					asset_in: DOT,
					asset_out: ADOT,
				},
				Trade {
					pool: XYK,
					asset_in: ADOT,
					asset_out: DAI,
				},
			]
			.try_into()
			.unwrap()
		));
	})
}

#[test]
fn second_hop_should_have_enough_funds_to_buy() {
	with_atoken(|| {
		assert_ok!(Currencies::deposit(
			HDX,
			&hydradx_runtime::Treasury::account_id(),
			2 * BAG
		));
		assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 2 * BAG));
		assert_ok!(hydradx_runtime::XYK::create_pool(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DAI,
			BAG,
			ADOT,
			BAG,
		));

		// poor mans fuzzer
		for i in 0..100 {
			let amount = 55108186 + i;
			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				DOT,
				DAI,
				amount,
				amount * 2,
				vec![
					Trade {
						pool: Aave,
						asset_in: DOT,
						asset_out: ADOT,
					},
					Trade {
						pool: XYK,
						asset_in: ADOT,
						asset_out: DAI,
					},
				]
				.try_into()
				.unwrap()
			));
		}
	})
}

#[test]
fn executor_ensures_that_out_asset_is_underlying() {
	with_atoken(|| {
		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				ADOT,
				HDX,
				ONE,
				0,
				vec![Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: HDX,
				}]
				.try_into()
				.unwrap()
			),
			Other("Asset mismatch: output asset must match aToken's underlying")
		);
	})
}

#[test]
fn executor_ensures_valid_asset_pair() {
	with_atoken(|| {
		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), BAG));
		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				1_000 * ONE,
				0,
				vec![Trade {
					pool: Aave,
					asset_in: HDX,
					asset_out: DOT,
				}]
				.try_into()
				.unwrap()
			),
			Other("Invalid asset pair")
		);
	})
}

#[test]
fn liquidity_depth_of_dot_is_higher_after_buying_atoken() {
	let mut original = 0;
	let mut after = 0;
	with_aave(|| {
		original = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, DOT, ADOT).unwrap();
	});
	with_atoken(|| {
		after = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, DOT, ADOT).unwrap();
	});
	assert!(original < after);
}

#[test]
fn liquidity_depth_of_adot_is_lower_after_buying_atoken() {
	let mut original = 0;
	let mut after = 0;
	with_aave(|| {
		original = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, ADOT, DOT).unwrap();
	});
	with_atoken(|| {
		after = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, ADOT, DOT).unwrap();
	});
	assert!(original > after);
}

#[test]
fn liquidity_depth_validates_tokens() {
	with_aave(|| {
		assert_eq!(
			AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, HDX, DOT),
			Err(ExecutorError::Error(
				"Asset mismatch: first asset atoken has to match second asset reserve".into()
			))
		);
	});
}

#[test]
fn router_should_set_on_chain_route() {
	with_aave(|| {
		let pair = AssetPair {
			asset_in: ADOT,
			asset_out: DOT,
		};
		let route = vec![Trade {
			pool: Aave,
			asset_in: ADOT,
			asset_out: DOT,
		}];
		assert_ok!(Router::set_route(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pair,
			route.clone().try_into().unwrap()
		));
		assert_eq!(Router::get_route(pair), route);
	})
}

#[test]
fn dca_schedule_buying_atokens_should_be_created() {
	with_aave(|| {
		create_schedule(
			ALICE,
			schedule_fake_with_sell_order(ALICE, Aave, 10 * ONE, DOT, ADOT, ONE),
		);
	})
}

#[test]
fn dca_schedule_selling_atokens_should_be_created() {
	with_aave(|| {
		assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			ADOT,
			FixedU128::from_rational(1, 100000),
		));
		hydradx_runtime::MultiTransactionPayment::on_initialize(0);
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			1000 * ONE,
			1000 * ONE + 2,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));
		create_schedule(
			ALICE,
			schedule_fake_with_sell_order(ALICE, Aave, 10 * ONE, ADOT, DOT, ONE),
		);
	})
}

#[test]
fn buy_adot_from_stablepool() {
	with_stablepool(|pool| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			Balance::MAX,
			vec![Trade {
				pool: PoolType::Stableswap(pool),
				asset_in: DOT,
				asset_out: ADOT,
			},]
			.try_into()
			.unwrap()
		));
	});
}

#[test]
fn sell_in_stable_after_rebase() {
	with_stablepool(|pool| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			0,
			vec![
				Trade {
					pool: Aave,
					asset_in: DOT,
					asset_out: ADOT,
				},
				Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(pool),
					asset_in: DOT,
					asset_out: ADOT,
				},
			]
			.try_into()
			.unwrap()
		));
	});
}

#[test]
fn buy_in_stable_after_rebase() {
	with_stablepool(|pool| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			Balance::MAX,
			vec![
				Trade {
					pool: Aave,
					asset_in: DOT,
					asset_out: ADOT,
				},
				Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(pool),
					asset_in: DOT,
					asset_out: ADOT,
				},
			]
			.try_into()
			.unwrap()
		));
	});
}

mod transfer_atoken {
	use super::*;
	use frame_support::assert_ok;
	#[test]
	fn transfer_almost_all_atoken_but_ed_should_leave_ed() {
		crate::aave_router::with_atoken(|| {
			let ed = 1000;
			AssetRegistry::update(
				hydradx_runtime::RuntimeOrigin::root(),
				crate::aave_router::ADOT,
				None,
				None,
				Some(ed),
				None,
				None,
				None,
				None,
				None,
			)
			.unwrap();

			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

			let alice_all_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());

			assert_eq!(alice_all_balance, 1000000000000000);
			let alice_dot_balance_before = 8999999999999998;
			assert_eq!(
				Currencies::free_balance(crate::aave_router::DOT, &ALICE.into()),
				alice_dot_balance_before
			);
			assert_eq!(Currencies::free_balance(crate::aave_router::DOT, &BOB.into()), 0);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				alice_all_balance - ed
			));
			let bob_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &BOB.into());
			assert_eq!(bob_new_balance, alice_all_balance - ed);

			let alice_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());
			assert_eq!(alice_new_balance, ed);

			assert_eq!(
				Currencies::free_balance(crate::aave_router::DOT, &ALICE.into()),
				alice_dot_balance_before
			);
			assert_eq!(Currencies::free_balance(crate::aave_router::DOT, &BOB.into()), 0);
		})
	}

	#[test]
	fn transfer_all_atoken_but_one_should_leave_one() {
		crate::aave_router::with_atoken(|| {
			let ed = 1000;
			AssetRegistry::update(
				hydradx_runtime::RuntimeOrigin::root(),
				crate::aave_router::ADOT,
				None,
				None,
				Some(ed),
				None,
				None,
				None,
				None,
				None,
			)
			.unwrap();

			assert_eq!(
				Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into()),
				1000000000000000
			);

			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

			let alice_all_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());

			assert_eq!(alice_all_balance, 1000000000000000);
			let alice_dot_balance_before = 8999999999999998;
			assert_eq!(
				Currencies::free_balance(crate::aave_router::DOT, &ALICE.into()),
				alice_dot_balance_before
			);
			assert_eq!(Currencies::free_balance(crate::aave_router::DOT, &BOB.into()), 0);

			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				alice_all_balance - 1
			));
			let bob_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &BOB.into());
			assert_eq!(bob_new_balance, alice_all_balance - 1);
			let alice_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());
			assert_eq!(1, alice_new_balance);

			assert_eq!(
				Currencies::free_balance(crate::aave_router::DOT, &ALICE.into()),
				alice_dot_balance_before
			);
			assert_eq!(Currencies::free_balance(crate::aave_router::DOT, &BOB.into()), 0);
		})
	}

	#[test]
	fn transfer_atoken_when_left_more_than_ed_should_transfer_specified_amount() {
		crate::aave_router::with_atoken(|| {
			let ed = 1000;
			AssetRegistry::update(
				hydradx_runtime::RuntimeOrigin::root(),
				crate::aave_router::ADOT,
				None,
				None,
				Some(ed),
				None,
				None,
				None,
				None,
				None,
			)
			.unwrap();

			let leftover = ed + 1;

			let alice_all_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());
			let adot_asset_id = HydraErc20Mapping::asset_address(crate::aave_router::ADOT);
			let amount = alice_all_balance - leftover;
			assert_ok!(<Erc20Currency<Runtime> as MultiCurrency<AccountId>>::transfer(
				adot_asset_id,
				&AccountId::from(ALICE),
				&AccountId::from(BOB),
				amount
			));
			let bob_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &BOB.into());

			let alice_new_balance = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());
			assert_eq!(leftover, alice_new_balance);
			assert_eq!(bob_new_balance, amount);
		})
	}

	#[test]
	fn transfer_some_specific_amount_leads_to_aave_rounding_issue() {
		TestNet::reset();

		crate::aave_router::with_atoken(|| {
			let start_balance: u128 = 1_000_000_000_000_000;

			let leftover = 737922657087018_u128;

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)));

			let alice_balance_before = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into());
			assert_eq!(alice_balance_before, start_balance, "Start balance is not as expected");

			// Transfer all but `ed` to BOB, leaving `ed` on ALICE → dust after ED=ed+1
			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				start_balance - leftover
			));

			assert_eq!(
				Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into()),
				leftover - 1
			);

			//Free balance leads to off-by-one due to Aave rounding issue
			assert_eq!(
				Currencies::free_balance(crate::aave_router::ADOT, &BOB.into()),
				start_balance - leftover
			);
		});
	}
}

pub mod stableswap_with_atoken {
	use super::*;
	use hydradx_runtime::{RuntimeOrigin, Stableswap};

	#[test]
	fn add_liquidity_shares_should_not_work_when_user_has_not_enough_atoken_balance() {
		crate::aave_router::with_atoken(|| {
			let ed = 1000;
			AssetRegistry::update(
				hydradx_runtime::RuntimeOrigin::root(),
				crate::aave_router::ADOT,
				None,
				None,
				Some(ed),
				None,
				None,
				None,
				None,
				None,
			)
			.unwrap();

			let (pool_id, _, _) = init_stableswap_with_atoken().unwrap();

			let alice_adot_balance = Currencies::free_balance(ADOT, &ALICE.into());
			assert_eq!(alice_adot_balance, 1001);

			//Should fail as alice has not enough asset to provide liquidity
			assert_noop!(
				Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(ALICE.into()),
					pool_id,
					666_000_000_000_000_000_000,
					ADOT,
					u128::MAX
				),
				Other("evm:0x4e487b710000000000000000000000000000000000000000000000000000000000000011")
			);
		})
	}
}

pub fn init_stableswap_with_atoken() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	//Add an asset
	let name: Vec<u8> = 10i32.to_ne_bytes().to_vec();
	let asset_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(name.try_into().unwrap()),
		AssetKind::Token,
		1u128,
		Some(b"xDUM".to_vec().try_into().unwrap()),
		Some(18u8),
		None,
		None,
	)?;

	asset_ids.push(asset_id);
	Currencies::update_balance(
		RuntimeOrigin::root(),
		AccountId::from(BOB),
		asset_id,
		initial_liquidity as i128,
	)?;
	initial.push(AssetAmount::new(asset_id, initial_liquidity));

	//Add atoken
	asset_ids.push(ADOT);
	let initial_adot_liquidity = Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into()) - 1001;
	//assert_eq!(initial_adot_liquidity, 999999999990000);
	initial.push(AssetAmount::new(ADOT, initial_adot_liquidity));
	Currencies::transfer(
		RuntimeOrigin::signed(ALICE.into()),
		AccountId::from(BOB),
		ADOT,
		initial_adot_liquidity,
	)?;
	assert_eq!(Currencies::free_balance(crate::aave_router::ADOT, &ALICE.into()), 1001);

	//
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(
		RuntimeOrigin::root(),
		pool_id,
		BoundedVec::truncate_from(asset_ids),
		amplification,
		fee,
	)?;

	Stableswap::add_liquidity(
		RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(initial),
	)?;

	Ok((pool_id, asset_in, asset_out))
}

#[test]
fn transfer_rounging_property_test() {
	with_aave(|| {
		//Make some atoken on alice account
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			BAG,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
			.try_into()
			.unwrap()
		));

		let alice_balance = Currencies::free_balance(ADOT, &ALICE.into());
		let bob_balance = Currencies::free_balance(ADOT, &BOB.into());

		//Transfer amount to bob, leading to rounding issue
		let amount = 55108183363806;
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			ADOT,
			amount
		));

		assert_eq!(Currencies::free_balance(ADOT, &BOB.into()), bob_balance + amount + 1);

		//Transfer back to alice the same amount
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			ALICE.into(),
			ADOT,
			bob_balance + amount + 1
		));

		//Alice should have the original balance back
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), alice_balance);
	})
}


use sp_runtime::codec::Encode;
#[test]
fn evm_permit_set_currency_dispatch_should_pay_evm_fee_in_atoken() {
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	with_atoken(|| {
		// ALICE has ADOT from with_atoken setup
		let fee_currency = ADOT;

		// Initialize omnipool and oracle

		// Transfer some ADOT from ALICE to the EVM user (alith)
		let adot_transfer_amount = BAG / 3; // Transfer 1 BAG of ADOT
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			alith_evm_account(),
			ADOT,
			adot_transfer_amount
		));

		// Send adot to protocol account so we can add it to ominpool
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			ADOT,
			FixedU128::from_rational(1, 2)
		));

		set_ed(ADOT, 1);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hydradx_runtime::Omnipool::protocol_account()
			)));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			hydradx_runtime::Omnipool::protocol_account(),
			ADOT,
			adot_transfer_amount
		));

		// // Add ADOT to omnipool so fee payment can work
		assert_ok!(Omnipool::add_token(
			RuntimeOrigin::root(),
			ADOT,
			FixedU128::from_rational(1, 2),
			Permill::from_percent(100),
			AccountId::from(ALICE),
		));

		// Do a small trade to populate the oracle for ADOT
		// assert_ok!(Omnipool::sell(
		// 	RuntimeOrigin::signed(ALICE.into()),
		// 	ADOT,
		// 	HDX,
		// 	BAG / 100,
		// 	Balance::MIN
		// ));

		hydradx_run_to_next_block();

		pallet_transaction_payment::pallet::NextFeeMultiplier::<Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		//Let's mutate timestamp to accrue some yield on ADOT holdings
		let current_timestamp = hydradx_runtime::Timestamp::get();
		let new_timestamp = current_timestamp + (1 * 1000); // milliseconds
		hydradx_runtime::Timestamp::set_timestamp(new_timestamp);


		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_fee_currency_issuance = Currencies::total_issuance(fee_currency);

		// Create the set_currency call to set ADOT as fee payment currency
		let set_currency_call = RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: fee_currency },
		);

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		// Generate permit
		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				set_currency_call.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		// Validate unsigned first
		let call: pallet_transaction_multi_payment::Call<Runtime> =
			pallet_transaction_multi_payment::Call::dispatch_permit {
				from: user_evm_address,
				to: DISPATCH_ADDR,
				value: U256::from(0),
				data: set_currency_call.encode(),
				gas_limit,
				deadline,
				v: v.serialize(),
				r: H256::from(rs.r.b32()),
				s: H256::from(rs.s.b32()),
			};

		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
				longevity: 64,
				propagate: true,
			})
		);

		// Dispatch the permit
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			set_currency_call.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		// Verify the currency was set to ADOT
		let currency = pallet_transaction_multi_payment::Pallet::<Runtime>::account_currency(&user_acc.address());
		assert_eq!(currency, fee_currency);

		// Verify total issuance didn't change (fees are transferred, not burned)
		let fee_currency_issuance = Currencies::total_issuance(fee_currency);
		assert_eq!(initial_fee_currency_issuance, fee_currency_issuance);

		// Verify user's ADOT balance decreased (fee was paid)
		let user_fee_currency_balance = user_acc.balance(fee_currency);
		assert!(user_fee_currency_balance < initial_user_fee_currency_balance);

		// Verify treasury received the fee
		let final_treasury_fee_balance = treasury_acc.balance(fee_currency);
		assert!(final_treasury_fee_balance > initial_treasury_fee_balance);

		// Verify the fee amount matches what treasury received
		let fee_amount = initial_user_fee_currency_balance - user_fee_currency_balance;
		let treasury_received = final_treasury_fee_balance - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_received);
	})
}

fn set_ed(asset_id: AssetId, ed: u128) {
	AssetRegistry::update(
		hydradx_runtime::RuntimeOrigin::root(),
		asset_id,
		None,
		None,
		Some(ed),
		None,
		None,
		None,
		None,
		None,
	)
		.unwrap();
}
