use crate::evm::precompiles;
use fp_evm::FeeCalculator;
use frame_support::dispatch::{Pays, PostDispatchInfo, RawOrigin};
use frame_support::ensure;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::traits::Time;
use frame_system::Origin;
use hydradx_traits::evm::InspectEvmAccounts;
use pallet_evm::{AddressMapping, GasWeightMapping, Runner};
use pallet_genesis_history::migration::Weight;
use pallet_transaction_multi_payment::EVMPermit;
use primitive_types::{H160, H256, U256};
use primitives::AccountId;
use sp_core::crypto::AccountId32;
use sp_io::hashing::keccak_256;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::{DispatchErrorWithPostInfo, DispatchResult};
use sp_std::vec::Vec;

pub struct EvmPermitHandler<R>(sp_std::marker::PhantomData<R>);

impl<R> EVMPermit for EvmPermitHandler<R>
where
	R: frame_system::Config
		+ pallet_evm::Config
		+ pallet_transaction_multi_payment::Config
		+ pallet_evm_accounts::Config
		+ pallet_dynamic_evm_fee::Config,
	R::Nonce: Into<U256>,
	AccountId: From<R::AccountId>,
	R::AccountId: AsRef<[u8; 32]> + frame_support::traits::IsType<AccountId32>,
{
	fn validate_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		deadline: U256,
		v: u8,
		r: H256,
		s: H256,
	) -> DispatchResult {
		let account_id = <R as pallet_evm::Config>::AddressMapping::into_account_id(source);
		let account_nonce = frame_system::Pallet::<R>::account_nonce(&account_id);

		let permit = pallet_evm_precompile_call_permit::CallPermitPrecompile::<R>::generate_permit(
			precompiles::CALLPERMIT,
			source,
			target,
			value,
			input,
			gas_limit,
			account_nonce.into(),
			deadline,
		);

		// Blockchain time is in ms while Ethereum use second timestamps.
		let timestamp: u128 = <R as pallet_evm::Config>::Timestamp::now().unique_saturated_into();
		let timestamp: U256 = U256::from(timestamp / 1000);

		ensure!(
			deadline >= timestamp,
			pallet_transaction_multi_payment::Error::<R>::EvmPermitExpired
		);

		let mut sig = [0u8; 65];
		sig[0..32].copy_from_slice(r.as_bytes());
		sig[32..64].copy_from_slice(s.as_bytes());
		sig[64] = v;
		let signer = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &permit)
			.map_err(|_| pallet_transaction_multi_payment::Error::<R>::EvmPermitInvalid)?;
		let signer = H160::from(H256::from_slice(keccak_256(&signer).as_slice()));
		ensure!(
			signer != H160::zero() && signer == source,
			pallet_transaction_multi_payment::Error::<R>::EvmPermitInvalid
		);

		Ok(())
	}

	fn dispatch_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: U256,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
	) -> DispatchResultWithPostInfo {
		let mut data: [u8; 32] = [0u8; 32];
		data[0..20].copy_from_slice(&source[..]);
		let a = R::AccountId::from(data.into());

		pallet_evm::Pallet::<R>::call(
			RawOrigin::Signed(a).into(),
			source,
			target,
			input,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
		)
	}

	fn gas_price() -> (U256, Weight) {
		pallet_dynamic_evm_fee::Pallet::<R>::min_gas_price()
	}

	fn dispatch_weight(gas_limit: u64) -> Weight {
		let without_base_extrinsic_weight = true;
		<R as pallet_evm::Config>::GasWeightMapping::gas_to_weight(gas_limit, without_base_extrinsic_weight)
	}
}
