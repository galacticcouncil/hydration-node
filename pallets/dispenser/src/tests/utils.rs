use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use alloy_sol_types::SolValue;
use codec::Encode;
use sp_core::Get;
use sp_io::hashing::keccak_256;
use sp_runtime::{AccountId32, BoundedVec};

use crate::tests::Dispenser;
use crate::{
	tests::{MaxChainIdLength, Test},
	EvmTransactionParams,
};

pub fn bounded_chain_id(v: Vec<u8>) -> BoundedVec<u8, MaxChainIdLength> {
	BoundedVec::try_from(v).unwrap()
}

pub fn create_test_tx_params() -> EvmTransactionParams {
	EvmTransactionParams {
		value: 0,
		gas_limit: 100_000,
		max_fee_per_gas: 30_000_000_000,
		max_priority_fee_per_gas: 1_000_000_000,
		nonce: 0,
		chain_id: 1,
	}
}

pub fn create_test_receiver_address() -> primitives::EvmAddress {
	primitives::EvmAddress::from([1u8; 20])
}

pub fn compute_request_id(
	requester: AccountId32,
	to: primitives::EvmAddress,
	amount_wei: u128,
	tx_params: &EvmTransactionParams,
) -> [u8; 32] {
	use sp_core::crypto::Ss58Codec;

	let call = crate::IGasFaucet::fundCall {
		to: Address::from_slice(to.as_bytes()),
		amount: U256::from(amount_wei),
	};

	let faucet_addr = <Test as crate::Config>::FaucetAddress::get();
	let rlp_encoded = pallet_signet::Pallet::<Test>::build_evm_tx(
		frame_system::RawOrigin::Signed(requester.clone()).into(),
		Some(faucet_addr),
		0u128,
		call.abi_encode(),
		tx_params.nonce,
		tx_params.gas_limit,
		tx_params.max_fee_per_gas,
		tx_params.max_priority_fee_per_gas,
		vec![],
		tx_params.chain_id,
	)
	.expect("build_evm_tx should succeed");

	let pallet_account = Dispenser::account_id();
	let encoded_sender = pallet_account.encode();

	let mut account_bytes = [0u8; 32];
	let len = core::cmp::min(encoded_sender.len(), 32);
	account_bytes[..len].copy_from_slice(&encoded_sender[..len]);

	let account_id32 = sp_runtime::AccountId32::from(account_bytes);
	let sender_ss58 = account_id32.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(0));
	let path = {
		let req_scale = requester.encode();
		let mut s = String::from("0x");
		s.push_str(&hex::encode(req_scale));
		s
	};

	// CAIP-2 chain ID format
	let caip2_id = format!("eip155:{}", tx_params.chain_id);

	let packed = (
		sender_ss58.as_str(),
		rlp_encoded.as_slice(),
		caip2_id.as_str(),
		0u32,
		path.as_str(),
		"ecdsa",
		"ethereum",
		"",
	)
		.abi_encode_packed();

	keccak_256(&packed)
}

pub fn acct(n: u8) -> AccountId32 {
	AccountId32::new([n; 32])
}
