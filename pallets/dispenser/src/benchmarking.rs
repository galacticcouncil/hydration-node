#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::traits::AccountIdConversion;

#[benchmarks(where T: Config)]
mod benches {
	use super::*;
	use alloy_primitives::{Address, U256};
	use alloy_sol_types::SolCall;
	use core::ops::{Add, Mul};
	use frame_support::traits::Currency;
	use frame_support::traits::fungibles::Mutate as FungiblesMutate;

	fn test_config_data() -> DispenserConfigData {
		DispenserConfigData {
			paused: false,
			faucet_balance_wei: (u64::MAX - 1) as u128,
			faucet_address: EvmAddress::from([1u8; 20]),
			min_faucet_threshold: 1,
			min_request: 100,
			max_dispense: 1_000_000_000,
			dispenser_fee: 10,
		}
	}

	#[benchmark]
	fn set_config() {
		#[extrinsic_call]
		set_config(
			RawOrigin::Root,
			EvmAddress::from([1u8; 20]),
			1u128,
			100u128,
			1_000_000_000u128,
			10u128,
			1_000_000_000_000u128,
		);
		assert!(DispenserConfig::<T>::get().is_some());
	}

	#[benchmark]
	fn pause() {
		DispenserConfig::<T>::put(test_config_data());

		#[extrinsic_call]
		pause(RawOrigin::Root);

		assert!(DispenserConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn unpause() {
		let mut cfg = test_config_data();
		cfg.paused = true;
		DispenserConfig::<T>::put(cfg);

		#[extrinsic_call]
		unpause(RawOrigin::Root);

		assert!(!DispenserConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn request_fund() {
		let signet_admin: T::AccountId = whitelisted_caller();

		let pallet_account: T::AccountId = Pallet::<T>::account_id();
		let signet_pallet_account: T::AccountId =
			<T as pallet_signet::Config>::PalletId::get().into_account_truncating();

		let fee_asset = T::FeeAsset::get();
		let faucet_asset = T::FaucetAsset::get();

		let large_balance: Balance = 340_266_920_938_463_463_374_607_431_768_211_455;
		let _ = <T as pallet::Config>::Currency::mint_into(fee_asset, &signet_admin, large_balance);
		let _ = <T as pallet::Config>::Currency::mint_into(faucet_asset, &signet_admin, large_balance);
		let _ = <T as pallet::Config>::Currency::mint_into(fee_asset, &pallet_account, large_balance);
		let _ = <T as pallet::Config>::Currency::mint_into(faucet_asset, &pallet_account, large_balance);

		let ed_native: BalanceOf<T> = <T as pallet_signet::Config>::Currency::minimum_balance();
		let chain_id: BoundedVec<u8, frame_support::traits::ConstU32<{ pallet_signet::MAX_CHAIN_ID_LENGTH }>> =
			BoundedVec::try_from(b"bench-chain".to_vec()).expect("bench-chain fits");

		assert_ok!(pallet_signet::Pallet::<T>::set_config(
			RawOrigin::Root.into(),
			ed_native,
			128u32,
			100_000u32,
			chain_id,
		));

		let requester_needed: BalanceOf<T> = ed_native.add(ed_native.mul(10u32.into()));
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&pallet_account, requester_needed);
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&signet_pallet_account, requester_needed);

		// Set dispenser config with a large faucet balance
		DispenserConfig::<T>::put(test_config_data());

		let caller: T::AccountId = whitelisted_caller();

		let amount: u128 = 100_000;
		let to = EvmAddress::from([1u8; 20]);

		let tx = EvmTransactionParams {
			value: 0,
			gas_limit: 100_000,
			max_fee_per_gas: 30_000_000_000,
			max_priority_fee_per_gas: 1_000_000_000,
			nonce: 0,
			chain_id: 1,
		};

		let call = crate::IGasFaucet::fundCall {
			to: Address::from_slice(to.as_bytes()),
			amount: U256::from(amount),
		};

		let config = DispenserConfig::<T>::get().expect("config must be set");
		let rlp = pallet_signet::Pallet::<T>::build_evm_tx(
			RawOrigin::Signed(caller.clone()).into(),
			Some(config.faucet_address),
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

		let path = SIGNING_PATH.to_vec();

		// CAIP-2 chain ID format
		let caip2_id = alloc::format!("eip155:{}", tx.chain_id);

		let req_id = Pallet::<T>::generate_request_id(
			&Pallet::<T>::account_id(),
			&rlp,
			&caip2_id,
			0,
			&path,
			b"ecdsa",
			b"ethereum",
			b"",
		);

		#[extrinsic_call]
		request_fund(RawOrigin::Signed(caller), to, amount, req_id, tx);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
