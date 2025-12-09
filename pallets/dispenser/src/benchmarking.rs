#![cfg(feature = "runtime-benchmarks")]

use super::*;
use core::ops::Add;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, storage::with_transaction};
use frame_system::RawOrigin;
use hydradx_traits::{AssetKind, Create};
use pallet_asset_registry as asset_registry;
use sp_runtime::{traits::AccountIdConversion, TransactionOutcome};
use sp_std::ops::Mul;

fn bench_chain_id<T: Config>() -> BoundedVec<u8, <T as pallet_signet::Config>::MaxChainIdLength> {
	let v: Vec<u8> = b"bench-chain".to_vec();
	BoundedVec::try_from(v).expect("bench-chain fits MaxChainIdLength")
}
type RegistryStrLimitOf<T> = <T as asset_registry::Config>::StringLimit;

#[benchmarks(where T: Config + asset_registry::Config<AssetId = AssetId>)]
mod benches {
	use super::*;
	use alloy_primitives::{Address, U256};
	use alloy_sol_types::SolCall;
	use sp_core::H160;
	use sp_runtime::traits::Scale;

	#[benchmark]
	fn set_faucet_balance() {
		DispenserConfig::<T>::put(DispenserConfigData { paused: false });
		#[extrinsic_call]
		set_faucet_balance(RawOrigin::Root, 123u128);
		assert_eq!(FaucetBalanceWei::<T>::get(), 123u128);
	}

	#[benchmark]
	fn pause() {
		DispenserConfig::<T>::put(DispenserConfigData { paused: false });

		#[extrinsic_call]
		pause(RawOrigin::Root);

		assert!(DispenserConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn unpause() {
		DispenserConfig::<T>::put(DispenserConfigData { paused: true });

		#[extrinsic_call]
		unpause(RawOrigin::Root);

		assert!(!DispenserConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn request_fund() {
		// Make sure faucet asset exists with sane ED

		let signet_admin: T::AccountId = whitelisted_caller();
		let chain_id = super::bench_chain_id::<T>();

		let pallet_account: T::AccountId = Pallet::<T>::account_id();
		let signet_pallet_account: T::AccountId =
			<T as pallet_signet::Config>::PalletId::get().into_account_truncating();

		let fee_asset = T::FeeAsset::get();
		let faucet_asset = T::FaucetAsset::get();

		// Initialize SigNet
		let ed_native: BalanceOf<T> = <T as pallet_signet::Config>::Currency::minimum_balance();
		assert_ok!(pallet_signet::Pallet::<T>::initialize(
			RawOrigin::Root.into(),
			signet_admin.clone(),
			ed_native,
			chain_id,
		));

		// Fund SigNet-related accounts
		let requester_needed: BalanceOf<T> = ed_native.add(ed_native.mul(10u32.into()));
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&pallet_account, requester_needed);
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&signet_pallet_account, requester_needed);

		// Set high faucet balance in storage (this is purely virtual balance in your pallet)
		let current_faucet_bal: u128 = u128::MAX - (u32::MAX as u128);
		assert_ok!(Pallet::<T>::set_faucet_balance(
			RawOrigin::Root.into(),
			current_faucet_bal
		));

		DispenserConfig::<T>::put(DispenserConfigData { paused: false });

		let caller: T::AccountId = whitelisted_caller();
		let treasury = T::FeeDestination::get();

		// ---- MINT FEE ASSET (this was already fine) ----
		assert_ok!(<T as pallet::Config>::Currency::mint_into(
			fee_asset,
			&treasury,
			1_000_000_000_000_000_000_000
		));
		assert_ok!(<T as pallet::Config>::Currency::mint_into(
			fee_asset,
			&caller,
			1_000_000_000_000_000_000_000
		));

		// ---- MINT FAUCET ASSET SAFELY ----
		// Get existential deposit for the faucet asset:
		let amount: u128 = 100_000;
		let faucet_ed = <T as pallet::Config>::Currency::minimum_balance(faucet_asset);
		let faucet_needed = Mul::mul(faucet_ed, 1000000000000000000);
		log::info!("--------amount {}", amount);

		assert_ok!(<T as pallet::Config>::Currency::mint_into(
			faucet_asset,
			&caller,
			faucet_needed
		));

		let to: [u8; 20] = [1u8; 20];

		let tx = EvmTransactionParams {
			value: 0,
			gas_limit: 100_000,
			max_fee_per_gas: 30_000_000_000,
			max_priority_fee_per_gas: 1_000_000_000,
			nonce: 0,
			chain_id: 1,
		};

		let call = crate::IGasFaucet::fundCall {
			to: Address::from_slice(&to),
			amount: U256::from(amount),
		};

		let faucet_addr = T::FaucetAddress::get();
		let rlp = pallet_signet::Pallet::<T>::build_evm_tx(
			RawOrigin::Signed(caller.clone()).into(),
			Some(H160::from(faucet_addr)),
			0u128,
			call.abi_encode(),
			tx.nonce,
			tx.gas_limit,
			tx.max_fee_per_gas,
			tx.max_priority_fee_per_gas,
			vec![],
			tx.chain_id,
		)
		.expect("build_evm_tx ok in benchmark");

		let path_bytes: Vec<u8> = {
			let enc = caller.encode();
			let mut s = String::from("0x");
			s.push_str(&hex::encode(enc));
			s.into_bytes()
		};

		let req_id = Pallet::<T>::generate_request_id(
			&Pallet::<T>::account_id(),
			&rlp,
			60,
			0,
			&path_bytes,
			ECDSA,
			ETHEREUM,
			b"",
		);

		#[extrinsic_call]
		request_fund(RawOrigin::Signed(caller), to, amount, req_id, tx);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}

fn ensure_faucet_asset_registered<TBench>()
where
	TBench: Config + asset_registry::Config<AssetId = AssetId>,
{
	let faucet_asset = TBench::FaucetAsset::get();

	// Arbitrary but valid metadata for the benchmark
	let name: BoundedVec<u8, RegistryStrLimitOf<TBench>> = b"WETH".to_vec().try_into().expect("name fits StringLimit");

	// Best-effort: if the asset is already registered, this will likely return an error
	// like AssetAlreadyRegistered, which we can safely ignore in a benchmark setup.
	let _ = asset_registry::Pallet::<TBench>::register_sufficient_asset(
		Some(faucet_asset),
		Some(name),
		AssetKind::Token,
		1,        // existential deposit
		None,     // xcm_asset_id
		Some(18), // decimals
		None,     // foreign_id
		None,     // location
	);
}
