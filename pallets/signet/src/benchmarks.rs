use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::traits::Saturating;
use sp_std::vec;

fn setup_config<T: Config>() {
	let deposit: BalanceOf<T> = T::Currency::minimum_balance().saturating_mul(10u32.into());
	let chain_id: BoundedVec<u8, ConstU32<MAX_CHAIN_ID_LENGTH>> =
		BoundedVec::try_from(b"bench-chain".to_vec()).expect("bench-chain fits");

	assert_ok!(Pallet::<T>::set_config(
		RawOrigin::Root.into(),
		deposit,
		128u32,
		100_000u32,
		chain_id,
	));
}

#[benchmarks(where T: Config)]
mod benches {
	use super::*;

	#[benchmark]
	fn set_config() {
		let deposit: BalanceOf<T> = T::Currency::minimum_balance().saturating_mul(10u32.into());
		let chain_id: BoundedVec<u8, ConstU32<MAX_CHAIN_ID_LENGTH>> =
			BoundedVec::try_from(b"bench-chain".to_vec()).expect("bench-chain fits");

		#[extrinsic_call]
		set_config(RawOrigin::Root, deposit, 128u32, 100_000u32, chain_id);

		assert!(SignetConfig::<T>::get().is_some());
	}

	#[benchmark]
	fn withdraw_funds() {
		setup_config::<T>();

		let pallet_account = Pallet::<T>::account_id();
		let amount: BalanceOf<T> = T::Currency::minimum_balance().saturating_mul(100u32.into());
		let _ = T::Currency::deposit_creating(&pallet_account, amount);

		let recipient: T::AccountId = whitelisted_caller();
		let withdraw_amount: BalanceOf<T> = T::Currency::minimum_balance().saturating_mul(50u32.into());

		#[extrinsic_call]
		withdraw_funds(RawOrigin::Root, recipient.clone(), withdraw_amount);

		assert!(T::Currency::free_balance(&recipient) >= withdraw_amount);
	}

	#[benchmark]
	fn sign() {
		setup_config::<T>();

		let requester: T::AccountId = whitelisted_caller();
		let config = SignetConfig::<T>::get().unwrap();
		let fund: BalanceOf<T> = config.signature_deposit.saturating_mul(10u32.into());
		let _ = T::Currency::deposit_creating(&requester, fund);

		let payload: [u8; 32] = [1u8; 32];
		let key_version: u32 = 1;

		let path_vec = vec![1u8; MAX_PATH_LENGTH as usize];
		let algo_vec = vec![2u8; MAX_ALGO_LENGTH as usize];
		let dest_vec = vec![3u8; MAX_DEST_LENGTH as usize];
		let params_vec = vec![4u8; MAX_PARAMS_LENGTH as usize];

		let path: BoundedVec<u8, ConstU32<MAX_PATH_LENGTH>> = BoundedVec::try_from(path_vec).expect("path fits");
		let algo: BoundedVec<u8, ConstU32<MAX_ALGO_LENGTH>> = BoundedVec::try_from(algo_vec).expect("algo fits");
		let dest: BoundedVec<u8, ConstU32<MAX_DEST_LENGTH>> = BoundedVec::try_from(dest_vec).expect("dest fits");
		let params: BoundedVec<u8, ConstU32<MAX_PARAMS_LENGTH>> =
			BoundedVec::try_from(params_vec).expect("params fits");

		#[extrinsic_call]
		sign(
			RawOrigin::Signed(requester.clone()),
			payload,
			key_version,
			path,
			algo,
			dest,
			params,
		);
	}

	#[benchmark]
	fn sign_bidirectional() {
		setup_config::<T>();

		let requester: T::AccountId = whitelisted_caller();
		let config = SignetConfig::<T>::get().unwrap();
		let fund: BalanceOf<T> = config.signature_deposit.saturating_mul(10u32.into());
		let _ = T::Currency::deposit_creating(&requester, fund);

		let tx_bytes = vec![5u8; MAX_TRANSACTION_LENGTH as usize];
		let serialized_transaction: BoundedVec<u8, ConstU32<MAX_TRANSACTION_LENGTH>> =
			BoundedVec::try_from(tx_bytes).expect("tx fits");

		let caip2_id: BoundedVec<u8, ConstU32<64>> =
			BoundedVec::try_from(b"eip155:11155111".to_vec()).expect("caip2 fits");
		let key_version: u32 = 1;

		let path_vec = vec![1u8; MAX_PATH_LENGTH as usize];
		let algo_vec = vec![2u8; MAX_ALGO_LENGTH as usize];
		let dest_vec = vec![3u8; MAX_DEST_LENGTH as usize];
		let params_vec = vec![4u8; MAX_PARAMS_LENGTH as usize];

		let path: BoundedVec<u8, ConstU32<MAX_PATH_LENGTH>> = BoundedVec::try_from(path_vec).expect("path fits");
		let algo: BoundedVec<u8, ConstU32<MAX_ALGO_LENGTH>> = BoundedVec::try_from(algo_vec).expect("algo fits");
		let dest: BoundedVec<u8, ConstU32<MAX_DEST_LENGTH>> = BoundedVec::try_from(dest_vec).expect("dest fits");
		let params: BoundedVec<u8, ConstU32<MAX_PARAMS_LENGTH>> =
			BoundedVec::try_from(params_vec).expect("params fits");

		let output_schema_vec = vec![6u8; MAX_SCHEMA_LENGTH as usize];
		let respond_schema_vec = vec![7u8; MAX_SCHEMA_LENGTH as usize];

		let output_deserialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>> =
			BoundedVec::try_from(output_schema_vec).expect("output schema fits");
		let respond_serialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>> =
			BoundedVec::try_from(respond_schema_vec).expect("respond schema fits");

		#[extrinsic_call]
		sign_bidirectional(
			RawOrigin::Signed(requester.clone()),
			serialized_transaction,
			caip2_id,
			key_version,
			path,
			algo,
			dest,
			params,
			output_deserialization_schema,
			respond_serialization_schema,
		);
	}

	#[benchmark]
	fn respond() {
		let responder: T::AccountId = whitelisted_caller();

		let mut ids: Vec<[u8; 32]> = Vec::with_capacity(MAX_BATCH_SIZE as usize);
		let mut sigs: Vec<Signature> = Vec::with_capacity(MAX_BATCH_SIZE as usize);

		for i in 0..MAX_BATCH_SIZE {
			let mut id = [0u8; 32];
			id[0] = i as u8;
			ids.push(id);

			let sig = Signature {
				big_r: AffinePoint {
					x: [1u8; 32],
					y: [2u8; 32],
				},
				s: [3u8; 32],
				recovery_id: 0,
			};
			sigs.push(sig);
		}

		let request_ids: BoundedVec<[u8; 32], ConstU32<MAX_BATCH_SIZE>> = BoundedVec::try_from(ids).expect("ids fit");
		let signatures: BoundedVec<Signature, ConstU32<MAX_BATCH_SIZE>> = BoundedVec::try_from(sigs).expect("sigs fit");

		#[extrinsic_call]
		respond(RawOrigin::Signed(responder.clone()), request_ids, signatures);
	}

	#[benchmark]
	fn respond_error() {
		let responder: T::AccountId = whitelisted_caller();

		let mut errs: Vec<ErrorResponse> = Vec::with_capacity(MAX_BATCH_SIZE as usize);

		for i in 0..MAX_BATCH_SIZE {
			let mut id = [0u8; 32];
			id[0] = i as u8;

			let msg_vec = vec![9u8; MAX_ERROR_MESSAGE_LENGTH as usize];
			let error_message: BoundedVec<u8, ConstU32<MAX_ERROR_MESSAGE_LENGTH>> =
				BoundedVec::try_from(msg_vec).expect("msg fits");

			errs.push(ErrorResponse {
				request_id: id,
				error_message,
			});
		}

		let errors: BoundedVec<ErrorResponse, ConstU32<MAX_BATCH_SIZE>> =
			BoundedVec::try_from(errs).expect("errors fit");

		#[extrinsic_call]
		respond_error(RawOrigin::Signed(responder.clone()), errors);
	}

	#[benchmark]
	fn respond_bidirectional() {
		let responder: T::AccountId = whitelisted_caller();

		let request_id: [u8; 32] = [7u8; 32];
		let output_vec = vec![8u8; MAX_SERIALIZED_OUTPUT_LENGTH as usize];
		let serialized_output: BoundedVec<u8, ConstU32<MAX_SERIALIZED_OUTPUT_LENGTH>> =
			BoundedVec::try_from(output_vec).expect("out fits");

		let signature = Signature {
			big_r: AffinePoint {
				x: [1u8; 32],
				y: [2u8; 32],
			},
			s: [3u8; 32],
			recovery_id: 0,
		};

		#[extrinsic_call]
		respond_bidirectional(
			RawOrigin::Signed(responder.clone()),
			request_id,
			serialized_output,
			signature,
		);
	}

	#[benchmark]
	fn pause() {
		setup_config::<T>();

		#[extrinsic_call]
		pause(RawOrigin::Root);

		assert!(SignetConfig::<T>::get().unwrap().paused);
	}

	#[benchmark]
	fn unpause() {
		setup_config::<T>();
		SignetConfig::<T>::mutate(|c| c.as_mut().unwrap().paused = true);

		#[extrinsic_call]
		unpause(RawOrigin::Root);

		assert!(!SignetConfig::<T>::get().unwrap().paused);
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
