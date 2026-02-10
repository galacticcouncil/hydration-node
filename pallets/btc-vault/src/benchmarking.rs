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

type BalanceOf<T> = <<T as pallet_signet::Config>::Currency as frame_support::traits::Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

#[benchmarks(where T: Config)]
mod benches {
	use super::*;
	use core::ops::{Add, Mul};
	use frame_support::traits::Currency;

	#[benchmark]
	fn pause() {
		PalletConfig::<T>::put(PalletConfigData { paused: false });

		#[extrinsic_call]
		pause(RawOrigin::Root);

		assert!(PalletConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn unpause() {
		PalletConfig::<T>::put(PalletConfigData { paused: true });

		#[extrinsic_call]
		unpause(RawOrigin::Root);

		assert!(!PalletConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn request_deposit() {
		let signet_admin: T::AccountId = whitelisted_caller();
		let chain_id = super::bench_chain_id::<T>();

		let pallet_account: T::AccountId = Pallet::<T>::account_id();
		let signet_pallet_account: T::AccountId =
			<T as pallet_signet::Config>::PalletId::get().into_account_truncating();

		let ed_native: BalanceOf<T> = <T as pallet_signet::Config>::Currency::minimum_balance();
		assert_ok!(pallet_signet::Pallet::<T>::initialize(
			RawOrigin::Root.into(),
			signet_admin,
			ed_native,
			chain_id,
		));

		let requester_needed: BalanceOf<T> = ed_native.add(ed_native.mul(10u32.into()));
		let _ =
			<T as pallet_signet::Config>::Currency::deposit_creating(&pallet_account, requester_needed);
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(
			&signet_pallet_account,
			requester_needed,
		);

		let caller: T::AccountId = whitelisted_caller();
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&caller, requester_needed);

		let vault_pubkey_hash = T::VaultPubkeyHash::get();
		let mut vault_script = Vec::with_capacity(22);
		vault_script.push(0x00);
		vault_script.push(0x14);
		vault_script.extend_from_slice(&vault_pubkey_hash);

		let input = pallet_signet::UtxoInput {
			txid: [1u8; 32],
			vout: 0,
			value: 100_000,
			script_pubkey: BoundedVec::try_from(vault_script.clone()).unwrap(),
			sequence: 0xffffffff,
		};
		let inputs: BoundedVec<pallet_signet::UtxoInput, T::MaxInputs> =
			BoundedVec::try_from(vec![input]).unwrap();

		let output = pallet_signet::BitcoinOutput {
			value: 90_000,
			script_pubkey: BoundedVec::try_from(vault_script).unwrap(),
		};
		let outputs: BoundedVec<pallet_signet::BitcoinOutput, T::MaxOutputs> =
			BoundedVec::try_from(vec![output]).unwrap();

		let lock_time = 0u32;

		let txid = pallet_signet::Pallet::<T>::get_txid(
			RawOrigin::Signed(caller.clone()).into(),
			inputs.clone(),
			outputs.clone(),
			lock_time,
		)
		.expect("get_txid ok in benchmark");

		let path: Vec<u8> = {
			let enc = pallet_account.encode();
			let mut s = Vec::with_capacity(2 + enc.len() * 2);
			s.extend_from_slice(b"0x");
			s.extend_from_slice(hex::encode(enc).as_bytes());
			s
		};

		let request_id = Pallet::<T>::generate_request_id(
			&pallet_account,
			txid.as_ref(),
			T::BitcoinCaip2::get(),
			T::KeyVersion::get(),
			&path,
			ECDSA,
			BITCOIN,
			b"",
		);

		#[extrinsic_call]
		request_deposit(RawOrigin::Signed(caller), request_id, inputs, outputs, lock_time);
	}

	#[benchmark]
	fn claim_deposit() {
		let caller: T::AccountId = whitelisted_caller();

		let request_id: Bytes32 = [42u8; 32];
		let amount_sats: u64 = 50_000;
		PendingDeposits::<T>::insert(
			request_id,
			PendingDepositData {
				requester: caller.clone(),
				amount_sats,
				txid: [1u8; 32],
				path: BoundedVec::try_from(b"0xdeadbeef".to_vec()).unwrap(),
			},
		);

		let serialized_output: BoundedVec<u8, ConstU32<{ MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		let dummy_signature = pallet_signet::Signature {
			big_r: pallet_signet::AffinePoint {
				x: [0u8; 32],
				y: [0u8; 32],
			},
			s: [0u8; 32],
			recovery_id: 0,
		};

		#[extrinsic_call]
		claim_deposit(RawOrigin::Signed(caller.clone()), request_id, serialized_output, dummy_signature);

		assert_eq!(UserBalances::<T>::get(&caller), amount_sats);
		assert!(PendingDeposits::<T>::get(request_id).is_none());
	}

	#[benchmark]
	fn withdraw_btc() {
		let signet_admin: T::AccountId = whitelisted_caller();
		let chain_id = super::bench_chain_id::<T>();

		let pallet_account: T::AccountId = Pallet::<T>::account_id();
		let signet_pallet_account: T::AccountId =
			<T as pallet_signet::Config>::PalletId::get().into_account_truncating();

		let ed_native: BalanceOf<T> = <T as pallet_signet::Config>::Currency::minimum_balance();
		assert_ok!(pallet_signet::Pallet::<T>::initialize(
			RawOrigin::Root.into(),
			signet_admin,
			ed_native,
			chain_id,
		));

		let requester_needed: BalanceOf<T> = ed_native.add(ed_native.mul(10u32.into()));
		let _ =
			<T as pallet_signet::Config>::Currency::deposit_creating(&pallet_account, requester_needed);
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(
			&signet_pallet_account,
			requester_needed,
		);

		let caller: T::AccountId = whitelisted_caller();
		let _ = <T as pallet_signet::Config>::Currency::deposit_creating(&caller, requester_needed);

		let vault_pubkey_hash = T::VaultPubkeyHash::get();
		let mut vault_script = Vec::with_capacity(22);
		vault_script.push(0x00);
		vault_script.push(0x14);
		vault_script.extend_from_slice(&vault_pubkey_hash);

		let recipient_script = vault_script.clone();

		let input = pallet_signet::UtxoInput {
			txid: [1u8; 32],
			vout: 0,
			value: 100_000,
			script_pubkey: BoundedVec::try_from(vault_script.clone()).unwrap(),
			sequence: 0xffffffff,
		};
		let inputs: BoundedVec<pallet_signet::UtxoInput, T::MaxInputs> =
			BoundedVec::try_from(vec![input]).unwrap();

		let output = pallet_signet::BitcoinOutput {
			value: 90_000,
			script_pubkey: BoundedVec::try_from(vault_script).unwrap(),
		};
		let outputs: BoundedVec<pallet_signet::BitcoinOutput, T::MaxOutputs> =
			BoundedVec::try_from(vec![output]).unwrap();

		let lock_time = 0u32;
		let amount = 90_000u64;

		// Give user some BTC balance to withdraw
		UserBalances::<T>::insert(&caller, amount);

		let txid = pallet_signet::Pallet::<T>::get_txid(
			RawOrigin::Signed(caller.clone()).into(),
			inputs.clone(),
			outputs.clone(),
			lock_time,
		)
		.expect("get_txid ok in benchmark");

		let request_id = Pallet::<T>::generate_request_id(
			&pallet_account,
			txid.as_ref(),
			T::BitcoinCaip2::get(),
			T::KeyVersion::get(),
			WITHDRAWAL_PATH,
			ECDSA,
			BITCOIN,
			b"",
		);

		#[extrinsic_call]
		withdraw_btc(
			RawOrigin::Signed(caller.clone()),
			request_id,
			amount,
			BoundedVec::try_from(recipient_script).unwrap(),
			inputs,
			outputs,
			lock_time,
		);

		assert_eq!(UserBalances::<T>::get(&caller), 0);
		assert!(PendingWithdrawals::<T>::get(request_id).is_some());
	}

	#[benchmark]
	fn complete_withdraw_btc() {
		let caller: T::AccountId = whitelisted_caller();

		let request_id: Bytes32 = [42u8; 32];
		let amount_sats: u64 = 50_000;
		UserBalances::<T>::insert(&caller, 100_000u64);
		PendingWithdrawals::<T>::insert(
			request_id,
			PendingWithdrawalData {
				requester: caller.clone(),
				amount_sats,
			},
		);

		let serialized_output: BoundedVec<u8, ConstU32<{ MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		let dummy_signature = pallet_signet::Signature {
			big_r: pallet_signet::AffinePoint {
				x: [0u8; 32],
				y: [0u8; 32],
			},
			s: [0u8; 32],
			recovery_id: 0,
		};

		#[extrinsic_call]
		complete_withdraw_btc(RawOrigin::Signed(caller.clone()), request_id, serialized_output, dummy_signature);

		assert!(PendingWithdrawals::<T>::get(request_id).is_none());
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
