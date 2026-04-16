#![cfg(test)]

use crate::gigahdx::PATH_TO_SNAPSHOT;
use crate::liquidation::{borrow, get_user_account_data, ApiProvider};
use crate::polkadot_test_net::*;
use frame_support::{
	assert_ok,
	traits::{OnInitialize, StorePreimage},
	BoundedVec,
};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::{
	evm::{precompiles::erc20_mapping::HydraErc20Mapping, Executor},
	Balances, Block, BorrowingTreasuryAccount, ConvictionVoting, Currencies, Democracy, EVMAccounts, GigaHdx,
	GigaHdxLiquidationAccount, Liquidation, OriginCaller, Preimage, Referenda, Runtime, RuntimeCall, RuntimeEvent,
	RuntimeOrigin, Scheduler, System,
};
use hydradx_traits::evm::{CallContext, Erc20Encoding, Erc20Mapping, InspectEvmAccounts, EVM};
use liquidation_worker_support::*;
use orml_traits::MultiCurrency;
use pallet_conviction_voting::{AccountVote, Conviction, Vote};
use primitives::constants::time::DAYS;
use primitives::{AssetId, Balance, EvmAddress};
use sp_core::{H160, H256, U256};
use xcm_emulator::Network;

const UNITS: Balance = 1_000_000_000_000;
const HOLLAR_UNITS: Balance = 1_000_000_000_000_000_000; // 18 decimals
const GIGAHDX: AssetId = 67;
const HOLLAR: AssetId = 222;

const MAINNET_PAP_CONTRACT: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fetch_pool_contract(caller: EvmAddress) -> EvmAddress {
	let block_number = hydradx_runtime::System::block_number();
	let hash = hydradx_runtime::System::block_hash(block_number);
	MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_pool::<ApiProvider<Runtime>>(
		&ApiProvider::<Runtime>(Runtime),
		hash,
		MAINNET_PAP_CONTRACT,
		caller,
	)
	.unwrap()
}

fn selector(sig: &str) -> Vec<u8> {
	sp_io::hashing::keccak_256(sig.as_bytes())[0..4].to_vec()
}

fn try_borrow(
	mm_pool: EvmAddress,
	user: EvmAddress,
	asset: EvmAddress,
	amount: Balance,
) -> (fp_evm::ExitReason, Vec<u8>) {
	use hydradx_runtime::evm::precompiles::handle::EvmDataWriter;
	let data = EvmDataWriter::new_with_selector(Function::Borrow)
		.write(asset)
		.write(amount)
		.write(2u32)
		.write(0u32)
		.write(user)
		.build();
	let result = Executor::<Runtime>::call(CallContext::new_call(mm_pool, user), data, U256::zero(), 50_000_000);
	(result.exit_reason, result.value)
}

fn borrow_should_fail(mm_pool: EvmAddress, user: EvmAddress, asset: EvmAddress, amount: Balance) {
	let (reason, value) = try_borrow(mm_pool, user, asset, amount);
	assert!(
		matches!(reason, fp_evm::ExitReason::Revert(_)),
		"Borrow should revert, but got: {:?} data={}",
		reason,
		hex::encode(&value)
	);
}

fn evm_call(target: EvmAddress, caller: EvmAddress, data: Vec<u8>, gas: u64, label: &str) {
	let result = Executor::<Runtime>::call(CallContext::new_call(target, caller), data, U256::zero(), gas);
	assert!(
		matches!(result.exit_reason, fp_evm::ExitReason::Succeed(_)),
		"[{label}] EVM call failed: {:?} data={}",
		result.exit_reason,
		hex::encode(&result.value)
	);
}

fn set_use_as_collateral(pool: EvmAddress, user: EvmAddress, asset: EvmAddress) {
	let mut data = selector("setUserUseReserveAsCollateral(address,bool)");
	data.extend_from_slice(H256::from(asset).as_bytes());
	data.extend_from_slice(&[0u8; 31]);
	data.push(1u8);
	evm_call(pool, user, data, 500_000, "setUserUseReserveAsCollateral");
}

/// Deploy a minimal EVM contract that returns a fixed uint256 from any call.
fn deploy_fixed_price_oracle(price: U256) -> EvmAddress {
	let acl_admin = EvmAddress::from_slice(&hex!("aa7e0000000000000000000000000000000aa7e0"));

	// Runtime bytecode: PUSH32 <price> | PUSH1 0 | MSTORE | PUSH1 32 | PUSH1 0 | RETURN
	let mut runtime = vec![0x7f]; // PUSH32
	runtime.extend_from_slice(&price.to_big_endian());
	runtime.extend_from_slice(&[0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xF3]);
	// runtime is 41 bytes

	// Constructor: copy runtime to memory and return it
	// PUSH1 <len> PUSH1 <offset> PUSH1 0 CODECOPY | PUSH1 <len> PUSH1 0 RETURN
	let rt_len = runtime.len() as u8;
	let code_offset = 12u8; // constructor is 12 bytes
	let mut init_code = vec![
		0x60,
		rt_len, // PUSH1 rt_len
		0x60,
		code_offset, // PUSH1 code_offset
		0x60,
		0x00, // PUSH1 0
		0x39, // CODECOPY
		0x60,
		rt_len, // PUSH1 rt_len
		0x60,
		0x00, // PUSH1 0
		0xF3, // RETURN
	];
	init_code.extend_from_slice(&runtime);

	use pallet_evm::Runner;
	<Runtime as pallet_evm::Config>::Runner::create(
		acl_admin,
		init_code,
		U256::zero(),
		1_000_000,
		Some(U256::from(1_000_000_000u64)),
		None,
		None,
		vec![],
		vec![],
		false,
		true,
		None,
		None,
		<Runtime as pallet_evm::Config>::config(),
	)
	.expect("Deploy mock oracle failed")
	.value
}

/// Query the current Aave oracle price for an asset.
fn get_aave_asset_price(asset: EvmAddress) -> U256 {
	let price_oracle_sel = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
	let result = Executor::<Runtime>::view(CallContext::new_view(MAINNET_PAP_CONTRACT), price_oracle_sel, 100_000);
	let price_oracle = EvmAddress::from_slice(&result.value[12..32]);

	let mut data = selector("getAssetPrice(address)");
	data.extend_from_slice(H256::from(asset).as_bytes());
	let result = Executor::<Runtime>::view(CallContext::new_view(price_oracle), data, 100_000);
	U256::from_big_endian(&result.value)
}

/// Redirect the Aave oracle's price source for an asset to a different contract.
fn set_aave_price_source(asset: EvmAddress, source: EvmAddress) {
	let acl_admin = EvmAddress::from_slice(&hex!("aa7e0000000000000000000000000000000aa7e0"));
	let price_oracle_sel = Into::<u32>::into(Function::GetPriceOracle).to_be_bytes().to_vec();
	let result = Executor::<Runtime>::view(CallContext::new_view(MAINNET_PAP_CONTRACT), price_oracle_sel, 100_000);
	let price_oracle = EvmAddress::from_slice(&result.value[12..32]);

	let mut data = selector("setAssetSources(address[],address[])");
	data.extend_from_slice(&H256::from_low_u64_be(64).0);
	data.extend_from_slice(&H256::from_low_u64_be(128).0);
	data.extend_from_slice(&H256::from_low_u64_be(1).0);
	data.extend_from_slice(H256::from(asset).as_bytes());
	data.extend_from_slice(&H256::from_low_u64_be(1).0);
	data.extend_from_slice(H256::from(source).as_bytes());
	evm_call(price_oracle, acl_admin, data, 500_000, "setAssetSources");
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
	Democracy::on_initialize(System::block_number());
}

fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		next_block();
	}
}

/// Create an ongoing referendum so ALICE can vote on it and lock her GIGAHDX.
fn begin_referendum() -> u32 {
	let referendum_index = pallet_referenda::pallet::ReferendumCount::<Runtime>::get();
	let now = System::block_number();

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		sp_runtime::AccountId32::from(CHARLIE),
		1_000_000 * UNITS,
	));
	let proposal = {
		let inner = pallet_balances::Call::force_set_balance {
			who: sp_runtime::AccountId32::from(DAVE),
			new_free: 2,
		};
		Preimage::bound(hydradx_runtime::RuntimeCall::Balances(inner)).unwrap()
	};
	assert_ok!(Referenda::submit(
		RuntimeOrigin::signed(sp_runtime::AccountId32::from(CHARLIE)),
		Box::new(RawOrigin::Root.into()),
		proposal,
		frame_support::traits::schedule::DispatchTime::At(now + 10 * DAYS),
	));
	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		sp_runtime::AccountId32::from(DAVE),
		2_000_000_000 * UNITS,
	));
	assert_ok!(Referenda::place_decision_deposit(
		RuntimeOrigin::signed(sp_runtime::AccountId32::from(DAVE)),
		referendum_index,
	));
	fast_forward_to(now + 5 * DAYS);

	referendum_index
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

/// Happy-path: user stakes HDX → gets GIGAHDX → borrows HOLLAR → price drops → treasury liquidates.
#[test]
fn gigahdx_liquidation_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// ---- Setup ----
		let alice = sp_runtime::AccountId32::from(ALICE);
		let treasury = BorrowingTreasuryAccount::get();
		let derived_account = GigaHdxLiquidationAccount::get();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let treasury_evm = EVMAccounts::evm_address(&treasury);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// ---- ALICE stakes HDX → gets GIGAHDX, enables as collateral, borrows HOLLAR ----
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);

		let borrow_amount: Balance = 5 * HOLLAR_UNITS;
		borrow(pool_contract, alice_evm, hollar_addr, borrow_amount);

		// ---- Fund treasury with stHDX collateral (needed to borrow HOLLAR during liquidation) ----
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			treasury.clone(),
			2_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(treasury.clone()),
			1_000_000 * UNITS
		));
		set_use_as_collateral(pool_contract, treasury_evm, sthdx_evm);

		// ---- Drop stHDX price → ALICE's health factor drops below 1 ----
		// Use a moderate drop so HF goes below 1 but liquidation still improves it.
		let original_price = get_aave_asset_price(sthdx_evm);
		let crashed_price = original_price * 30 / 100; // 70% drop
		let mock_oracle = deploy_fixed_price_oracle(crashed_price);
		set_aave_price_source(sthdx_evm, mock_oracle);

		let user_data = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert!(
			user_data.health_factor < U256::from(1_000_000_000_000_000_000u128),
			"Health factor should be < 1, got: {:?}",
			user_data.health_factor
		);

		// ---- Record pre-liquidation state ----
		let alice_gigahdx_before = Currencies::free_balance(GIGAHDX, &alice);
		let derived_gigahdx_before = Currencies::free_balance(GIGAHDX, &derived_account);
		let treasury_evm_account = EVMAccounts::account_id(treasury_evm);
		let treasury_gigahdx_before = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		let debt_before = user_data.total_debt_base;
		let hf_before = user_data.health_factor;
		let treasury_data_before = get_user_account_data(pool_contract, treasury_evm).unwrap();
		let treasury_debt_before = treasury_data_before.total_debt_base;

		// ---- Execute GIGAHDX treasury liquidation ----
		let debt_to_cover = borrow_amount / 2;
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			debt_to_cover,
			BoundedVec::new(),
		));

		// ---- Assertions ----
		let alice_gigahdx_after = Currencies::free_balance(GIGAHDX, &alice);
		assert!(
			alice_gigahdx_after < alice_gigahdx_before,
			"ALICE should have less GIGAHDX: before={}, after={}",
			alice_gigahdx_before,
			alice_gigahdx_after
		);

		let derived_gigahdx_after = Currencies::free_balance(GIGAHDX, &derived_account);
		let gigahdx_seized = derived_gigahdx_after - derived_gigahdx_before;
		assert!(
			gigahdx_seized > 0,
			"Derived account should have received seized GIGAHDX"
		);

		let treasury_gigahdx_after = Currencies::free_balance(GIGAHDX, &treasury_evm_account);
		assert_eq!(
			treasury_gigahdx_after, treasury_gigahdx_before,
			"Treasury GIGAHDX should not increase (seized goes to derived)"
		);

		let user_data_after = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert!(
			user_data_after.total_debt_base < debt_before,
			"Debt should be reduced: before={:?}, after={:?}",
			debt_before,
			user_data_after.total_debt_base
		);

		// Verify GigaHdxLiquidated event
		let events = hydradx_runtime::System::events();
		let liq_events: Vec<_> = events
			.iter()
			.filter(|r| {
				matches!(
					r.event,
					RuntimeEvent::Liquidation(pallet_liquidation::Event::GigaHdxLiquidated { .. })
				)
			})
			.collect();
		assert_eq!(liq_events.len(), 1);

		if let RuntimeEvent::Liquidation(pallet_liquidation::Event::GigaHdxLiquidated {
			user,
			debt_repaid,
			gigahdx_seized: seized,
		}) = &liq_events[0].event
		{
			assert_eq!(*user, alice_evm);
			assert_eq!(*debt_repaid, debt_to_cover);
			assert_eq!(*seized, gigahdx_seized);
		} else {
			panic!("Expected GigaHdxLiquidated event");
		}

		// Treasury should have acquired HOLLAR debt (it borrowed HOLLAR to fund the liquidation)
		let treasury_data_after = get_user_account_data(pool_contract, treasury_evm).unwrap();
		assert!(
			treasury_data_after.total_debt_base > treasury_debt_before,
			"Treasury should have HOLLAR debt: before={:?}, after={:?}",
			treasury_debt_before,
			treasury_data_after.total_debt_base
		);

		// Alice's health factor should improve after liquidation
		let user_data_final = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert!(
			user_data_final.health_factor > hf_before,
			"Health factor should improve: before={:?}, after={:?}",
			hf_before,
			user_data_final.health_factor
		);

		// Note: can't directly compare gigahdx_seized vs debt_to_cover — different tokens
		// with different decimals (12 vs 18) and different prices.
	});
}

/// Liquidation should succeed even when the user has voting locks on their GIGAHDX.
/// prepare_for_liquidation (Step 1) should clear all locks before Aave's liquidationCall.
///
/// Currently FAILS because prepare_for_liquidation does not clear GigaHdxVotingLock storage,
/// so the LockableAToken still blocks the transfer.
/// Fix: add GigaHdxVotingLock::remove(who) and LockSplit::remove(who) to prepare_for_liquidation.
#[test]
fn gigahdx_liquidation_with_voting_locks_should_clear_locks() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let treasury = BorrowingTreasuryAccount::get();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let treasury_evm = EVMAccounts::evm_address(&treasury);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// ALICE stakes HDX → gets GIGAHDX
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);

		// ALICE votes on a referendum → locks ALL her GIGAHDX
		let referendum_index = begin_referendum();
		assert_ok!(ConvictionVoting::vote(
			RuntimeOrigin::signed(alice.clone()),
			referendum_index,
			AccountVote::Standard {
				vote: Vote {
					aye: true,
					conviction: Conviction::Locked1x
				},
				balance: stake_amount,
			},
		));
		assert_eq!(
			pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&alice),
			stake_amount,
		);

		// ALICE borrows HOLLAR
		let borrow_amount: Balance = 5 * HOLLAR_UNITS;
		borrow(pool_contract, alice_evm, hollar_addr, borrow_amount);

		// Fund treasury
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			treasury.clone(),
			2_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(treasury.clone()),
			1_000_000 * UNITS
		));
		set_use_as_collateral(pool_contract, treasury_evm, sthdx_evm);

		// Crash price → HF < 1
		let original_price = get_aave_asset_price(sthdx_evm);
		let mock_oracle = deploy_fixed_price_oracle(original_price * 30 / 100); // 70% drop
		set_aave_price_source(sthdx_evm, mock_oracle);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			borrow_amount / 2,
			BoundedVec::new(),
		));
	});
}

//TODO: BUG: verify and fix
/// After giga_stake, stHDX is not auto-enabled as collateral because it's an isolated asset
/// (debtCeiling != 0). This means the user has GIGAHDX but zero borrowing power.
/// The UI handles this for PRIME by bundling a setUserUseReserveAsCollateral tx,
/// but giga_stake bypasses the UI — so the pallet needs to handle it.
#[test]
fn giga_stake_does_not_enable_collateral_for_isolated_sthdx() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let pool_contract = fetch_pool_contract(alice_evm);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Alice stakes HDX → gets GIGAHDX
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		// Alice has GIGAHDX but Aave shows zero collateral — stHDX is isolated and not auto-enabled
		let user_data = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert_eq!(
			user_data.total_collateral_base,
			U256::zero(),
			"Collateral should be 0 — stHDX is isolated and not auto-enabled"
		);
		assert_eq!(
			user_data.available_borrows_base,
			U256::zero(),
			"Borrowing power should be 0 without collateral enabled"
		);

		// Trying to borrow HOLLAR fails — no collateral enabled
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);
		borrow_should_fail(pool_contract, alice_evm, hollar_addr, 1 * HOLLAR_UNITS);

		// After manually enabling stHDX as collateral, borrowing works
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);

		let user_data_after = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert!(
			user_data_after.total_collateral_base > U256::zero(),
			"Collateral should be non-zero after enabling"
		);
		assert!(
			user_data_after.available_borrows_base > U256::zero(),
			"Borrowing power should be non-zero after enabling"
		);

		// Now borrow succeeds
		borrow(pool_contract, alice_evm, hollar_addr, 1 * HOLLAR_UNITS);
	});
}

/// stHDX is an isolated asset. HOLLAR, USDC, and USDT are borrowable in isolation
/// (configured via setBorrowableInIsolation on prod). All other assets are rejected.
#[test]
fn isolated_sthdx_only_allows_borrowing_hollar() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Alice stakes HDX → gets GIGAHDX, enable as collateral
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);

		// Borrowable in isolation: HOLLAR, USDC, USDT
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);
		borrow(pool_contract, alice_evm, hollar_addr, 1 * HOLLAR_UNITS);

		let usdc_addr = HydraErc20Mapping::encode_evm_address(22);
		borrow(pool_contract, alice_evm, usdc_addr, 1_000_000); // 1 USDC

		let usdt_addr = HydraErc20Mapping::encode_evm_address(10);
		borrow(pool_contract, alice_evm, usdt_addr, 1_000_000); // 1 USDT

		// All other borrowable assets should fail in isolation mode
		// All revert with Aave error "60" = ASSET_NOT_BORROWABLE_IN_ISOLATION
		let non_borrowable_in_isolation: Vec<(AssetId, Balance, &str)> = vec![
			(19, 100_000, "WBTC"),                    // 8 decimals
			(5, 10_000_000_000, "DOT"),               // 10 decimals
			(15, 10_000_000_000, "VDOT"),             // 10 decimals
			(34, 1_000_000_000_000_000, "ETH"),       // 18 decimals
			(43, 1_000_000, "PRIME"),                 // 6 decimals
			(1000765, 1_000_000_000_000_000, "tBTC"), // 18 decimals
			(39, 1_000_000_000_000_000, "PAXG"),      // 18 decimals
			(1000752, 1_000_000_000, "SOL"),          // 9 decimals
			(44, 1_000_000, "EURC"),                  // 6 decimals
		];

		// Aave error "60" = ASSET_NOT_BORROWABLE_IN_ISOLATION encoded as Error(string)
		let error_60 = hex::decode(
			"08c379a0\
			 0000000000000000000000000000000000000000000000000000000000000020\
			 0000000000000000000000000000000000000000000000000000000000000002\
			 3630000000000000000000000000000000000000000000000000000000000000",
		)
		.unwrap();

		for (asset_id, amount, name) in non_borrowable_in_isolation {
			let addr = HydraErc20Mapping::encode_evm_address(asset_id);
			let (reason, value) = try_borrow(pool_contract, alice_evm, addr, amount);
			assert!(
				matches!(reason, fp_evm::ExitReason::Revert(_)),
				"{name}: borrow should revert, but got: {:?}",
				reason
			);
			assert_eq!(
				value,
				error_60,
				"{name}: expected ASSET_NOT_BORROWABLE_IN_ISOLATION (error 60), got: {}",
				hex::encode(&value)
			);
		}
	});
}

/// Liquidation should fail when the user's health factor is above 1 (position is healthy).
#[test]
fn gigahdx_liquidation_fails_when_position_is_healthy() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let treasury = BorrowingTreasuryAccount::get();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let treasury_evm = EVMAccounts::evm_address(&treasury);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Alice stakes and borrows a small amount — position is healthy
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);
		borrow(pool_contract, alice_evm, hollar_addr, 1 * HOLLAR_UNITS);

		// Fund treasury
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			treasury.clone(),
			2_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(treasury.clone()),
			1_000_000 * UNITS
		));
		set_use_as_collateral(pool_contract, treasury_evm, sthdx_evm);

		// Verify HF > 1
		let user_data = get_user_account_data(pool_contract, alice_evm).unwrap();
		assert!(
			user_data.health_factor > U256::from(1_000_000_000_000_000_000u128),
			"HF should be > 1"
		);

		// Liquidation should fail — position is healthy
		assert!(Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			1 * HOLLAR_UNITS,
			BoundedVec::new(),
		)
		.is_err());
	});
}

/// Treasury borrow fails when treasury has no collateral in the money market.
#[test]
fn gigahdx_liquidation_fails_when_treasury_has_no_collateral() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Alice stakes, enables collateral, borrows
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);
		borrow(pool_contract, alice_evm, hollar_addr, 5 * HOLLAR_UNITS);

		// Crash price → HF < 1
		let original_price = get_aave_asset_price(sthdx_evm);
		let mock_oracle = deploy_fixed_price_oracle(original_price * 30 / 100);
		set_aave_price_source(sthdx_evm, mock_oracle);

		// Do NOT fund treasury — no collateral to borrow against

		// Liquidation should fail with BorrowFailed
		let result = Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			5 * HOLLAR_UNITS / 2,
			BoundedVec::new(),
		);
		assert!(result.is_err(), "Should fail when treasury has no collateral");
	});
}

#[test]
fn exchange_rate_unchanged_after_liquidation() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let treasury = BorrowingTreasuryAccount::get();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let treasury_evm = EVMAccounts::evm_address(&treasury);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// BOB also stakes — he's an innocent bystander
		let bob = sp_runtime::AccountId32::from(BOB);
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			bob.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(bob.clone()), 5_000 * UNITS));

		// Alice stakes and borrows
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);
		borrow(pool_contract, alice_evm, hollar_addr, 5 * HOLLAR_UNITS);

		// Fund treasury
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			treasury.clone(),
			2_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(treasury.clone()),
			1_000_000 * UNITS
		));
		set_use_as_collateral(pool_contract, treasury_evm, sthdx_evm);

		// Record exchange rate before liquidation
		let rate_before = GigaHdx::exchange_rate();
		let total_hdx_before = GigaHdx::total_hdx();
		let total_st_hdx_before = GigaHdx::total_st_hdx_supply();

		// Crash price and liquidate
		let original_price = get_aave_asset_price(sthdx_evm);
		let mock_oracle = deploy_fixed_price_oracle(original_price * 30 / 100);
		set_aave_price_source(sthdx_evm, mock_oracle);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			5 * HOLLAR_UNITS / 2,
			BoundedVec::new(),
		));

		// Exchange rate must be unchanged — liquidation doesn't touch gigapot or stHDX supply
		let rate_after = GigaHdx::exchange_rate();
		let total_hdx_after = GigaHdx::total_hdx();
		let total_st_hdx_after = GigaHdx::total_st_hdx_supply();

		assert_eq!(
			rate_before, rate_after,
			"Exchange rate must not change after liquidation"
		);
		assert_eq!(
			total_hdx_before, total_hdx_after,
			"Total HDX in gigapot must not change"
		);
		assert_eq!(
			total_st_hdx_before, total_st_hdx_after,
			"Total stHDX supply must not change"
		);
	});
}

#[test]
fn other_users_can_stake_and_unstake_after_liquidation() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let treasury = BorrowingTreasuryAccount::get();

		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			GigaHdx::gigapot_account_id(),
			UNITS,
		));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			BOB
		))));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(AccountId::from(
			CHARLIE
		))));

		let alice_evm = EVMAccounts::evm_address(&alice);
		let treasury_evm = EVMAccounts::evm_address(&treasury);
		let pool_contract = fetch_pool_contract(alice_evm);
		let sthdx_evm = HydraErc20Mapping::encode_evm_address(670);
		let hollar_addr = HydraErc20Mapping::asset_address(HOLLAR);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));

		// Alice stakes and borrows
		let stake_amount = 10_000 * UNITS;
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			alice.clone(),
			100_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(alice.clone()), stake_amount));
		set_use_as_collateral(pool_contract, alice_evm, sthdx_evm);
		borrow(pool_contract, alice_evm, hollar_addr, 5 * HOLLAR_UNITS);

		// Fund treasury
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			treasury.clone(),
			2_000_000 * UNITS
		));
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(treasury.clone()),
			1_000_000 * UNITS
		));
		set_use_as_collateral(pool_contract, treasury_evm, sthdx_evm);

		// Crash price and liquidate Alice
		let original_price = get_aave_asset_price(sthdx_evm);
		let mock_oracle = deploy_fixed_price_oracle(original_price * 30 / 100);
		set_aave_price_source(sthdx_evm, mock_oracle);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(AccountId::from(BOB)),
			GIGAHDX,
			HOLLAR,
			alice_evm,
			5 * HOLLAR_UNITS / 2,
			BoundedVec::new(),
		));

		// Restore price for normal operations
		set_aave_price_source(sthdx_evm, deploy_fixed_price_oracle(original_price));

		// CHARLIE stakes fresh — should work normally
		let charlie = sp_runtime::AccountId32::from(CHARLIE);
		assert_ok!(Balances::force_set_balance(
			RawOrigin::Root.into(),
			charlie.clone(),
			100_000 * UNITS
		));
		let charlie_stake = 5_000 * UNITS;
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(charlie.clone()),
			charlie_stake
		));
		let charlie_gigahdx = Currencies::free_balance(GIGAHDX, &charlie);
		assert!(
			charlie_gigahdx > charlie_stake * 99 / 100,
			"Charlie should receive ~stake_amount GIGAHDX, got: {}",
			charlie_gigahdx
		);

		// CHARLIE unstakes — should get HDX back at correct exchange rate
		assert_ok!(GigaHdx::giga_unstake(
			RuntimeOrigin::signed(charlie.clone()),
			charlie_gigahdx
		));
	});
}
