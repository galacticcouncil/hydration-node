#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::traits::AccountIdConversion;

fn bench_chain_id<T: Config>() -> BoundedVec<u8, <T as pallet_signet::Config>::MaxChainIdLength> {
	let v: Vec<u8> = b"bench-chain".to_vec();
	BoundedVec::try_from(v).expect("bench-chain fits MaxChainIdLength")
}

#[benchmarks(where T: Config)]
mod benches {
	use super::*;
	use alloy_primitives::{Address, U256};
	use alloy_sol_types::SolCall;
	use core::ops::{Add, Mul};
	use sp_core::H160;

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
		let signet_admin: T::AccountId = whitelisted_caller();
		let chain_id = super::bench_chain_id::<T>();

		let pallet_account: T::AccountId = Pallet::<T>::account_id();
		let signet_pallet_account: T::AccountId =
			<T as pallet_signet::Config>::PalletId::get().into_account_truncating();

		let fee_asset = T::FeeAsset::get();
		let faucet_asset = T::FaucetAsset::get();

		<T as pallet::Config>::Currency::set_balance(fee_asset, &signet_admin, 340266920938463463374607431768211455);
		<T as pallet::Config>::Currency::set_balance(
			faucet_asset,
			&signet_admin,
			340282366920938463463374607431768211455,
		);
		<T as pallet::Config>::Currency::set_balance(fee_asset, &pallet_account, 340266920938463463374607431768211455);
		<T as pallet::Config>::Currency::set_balance(
			faucet_asset,
			&pallet_account,
			340282366920938463463374607431768211455,
		);

		let ed_native: BalanceOf<T> = <T as pallet_signet::Config>::Currency::minimum_balance();
		assert_ok!(pallet_signet::Pallet::<T>::initialize(
			RawOrigin::Root.into(),
			signet_admin,
			ed_native,
			chain_id,
		));

		let requester_needed: BalanceOf<T> = ed_native.add(ed_native.mul(10u32.into()));
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&pallet_account, requester_needed);
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&signet_pallet_account, requester_needed);

		let current_faucet_bal: u128 = (u64::MAX - 1) as u128;
		assert_ok!(Pallet::<T>::set_faucet_balance(
			RawOrigin::Root.into(),
			current_faucet_bal
		));

		let caller: T::AccountId = whitelisted_caller();
		let treasury = T::FeeDestination::get();

		let amount: u128 = 100_000;
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
			b"ecdsa",
			b"ethereum",
			b"",
		);

		#[extrinsic_call]
		request_fund(RawOrigin::Signed(caller), to, amount, req_id, tx);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
