use frame_support::dispatch::PostDispatchInfo;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::sp_runtime::DispatchResult;
use sp_core::{H160, H256, U256};
use sp_std::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentInfo<Balance, AssetId, Price> {
	Native(Balance),
	NonNative(Balance, AssetId, Price),
}

/// Handler for dealing with fees
pub trait DepositFee<AccountId, AssetId, Balance> {
	fn deposit_fee(who: &AccountId, currency: AssetId, amount: Balance) -> DispatchResult;
}

pub trait EVMPermit {
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
	) -> DispatchResult;

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
	) -> DispatchResultWithPostInfo;
}

impl EVMPermit for () {
	fn validate_permit(
		_source: H160,
		_target: H160,
		_input: Vec<u8>,
		_value: U256,
		_gas_limit: u64,
		_max_fee_per_gas: U256,
		_nonce: U256,
		_deadline: U256,
		_v: u8,
		_r: H256,
		_s: H256,
	) -> DispatchResult {
		Ok(())
	}

	fn dispatch_permit(
		_source: H160,
		_target: H160,
		_input: Vec<u8>,
		_value: U256,
		_gas_limit: u64,
		_max_fee_per_gas: Option<U256>,
		_max_priority_fee_per_gas: Option<U256>,
		_nonce: Option<U256>,
		_access_list: Vec<(H160, Vec<H256>)>,
	) -> DispatchResultWithPostInfo {
		Ok(PostDispatchInfo::default())
	}
}
