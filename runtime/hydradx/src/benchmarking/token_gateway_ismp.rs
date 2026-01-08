#![cfg(feature = "runtime-benchmarks")]

use crate::Runtime;
use orml_benchmarking::runtime_benchmarks;
use sp_std::vec;
use sp_std::vec::Vec;

use alloy_primitives::{FixedBytes, U256 as AlloyU256};
use alloy_sol_types::SolValue;
use ismp::host::StateMachine;
use ismp::module::IsmpModule;
use ismp::router::{PostRequest, PostResponse, Request, Response, Timeout};
use pallet_token_gateway as tg;
use pallet_token_gateway::types::Body;
use sp_core::{Get, H256};

// Helper: encode a token-gateway Body and prefix with a selector byte expected by on_accept/on_timeout
fn encode_body(body: Body) -> Vec<u8> {
	let mut data = Vec::with_capacity(1 + 128);
	data.push(0u8); // selector/prefix, pallet slices off the first byte before abi_decode
	data.extend_from_slice(&body.abi_encode());
	data
}

fn host_state_machine() -> StateMachine {
	<Runtime as pallet_ismp::Config>::HostStateMachine::get()
}

fn setup_mappings(
	local_asset_id: <<Runtime as tg::Config>::Assets as frame_support::traits::fungibles::Inspect<
		<Runtime as frame_system::Config>::AccountId,
	>>::AssetId,
	remote_asset_id: H256,
	chain_for_precision: StateMachine,
) {
	// Map remote asset id to local asset id
	tg::LocalAssets::<Runtime>::insert(remote_asset_id, local_asset_id.clone());
	// Set precision mapping to runtime native decimals to keep conversion simple
	let decimals = <Runtime as tg::Config>::Decimals::get();
	tg::Precisions::<Runtime>::insert(local_asset_id.clone(), chain_for_precision, decimals);
	// Mark the local asset as non-native so pallet issues/mints instead of transferring from pallet account
	tg::NativeAssets::<Runtime>::insert(local_asset_id, false);
}

// #[benchmarks(
// 	where
// 		T: pallet_ismp::Config + pallet_token_gateway::Config,
// )]
// mod benchmarks {
// 	use super::*;
//
// 	#[benchmark]
// 	fn tg_on_accept() {
// 		// Environment
// 		let source = StateMachine::Evm(100);
// 		let dest = host_state_machine();
// 		let nonce: u64 = 1;
//
// 		// Configure TokenGateway address for the source chain to pass origin check
// 		let module_addr: Vec<u8> = b"tg-address".to_vec();
// 		tg::TokenGatewayAddresses::<T>::insert(source, module_addr.clone());
//
// 		// Use the native asset id in runtime
// 		let local_asset_id = <T as tg::Config>::NativeAssetId::get();
// 		let remote_asset_id = H256::repeat_byte(7u8);
// 		setup_mappings(local_asset_id.clone(), remote_asset_id, source);
//
// 		// Beneficiary is derived from Body.to
// 		let beneficiary_bytes = [2u8; 32];
// 		let amount_u256 = AlloyU256::from(1_000u64);
// 		let body = Body {
// 			amount: amount_u256,
// 			asset_id: FixedBytes::<32>::from_slice(remote_asset_id.as_fixed_bytes()),
// 			redeem: false,
// 			from: FixedBytes::<32>::from([1u8; 32]),
// 			to: FixedBytes::<32>::from(beneficiary_bytes),
// 		};
// 		let encoded_body = encode_body(body);
//
// 		let post = PostRequest {
// 			source,
// 			dest,
// 			nonce,
// 			from: module_addr,
// 			to: Vec::new(),
// 			timeout_timestamp: 0,
// 			body: encoded_body,
// 		};
//
// 		#[block]
// 		{
// 			tg::Pallet::<T>::default().on_accept(post).unwrap()
// 		}
// 	}
// }

// The runtime_benchmarks macro expects this to exist to generate tests, even if unused here.
#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: crate::NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}

runtime_benchmarks! {
	{ Runtime, pallet_token_gateway }

	on_accept {
		// Environment
		let source = StateMachine::Evm(100);
		let dest = host_state_machine();
		let nonce: u64 = 1;

		// Configure TokenGateway address for the source chain to pass origin check
		let module_addr: Vec<u8> = b"tg-address".to_vec();
		tg::TokenGatewayAddresses::<Runtime>::insert(source, module_addr.clone());

		// Use the native asset id in runtime
		let local_asset_id = <Runtime as tg::Config>::NativeAssetId::get();
		let remote_asset_id = H256::repeat_byte(7u8);
		setup_mappings(local_asset_id.clone(), remote_asset_id, source);

		// Beneficiary is derived from Body.to
		let beneficiary_bytes = [2u8; 32];
		let amount_u256 = AlloyU256::from(1_000u64);
		let body = Body {
			amount: amount_u256,
			asset_id: FixedBytes::<32>::from_slice(remote_asset_id.as_fixed_bytes()),
			redeem: false,
			from: FixedBytes::<32>::from([1u8; 32]),
			to: FixedBytes::<32>::from(beneficiary_bytes),
		};
		let encoded_body = encode_body(body);

		let post = PostRequest {
			source,
			dest,
			nonce,
			from: module_addr,
			to: Vec::new(),
			timeout_timestamp: 0,
			body: encoded_body,
		};
	}: {
		// Call the IsmpModule hook directly
		let _ = tg::Pallet::<Runtime>::default().on_accept(post);
	}

	on_response {
		// Setup a minimal PostRequest (same as in on_accept)
		let source = StateMachine::Evm(100);
		let dest = host_state_machine();
		let nonce: u64 = 1;
		let post = PostRequest {
			source,
			dest,
			nonce,
			from: b"tg-address".to_vec(),
			to: Vec::new(),
			timeout_timestamp: 0,
			body: vec![0u8], // body is unused in on_response path
		};
		let response = Response::Post(PostResponse { post, response: Vec::new(), timeout_timestamp: 0 });
	}: {
		// Should return an error, but we just execute to benchmark code path
		let _ = tg::Pallet::<Runtime>::default().on_response(response);
	}

	on_timeout {
		// Use dest precision mapping in on_timeout path
		let source = host_state_machine(); // source in Timeout::Request is the origin of original request
		let dest = StateMachine::Evm(100);
		let nonce: u64 = 1;

		let local_asset_id = <Runtime as tg::Config>::NativeAssetId::get();
		let remote_asset_id = H256::repeat_byte(9u8);
		// on_timeout uses precision for `dest`
		setup_mappings(local_asset_id.clone(), remote_asset_id, dest);

		// Funds should be refunded to Body.from
		let refund_beneficiary_bytes = [3u8; 32];
		let amount_u256 = AlloyU256::from(1_000u64);
		let body = Body {
			amount: amount_u256,
			asset_id: FixedBytes::<32>::from_slice(remote_asset_id.as_fixed_bytes()),
			redeem: false,
			from: FixedBytes::<32>::from(refund_beneficiary_bytes),
			to: FixedBytes::<32>::from([4u8; 32]),
		};
		let encoded_body = encode_body(body);

		let post = PostRequest {
			source,
			dest,
			nonce,
			from: b"tg-address".to_vec(),
			to: Vec::new(),
			timeout_timestamp: 1, // non-zero but irrelevant here
			body: encoded_body,
		};
		let timeout = Timeout::Request(Request::Post(post));
	}: {
		let _ = tg::Pallet::<Runtime>::default().on_timeout(timeout);
	}
}
