use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::hydradx_run_to_next_block;
use crate::polkadot_test_net::{TestNet, ALICE, BOB, HDX};
use fp_evm::ExitSucceed::Returned;
use fp_evm::{ExitReason::Succeed, ExitSucceed::Stopped};
use frame_support::assert_ok;
use frame_support::dispatch::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::{
	evm::{
		precompiles::{handle::EvmDataWriter, Bytes},
		Executor,
	},
	AccountId, Currencies, EVMAccounts, FixedU128, Liquidation, Router, Tokens, Treasury, HSM,
};
use hydradx_runtime::{RuntimeOrigin, Stableswap};
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::MultiCurrency;
use pallet_asset_registry::AssetType;
use pallet_ema_oracle::BIFROST_SOURCE;
use pallet_stableswap::types::BoundedPegSources;
use pallet_stableswap::types::PegSource;
use pretty_assertions::assert_eq;
use primitives::{AssetId, Balance};
use sp_core::{RuntimeDebug, H256, U256};
use sp_runtime::traits::One;
use sp_runtime::BoundedVec;
use sp_runtime::Perbill;
use sp_runtime::Permill;
use std::sync::Arc;
use xcm_emulator::{Network, TestExt};

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	AddFacilitator = "addFacilitator(address,string,uint128)",
	Mint = "mint(address,uint256)",
	ListFacilitator = "getFacilitatorsList()",
	BalanceOf = "balanceOf(address)",
	FlashLoan = "flashLoan(address,address,uint256,bytes)",
	AddFlashBorrower = "addFlashBorrower(address)",
	IsFlashBorrower = "isFlashBorrower(address)",
}

fn hollar_contract_address() -> EvmAddress {
	EvmAddress::from_slice(&hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8"))
}

fn hollar_contract_manager() -> EvmAddress {
	EvmAddress::from_slice(&hex!("52341e77341788Ebda44C8BcB4C8BD1B1913B204"))
}

fn minter() -> EvmAddress {
	EvmAddress::from_slice(&hex!("8f3ac7f6482abc1a5c48a95d97f7a235186dbb68"))
}

fn balance_of(address: EvmAddress) -> U256 {
	let context = CallContext::new_view(hollar_contract_address());
	let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
		.write(address)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	std::assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	sp_core::U256::from(value.as_slice())
}

fn list_facilitators() -> Vec<EvmAddress> {
	let data = Into::<u32>::into(Function::ListFacilitator).to_be_bytes().to_vec();
	let context = CallContext::new_view(hollar_contract_address());
	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	std::assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	let mut r = vec![];
	for c in value.chunks(32) {
		r.push(EvmAddress::from(H256::from_slice(c)));
	}
	r
}

fn add_facilitator(facilitator: EvmAddress, label: &str, capacity: u128) {
	let context = CallContext::new_call(hollar_contract_address(), hollar_contract_manager());
	let data = EvmDataWriter::new_with_selector(Function::AddFacilitator)
		.write(facilitator)
		.write(Bytes::from(label))
		.write(capacity)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

fn add_flash_borrower(borrower: EvmAddress) {
	let acl_manager = hex!["c54dcFaEB75F56907E8B1De931dB4E37Bd0Afbb4"].into();
	let context = CallContext::new_call(acl_manager, hollar_contract_manager());
	let data = EvmDataWriter::new_with_selector(Function::AddFlashBorrower)
		.write(borrower)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

fn check_flash_borrower(borrower: EvmAddress) -> bool {
	let acl_manager = hex!["c54dcFaEB75F56907E8B1De931dB4E37Bd0Afbb4"].into();
	let data = EvmDataWriter::new_with_selector(Function::IsFlashBorrower)
		.write(borrower)
		.build();
	let context = CallContext::new_view(acl_manager);
	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	std::assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	!value.is_empty() && value.iter().any(|&x| x != 0)
}

fn mint(facilitator: EvmAddress, to: EvmAddress, amount: u128) {
	let context = CallContext::new_call(hollar_contract_address(), facilitator);
	let data = EvmDataWriter::new_with_selector(Function::Mint)
		.write(to)
		.write(amount)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 4_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

#[test]
fn add_hsm_facilitator_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		let facilitators = list_facilitators();
		assert!(facilitators.contains(&hsm_evm_address), "Facilitator not added");
	});
}

#[test]
fn buying_hollar_from_hsm_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

		let pool_id = 9876;
		let asset_ids = vec![222, 2];

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(pool_id),
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(hydradx_runtime::Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			BoundedVec::truncate_from(asset_ids),
			amplification,
			fee,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			2,
			20_000_000_000_000_000_000,
			0,
		));

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			2,
			pool_id,
			Permill::zero(),
			FixedU128::one(),
			Permill::zero(),
			Perbill::one(),
			None
		));

		assert_ok!(HSM::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			2,
			222,
			1_000_000_000_000_000_000,
			u128::MAX,
		));
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1001_000_000_000_000_000_000u128));

		let hsm_dai_balance = Tokens::free_balance(2, &hsm_address);
		assert_eq!(hsm_dai_balance, 1000000000000000000);

		let alice_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		assert_eq!(alice_dai_balance, 20_000_000_000_000_000_000 - hsm_dai_balance);
	});
}

#[test]
fn buying_hollar_from_hsm_via_router_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

		let pool_id = 9876;
		let asset_ids = vec![222, 2];

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(pool_id),
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(hydradx_runtime::Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			BoundedVec::truncate_from(asset_ids),
			amplification,
			fee,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			2,
			20_000_000_000_000_000_000,
			0,
		));

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			2,
			pool_id,
			Permill::zero(),
			FixedU128::one(),
			Permill::zero(),
			Perbill::one(),
			None
		));

		assert_ok!(Router::buy(
			RuntimeOrigin::signed(ALICE.into()),
			2,
			222,
			1_000_000_000_000_000_000,
			u128::MAX,
			Route::truncate_from(vec![Trade {
				pool: PoolType::HSM,
				asset_in: 2,
				asset_out: 222,
			}])
		));
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1001_000_000_000_000_000_000u128));

		let hsm_dai_balance = Tokens::free_balance(2, &hsm_address);
		assert_eq!(hsm_dai_balance, 1000000000000000000);

		let alice_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		assert_eq!(alice_dai_balance, 20_000_000_000_000_000_000 - hsm_dai_balance);
	});
}

#[test]
fn selling_hollar_to_hsm_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

		let pool_id = 9876;
		let asset_ids = vec![222, 2];

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(pool_id),
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(hydradx_runtime::Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			BoundedVec::truncate_from(asset_ids),
			amplification,
			fee,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			2,
			920_000_000_000_000_000_000,
			0,
		));
		let initial_liquidity = vec![
			AssetAmount::new(2, 900_000_000_000_000_000_000u128),
			AssetAmount::new(222, 1000_000_000_000_000_000_000u128),
		];

		assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			BoundedVec::truncate_from(initial_liquidity),
			0
		));

		hydradx_run_to_next_block();

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			2,
			pool_id,
			Permill::zero(),
			FixedU128::from_rational(110, 100),
			Permill::zero(),
			Perbill::from_percent(70),
			None
		));
		assert_ok!(HSM::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			2,
			222,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let alice_hollar_balance = balance_of(alice_evm_address);
		let alice_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		let hsm_dai_balance = Tokens::free_balance(2, &hsm_address);

		assert_ok!(HSM::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			222,
			2,
			300_000_000_000_000_000,
			0,
		));

		let alice_final_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(
			alice_final_hollar_balance,
			alice_hollar_balance - U256::from(300_000_000_000_000_000u128)
		);

		let hsm_final_dai_balance = Tokens::free_balance(2, &hsm_address);
		let paid_dai = hsm_dai_balance - hsm_final_dai_balance;

		let alice_final_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		assert_eq!(alice_final_dai_balance, alice_dai_balance + paid_dai);

		let hsm_hollar_balance = balance_of(hsm_evm_address);
		assert_eq!(hsm_hollar_balance, U256::zero());
	});
}

#[test]
fn selling_hollar_to_hsm_via_router_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

		let pool_id = 9876;
		let asset_ids = vec![222, 2];

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(pool_id),
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(hydradx_runtime::Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			BoundedVec::truncate_from(asset_ids),
			amplification,
			fee,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			2,
			920_000_000_000_000_000_000,
			0,
		));
		let initial_liquidity = vec![
			AssetAmount::new(2, 900_000_000_000_000_000_000u128),
			AssetAmount::new(222, 1000_000_000_000_000_000_000u128),
		];

		assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			BoundedVec::truncate_from(initial_liquidity),
			0
		));

		hydradx_run_to_next_block();

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			2,
			pool_id,
			Permill::zero(),
			FixedU128::from_rational(110, 100),
			Permill::zero(),
			Perbill::from_percent(70),
			None
		));
		assert_ok!(HSM::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			2,
			222,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let alice_hollar_balance = balance_of(alice_evm_address);
		let alice_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		let hsm_dai_balance = Tokens::free_balance(2, &hsm_address);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			222,
			2,
			300_000_000_000_000_000,
			0,
			Route::truncate_from(vec![Trade {
				pool: PoolType::HSM,
				asset_in: 222,
				asset_out: 2,
			}])
		));
		let alice_final_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(
			alice_final_hollar_balance,
			alice_hollar_balance - U256::from(300_000_000_000_000_000u128)
		);

		let hsm_final_dai_balance = Tokens::free_balance(2, &hsm_address);
		let paid_dai = hsm_dai_balance - hsm_final_dai_balance;

		let alice_final_dai_balance = Tokens::free_balance(2, &AccountId::from(ALICE));
		assert_eq!(alice_final_dai_balance, alice_dai_balance + paid_dai);

		let hsm_hollar_balance = balance_of(hsm_evm_address);
		assert_eq!(hsm_hollar_balance, U256::zero());
	});
}

#[test]
#[ignore]
fn deploy_gho_token_should_work() {
	TestNet::reset();
	crate::polkadot_test_net::Hydra::execute_with(|| {
		let admin_evm: EvmAddress = hex!["52341e77341788Ebda44C8BcB4C8BD1B1913B204"].into();
		let _gho_contract_addr = crate::utils::contracts::deploy_contract("GhoToken", admin_evm);
	});
}

const HOLLAR: AssetId = 222;
const COLLATERAL: AssetId = 10234;
const DECIMALS: u8 = 18;
const HOLLAR_COLLATERAL_PRICE: (Balance, Balance) = (1_000_000_000_000_000_000, 500_000_000_000_000_000);
const POOL_ID: AssetId = 9876;

#[test]
fn buy_hollar_with_yield_bearing_token_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			),));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 450 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(110, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_ok!(HSM::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				1_000_000_000_000_000_000,
				u128::MAX,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				alice_hollar_balance - initial_alice_hollar_balance,
				U256::from(1_000_000_000_000_000_000u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			assert_eq!(hsm_collateral_balance, 500000000000000000);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(
				alice_collateral_balance,
				initial_alice_collateral_balance - hsm_collateral_balance
			);
		});
}

#[test]
fn sell_yield_bearing_token_to_get_hollar_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			),));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 900 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				COLLATERAL,
				HOLLAR,
				10_000_000_000_000_000,
				0,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			let received = alice_hollar_balance - initial_alice_hollar_balance;
			assert_eq!(received, U256::from(19_866_697_319_636_617u128));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(110, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_ok!(HSM::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				10_000_000_000_000_000_000,
				0,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				alice_hollar_balance - initial_alice_hollar_balance,
				U256::from(20_000_000_000_000_000_000u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			assert_eq!(hsm_collateral_balance, 10000000000000000000);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(
				alice_collateral_balance,
				initial_alice_collateral_balance - hsm_collateral_balance
			);
		});
}

#[test]
fn sell_collateral_to_get_hollar_via_router_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			),));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 900 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				COLLATERAL,
				HOLLAR,
				10_000_000_000_000_000,
				0,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			let received = alice_hollar_balance - initial_alice_hollar_balance;
			assert_eq!(received, U256::from(19_866_697_319_636_617u128));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(110, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));

			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				10_000_000_000_000_000_000,
				0,
				Route::truncate_from(vec![Trade {
					pool: PoolType::HSM,
					asset_in: COLLATERAL,
					asset_out: HOLLAR,
				}])
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				alice_hollar_balance - initial_alice_hollar_balance,
				U256::from(20_000_000_000_000_000_000u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			assert_eq!(hsm_collateral_balance, 10000000000000000000);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(
				alice_collateral_balance,
				initial_alice_collateral_balance - hsm_collateral_balance
			);
		});
}

#[test]
fn sell_collateral_to_get_hollar_via_router_should_work_when_collateral_is_acquired_from_omnipool() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.add_asset_to_omnipool(COLLATERAL, 1000_000_000_000_000_000_000, FixedU128::one())
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 910 * 10u128.pow(DECIMALS as u32))
		.endow_account(ALICE.into(), HDX, 1_000_000_000_000_000_000u128)
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			),));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 900 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				hydradx_runtime::Omnipool::protocol_account(),
				HDX,
				1_000_000_000_000_000 as i128,
			));

			hydradx_run_to_next_block();

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				COLLATERAL,
				HOLLAR,
				10_000_000_000_000_000_000,
				0,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			let received = alice_hollar_balance - initial_alice_hollar_balance;
			assert_eq!(received, U256::from(19862174293799271720u128));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(110, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(initial_alice_collateral_balance, 0u128);
			let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			assert_eq!(hsm_collateral_balance, 0u128);
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				HOLLAR,
				100_000_000_000_000,
				0,
				Route::truncate_from(vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: COLLATERAL,
					},
					Trade {
						pool: PoolType::HSM,
						asset_in: COLLATERAL,
						asset_out: HOLLAR,
					}
				])
			));
			let final_alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
			assert_eq!(final_alice_hdx_balance, alice_hdx_balance - 100_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				alice_hollar_balance - initial_alice_hollar_balance,
				U256::from(88535149283848u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			assert_eq!(hsm_collateral_balance, 44267574641924);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(alice_collateral_balance, 0u128);
		});
}

#[test]
fn sell_hollar_to_get_yield_bearing_token_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 450 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(210, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			assert_ok!(HSM::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				1_000_000_000_000_000_000,
				u128::MAX,
			));

			let initial_hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_ok!(HSM::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HOLLAR,
				COLLATERAL,
				1_000_000_000_000_000_000,
				0,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				initial_alice_hollar_balance - alice_hollar_balance,
				U256::from(1_000_000_000_000_000_000u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let received = initial_hsm_collateral_balance - hsm_collateral_balance;
			assert_eq!(received, 499_481_608_053_132_511);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(alice_collateral_balance, initial_alice_collateral_balance + received);
		});
}

#[test]
fn buy_yield_bearing_token_with_hollar_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 450 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(210, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			assert_ok!(HSM::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				2_000_000_000_000_000_000,
				u128::MAX,
			));

			let initial_hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_ok!(HSM::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HOLLAR,
				COLLATERAL,
				500_000_000_000_000_000,
				u128::MAX,
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				initial_alice_hollar_balance - alice_hollar_balance,
				U256::from(1_001_037_848_935_021_083u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let received = initial_hsm_collateral_balance - hsm_collateral_balance;
			assert_eq!(received, 500_000_000_000_000_000);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(alice_collateral_balance, initial_alice_collateral_balance + received);
		});
}

#[test]
fn buy_collateral_with_hollar_via_router_should_work() {
	let collateral_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		1,
		polkadot_xcm::v4::Junctions::X2(Arc::new([
			polkadot_xcm::v4::Junction::Parachain(2000),
			polkadot_xcm::v4::Junction::GeneralIndex(0),
		])),
	);
	let hollar_location: polkadot_xcm::v4::Location = polkadot_xcm::v4::Location::new(
		0,
		polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::AccountKey20 {
			network: None,
			key: hex!("c130c89f2b1066a77bd820aafebcf4519d0103d8").into(),
		}])),
	);

	let hollar_boxed = Box::new(hollar_location.clone().into_versioned());
	let collateral_boxed = Box::new(collateral_location.clone().into_versioned());

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.register_asset(COLLATERAL, b"myCOL", DECIMALS, Some(collateral_location))
		.register_asset(POOL_ID, b"pool", DECIMALS, None)
		.update_bifrost_oracle(hollar_boxed, collateral_boxed, HOLLAR_COLLATERAL_PRICE)
		.new_block()
		.endow_account(ALICE.into(), COLLATERAL, 1_000_000 * 10u128.pow(DECIMALS as u32))
		.execute(|| {
			let hsm_address = hydradx_runtime::HSM::account_id();
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				hsm_address.clone().into()
			)));
			let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
			add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)));
			let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
			mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

			let assets = vec![HOLLAR, COLLATERAL];
			let pegs = vec![
				PegSource::Value((1, 1)),                                             // aDOT peg
				PegSource::Oracle((BIFROST_SOURCE, OraclePeriod::LastBlock, HOLLAR)), // vDOT peg
			];
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(assets),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(pegs),
				Permill::from_percent(100),
			));

			let liquidity = vec![
				AssetAmount::new(HOLLAR, 1_000 * 10u128.pow(DECIMALS as u32)),
				AssetAmount::new(COLLATERAL, 450 * 10u128.pow(DECIMALS as u32)),
			];

			// Add initial liquidity
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(liquidity),
				0,
			));

			hydradx_run_to_next_block();

			assert_ok!(HSM::add_collateral_asset(
				hydradx_runtime::RuntimeOrigin::root(),
				COLLATERAL,
				POOL_ID,
				Permill::zero(),
				FixedU128::from_rational(210, 100),
				Permill::zero(),
				Perbill::from_percent(70),
				None
			));

			assert_ok!(HSM::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				COLLATERAL,
				HOLLAR,
				2_000_000_000_000_000_000,
				u128::MAX,
			));

			let initial_hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let initial_alice_hollar_balance = balance_of(alice_evm_address);
			let initial_alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));

			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HOLLAR,
				COLLATERAL,
				500_000_000_000_000_000,
				u128::MAX,
				Route::truncate_from(vec![Trade {
					pool: PoolType::HSM,
					asset_in: HOLLAR,
					asset_out: COLLATERAL,
				}])
			));
			let alice_hollar_balance = balance_of(alice_evm_address);
			assert_eq!(
				initial_alice_hollar_balance - alice_hollar_balance,
				U256::from(1_001_037_848_935_021_083u128)
			);

			let hsm_collateral_balance = Tokens::free_balance(COLLATERAL, &hsm_address);
			let received = initial_hsm_collateral_balance - hsm_collateral_balance;
			assert_eq!(received, 500_000_000_000_000_000);

			let alice_collateral_balance = Tokens::free_balance(COLLATERAL, &AccountId::from(ALICE));
			assert_eq!(alice_collateral_balance, initial_alice_collateral_balance + received);
		});
}

use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_traits::router::{PoolType, Route, Trade};

#[test]
fn arbitrage_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();

		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert!(!check_flash_borrower(hsm_evm_address));
		add_flash_borrower(hsm_evm_address);
		assert!(check_flash_borrower(hsm_evm_address));

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(minter(), alice_evm_address, 1000_000_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1000_000_000_000_000_000_000u128));

		let pool_id = 9876;
		let asset_ids = vec![222, 2];

		assert_ok!(hydradx_runtime::AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(pool_id),
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(hydradx_runtime::Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			BoundedVec::truncate_from(asset_ids),
			amplification,
			fee,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			2,
			920_000_000_000_000_000_000,
			0,
		));
		let initial_liquidity = vec![
			AssetAmount::new(2, 900_000_000_000_000_000_000u128),
			AssetAmount::new(222, 1000_000_000_000_000_000_000u128),
		];

		assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			BoundedVec::truncate_from(initial_liquidity),
			0
		));

		hydradx_run_to_next_block();

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			2,
			pool_id,
			Permill::zero(),
			FixedU128::from_rational(110, 100),
			Permill::zero(),
			Perbill::from_percent(70),
			None
		));

		assert_ok!(HSM::set_flash_minter(
			hydradx_runtime::RuntimeOrigin::root(),
			flash_minter,
		));

		// let's buy some hollar, so hsm holds some collateral
		assert_ok!(HSM::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			2,
			222,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let hsm_dai_balance = Tokens::free_balance(2, &hsm_address);
		assert_ok!(HSM::execute_arbitrage(hydradx_runtime::RuntimeOrigin::none(), 2));
		let final_hsm_dai_balance = Tokens::free_balance(2, &hsm_address);
		let traded_amount = hsm_dai_balance - final_hsm_dai_balance;
		assert_eq!(traded_amount, 999_642_225_291_583_959);
	});
}

const DOT: AssetId = 5;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH: AssetId = 20;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const ALICE_INITIAL_WETH_BALANCE: Balance = 20 * WETH_UNIT;
const ALICE_INITIAL_DOT_BALANCE: Balance = 10_000 * DOT_UNIT;

use hydradx_traits::evm::Erc20Encoding;
use sp_runtime::traits::CheckedConversion;

#[test]
fn hollar_liquidation_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
		assert_ok!(HSM::set_flash_minter(
			hydradx_runtime::RuntimeOrigin::root(),
			flash_minter,
		));

		// Arrange
		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let pool_contract = crate::liquidation::get_pool(pap_contract);
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		let pallet_acc = Liquidation::account_id();
		let dot_asset_address = HydraErc20Mapping::encode_evm_address(DOT);
		let weth_asset_address = HydraErc20Mapping::encode_evm_address(WETH);

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), ALICE_INITIAL_DOT_BALANCE));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), ALICE_INITIAL_WETH_BALANCE));

		let treasury_hollar_initial_balance = Currencies::free_balance(222, &Treasury::account_id());

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(BOB.into()),));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(pallet_acc.clone()),));

		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let liquidation_evm_address = EVMAccounts::evm_address(&pallet_acc);
		assert!(!check_flash_borrower(liquidation_evm_address));
		add_flash_borrower(liquidation_evm_address);
		assert!(check_flash_borrower(liquidation_evm_address));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Create pool to swap collateral for hollar after liquidation
		let stable_pool_id = 123456;
		let weth_liquidity = 1990476190476190476 * 2;
		let hollar_liquidity = 20_000 * 1_000_000_000_000_000_000u128;
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), weth_liquidity));
		mint(minter(), alice_evm_address, hollar_liquidity);
		let initial_stable_liquidity = vec![
			AssetAmount::new(WETH, weth_liquidity),
			AssetAmount::new(222, hollar_liquidity),
		];
		create_stablepool(stable_pool_id, vec![WETH, 222], initial_stable_liquidity);

		let collateral_weth_amount: Balance = 2 * WETH_UNIT;
		let collateral_dot_amount = 1_000 * DOT_UNIT;
		crate::liquidation::supply(
			pool_contract,
			alice_evm_address,
			weth_asset_address,
			collateral_weth_amount,
		);
		crate::liquidation::supply(
			pool_contract,
			alice_evm_address,
			dot_asset_address,
			collateral_dot_amount,
		);

		std::assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount
		);
		std::assert_eq!(
			Currencies::free_balance(WETH, &ALICE.into()),
			ALICE_INITIAL_WETH_BALANCE - collateral_weth_amount
		);

		let hollar_address = hollar_contract_address();
		let hollar_borrow_amount: Balance = 5_000 * 1_000_000_000_000_000_000u128;
		std::assert_eq!(Currencies::free_balance(222, &ALICE.into()), 0);

		crate::liquidation::borrow(pool_contract, alice_evm_address, hollar_address, hollar_borrow_amount);

		std::assert_eq!(Currencies::free_balance(222, &ALICE.into()), hollar_borrow_amount,);
		std::assert_eq!(
			Currencies::free_balance(DOT, &ALICE.into()),
			ALICE_INITIAL_DOT_BALANCE - collateral_dot_amount
		);

		let (price, timestamp) = crate::liquidation::get_oracle_price("DOT/USD");
		let price = price.as_u128() / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		crate::liquidation::update_oracle_price("DOT/USD", U256::checked_from(&data[0..32]).unwrap());

		let (price, timestamp) = crate::liquidation::get_oracle_price("WETH/USD");
		let price = price.as_u128() / 2;
		let timestamp = timestamp.as_u128() + 6;
		let mut data = price.to_be_bytes().to_vec();
		data.extend_from_slice(timestamp.to_be_bytes().as_ref());
		crate::liquidation::update_oracle_price("WETH/USD", U256::checked_from(&data[0..32]).unwrap());

		// ensure that the health_factor < 1
		let user_data = crate::liquidation::get_user_account_data(pool_contract, alice_evm_address);
		assert!(user_data.5 < U256::from(1_000_000_000_000_000_000u128));

		let route = BoundedVec::truncate_from(vec![hydradx_traits::router::Trade {
			pool: PoolType::Stableswap(stable_pool_id),
			asset_in: WETH,
			asset_out: 222,
		}]);

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(BOB.into()),
			WETH,
			222,
			alice_evm_address,
			hollar_borrow_amount,
			route
		));

		// Assert
		std::assert_eq!(Currencies::free_balance(DOT, &pallet_acc), 0);
		std::assert_eq!(Currencies::free_balance(WETH, &pallet_acc), 0);
		std::assert_eq!(Currencies::free_balance(222, &pallet_acc), 0);

		assert!(Currencies::free_balance(222, &Treasury::account_id()) > treasury_hollar_initial_balance);

		std::assert_eq!(Currencies::free_balance(DOT, &BOB.into()), 0);
		std::assert_eq!(Currencies::free_balance(WETH, &BOB.into()), 0);
		std::assert_eq!(Currencies::free_balance(222, &BOB.into()), 0);
	});
}

fn create_stablepool(pool_id: AssetId, assets: Vec<AssetId>, initial_liquidity: Vec<AssetAmount<AssetId>>) {
	assert_ok!(hydradx_runtime::AssetRegistry::register(
		RawOrigin::Root.into(),
		Some(pool_id),
		Some(b"mypool".to_vec().try_into().unwrap()),
		AssetType::StableSwap,
		Some(1u128),
		None,
		None,
		None,
		None,
		true,
	));

	let amplification = 100u16;
	let fee = Permill::from_percent(0);

	assert_ok!(hydradx_runtime::Stableswap::create_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		pool_id,
		BoundedVec::truncate_from(assets),
		amplification,
		fee,
	));

	assert_ok!(hydradx_runtime::Stableswap::add_assets_liquidity(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		pool_id,
		BoundedVec::truncate_from(initial_liquidity),
		0
	));
}
