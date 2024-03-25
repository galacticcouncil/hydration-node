use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::ensure;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::traits::{Get, Time};
use hex_literal::hex;
use pallet_evm::{AddressMapping, GasWeightMapping, Runner};
use pallet_transaction_multi_payment::EVMPermit;
use primitive_types::{H160, H256, U256};
use sp_io::hashing::keccak_256;
use sp_runtime::{DispatchErrorWithPostInfo, DispatchResult};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::vec::Vec;
use pallet_dca::pallet;
use precompile_utils::{keccak256, solidity};
use precompile_utils::prelude::{Address, revert};
use primitives::AccountId;
use crate::evm::precompiles;

pub struct EvmPermitHandler<R>(sp_std::marker::PhantomData<R>);

/// EIP712 permit typehash.
pub const PERMIT_TYPEHASH: [u8; 32] = keccak256!(
	"CallPermit(address from,address to,uint256 value,bytes data,uint64 gaslimit\
,uint256 nonce,uint256 deadline)"
);

/// EIP712 permit domain used to compute an individualized domain separator.
const PERMIT_DOMAIN: [u8; 32] = keccak256!(
	"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
);

pub const CALL_DATA_LIMIT: u32 = 2u32.pow(16);


fn compute_domain_separator<R>(address: H160) -> [u8; 32]
where R: pallet_evm::Config{
	let name: H256 = keccak_256(b"Call Permit Precompile").into();
	let version: H256 = keccak256!("1").into();
	let chain_id: U256 = R::ChainId::get().into();

	let domain_separator_inner = precompile_utils::solidity::encode_arguments((
		H256::from(PERMIT_DOMAIN),
		name,
		version,
		chain_id,
		Address(address),
	));

	keccak_256(&domain_separator_inner).into()
}

impl<R> EVMPermit for EvmPermitHandler<R>
where R: frame_system::Config + pallet_evm::Config,
R::Nonce: Into<U256>,
AccountId: From<R::AccountId>,{
	fn validate_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: U256,
		nonce: U256,
		deadline: U256,
		v: u8,
		r: H256,
		s: H256,
	) -> DispatchResult {
		let account_id = <R as pallet_evm::Config>::AddressMapping::into_account_id(source);
		let account_nonce = frame_system::Pallet::<R>::account_nonce(&account_id);

		let domain_separator = compute_domain_separator::<R>(precompiles::DISPATCH_ADDR);

		let permit_content = solidity::encode_arguments((
			H256::from(PERMIT_TYPEHASH),
			Address(source),
			Address(target),
			value,
			// bytes are encoded as the keccak_256 of the content
			H256::from(keccak_256(&input)),
			gas_limit,
			account_nonce.into(),
			deadline,
		));
		let permit_content = keccak_256(&permit_content);
		let mut pre_digest = Vec::with_capacity(2 + 32 + 32);
		pre_digest.extend_from_slice(b"\x19\x01");
		pre_digest.extend_from_slice(&domain_separator);
		pre_digest.extend_from_slice(&permit_content);
		let permit = keccak_256(&pre_digest);

		// Blockchain time is in ms while Ethereum use second timestamps.
		let timestamp: u128 =
			<R as pallet_evm::Config>::Timestamp::now().unique_saturated_into();
		let timestamp: U256 = U256::from(timestamp / 1000);

		//TODO: ??
		//ensure!(deadline >= timestamp, revert("Permit expired"));

		let mut sig = [0u8; 65];
		sig[0..32].copy_from_slice(&r.as_bytes());
		sig[32..64].copy_from_slice(&s.as_bytes());
		sig[64] = v;

		let signer = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &permit)
			.map_err(|_| pallet_evm::Error::<R>::InvalidNonce)?;
		let signer = H160::from(H256::from_slice(keccak_256(&signer).as_slice()));

		//panic!("signer: {:?}", signer);

		//TODO: more reasonable error
		ensure!(
			signer != H160::zero() && signer == source,
			pallet_evm::Error::<R>::Undefined
		);

		Ok(())
	}

	fn dispatch_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
	) -> DispatchResultWithPostInfo {
		let is_transactional = true;
		let validate = true;
		let info = match <R as pallet_evm::Config>::Runner::call(
			source,
			target,
			input,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
			is_transactional,
			validate,
			None,
			None,
			<R as pallet_evm::Config>::config(),
		) {
			Ok(info) => info,
			Err(e) => {
				return Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight: Some(e.weight),
						pays_fee: Pays::Yes,
					},
					error: e.error.into(),
				})
			}
		};
		Ok(PostDispatchInfo {
			actual_weight: {
				let mut gas_to_weight = <R as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
					info.used_gas.standard.unique_saturated_into(),
					true,
				);
				if let Some(weight_info) = info.weight_info {
					if let Some(proof_size_usage) = weight_info.proof_size_usage {
						*gas_to_weight.proof_size_mut() = proof_size_usage;
					}
				}
				Some(gas_to_weight)
			},
			pays_fee: Pays::No,
		})
	}
}
