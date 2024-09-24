use evm::executor::stack::{StackExecutor, StackSubstateMetadata};
use evm::ExitFatal::Other;
use evm::ExitReason;
use fp_evm::Vicinity;
use frame_support::storage::with_transaction;
use frame_support::traits::Get;
use hydradx_traits::evm::{CallContext, EVM};
use pallet_evm::runner::stack::{Recorded, SubstrateStackState};
use pallet_evm::{AddressMapping, Config};
use primitive_types::{H160, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec;
use sp_std::vec::Vec;
use evm::backend::{Backend, Basic};
use evm::executor::stack::{StackState};
use evm::{ExitError, Transfer};
use hex_literal::hex;
use sp_core::{H256,};
use crate::evm::precompiles::{is_precompile};

pub struct Executor<R>(sp_std::marker::PhantomData<R>);

pub type CallResult = (ExitReason, Vec<u8>);

type BalanceOf<T> = <<T as pallet_evm::Config>::Currency as frame_support::traits::Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

impl<T> Executor<T>
where
	T: Config + frame_system::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	pub fn execute<'config, F>(origin: H160, gas: u64, f: F) -> CallResult
	where
		F: for<'precompiles> FnOnce(
			&mut StackExecutor<'config, 'precompiles, CustomSubstrateStackState<'_, 'config, T>, T::PrecompilesType>,
		) -> (ExitReason, Vec<u8>),
	{
		let gas_price = U256::one();
		let vicinity = Vicinity { gas_price, origin };

		let config = <T as Config>::config();
		let precompiles = T::PrecompilesValue::get();
		let metadata = StackSubstateMetadata::new(gas, config);
		let inner_state = SubstrateStackState::new(&vicinity, metadata, None, None);
		let state = CustomSubstrateStackState::new(inner_state);
		let account = T::AddressMapping::into_account_id(origin);
		let nonce = frame_system::Account::<T>::get(account.clone()).nonce;
		let mut executor = StackExecutor::new_with_precompiles(state, config, &precompiles);
		let result = f(&mut executor);
		frame_system::Account::<T>::mutate(account, |a| a.nonce = nonce);
		result
	}
}

impl<T> EVM<CallResult> for Executor<T>
where
	T: Config + frame_system::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	fn call(context: CallContext, data: Vec<u8>, value: U256, gas: u64) -> CallResult {
		Self::execute(context.origin, gas, |executor| {
			executor.transact_call(context.sender, context.contract, value, data, gas, vec![])
		})
	}

	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> CallResult {
		with_transaction(|| {
			let result = Self::execute(context.origin, gas, |executor| {
				executor.transact_call(context.sender, context.contract, U256::zero(), data, gas, vec![])
			});
			TransactionOutcome::Rollback(Ok::<CallResult, DispatchError>(result))
		})
		.unwrap_or((ExitReason::Fatal(Other("TransactionalError".into())), Vec::new()))
	}
}



///CustomSubstrateStackState to override some functionalities we need for our runtime
pub struct CustomSubstrateStackState<'vicinity, 'config, T: Config> {
	inner: SubstrateStackState<'vicinity, 'config, T>,
}

impl<'vicinity, 'config, T: Config> CustomSubstrateStackState<'vicinity, 'config, T> {
	pub fn new(inner: SubstrateStackState<'vicinity, 'config, T>) -> Self {
		Self { inner }
	}
}

impl<'vicinity, 'config, T: Config> Backend for CustomSubstrateStackState<'vicinity, 'config, T>
	where
		BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	fn gas_price(&self) -> U256 {
		self.inner.gas_price()
	}

	fn origin(&self) -> H160 {
		self.inner.origin()
	}

	fn block_hash(&self, number: U256) -> H256 {
		self.inner.block_hash(number)
	}

	fn block_number(&self) -> U256 {
		self.inner.block_number()
	}

	fn block_coinbase(&self) -> H160 {
		self.inner.block_coinbase()
	}

	fn block_timestamp(&self) -> U256 {
		self.inner.block_timestamp()
	}

	fn block_difficulty(&self) -> U256 {
		self.inner.block_difficulty()
	}

	fn block_randomness(&self) -> Option<H256> {
		self.inner.block_randomness()
	}

	fn block_gas_limit(&self) -> U256 {
		self.inner.block_gas_limit()
	}

	fn block_base_fee_per_gas(&self) -> U256 {
		self.inner.block_base_fee_per_gas()
	}

	fn chain_id(&self) -> U256 {
		self.inner.chain_id()
	}

	fn exists(&self, address: H160) -> bool {
		self.inner.exists(address)
	}

	fn basic(&self, address: H160) -> Basic {
		self.inner.basic(address)
	}

	fn code(&self, address: H160) -> Vec<u8> {
		if is_precompile(address) {
			hex!["00"].to_vec()
		} else {
			self.inner.code(address)
		}
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		self.inner.storage(address, index)
	}

	fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
		self.inner.original_storage(address, index)
	}
}

impl<'vicinity, 'config, T: Config> StackState<'config> for CustomSubstrateStackState<'vicinity, 'config, T>
	where
		BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	fn metadata(&self) -> &StackSubstateMetadata<'config> {
		self.inner.metadata()
	}

	fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
		self.inner.metadata_mut()
	}

	fn enter(&mut self, gas_limit: u64, is_static: bool) {
		self.inner.enter(gas_limit, is_static)
	}

	fn exit_commit(&mut self) -> Result<(), ExitError> {
		self.inner.exit_commit()
	}

	fn exit_revert(&mut self) -> Result<(), ExitError> {
		self.inner.exit_revert()
	}

	fn exit_discard(&mut self) -> Result<(), ExitError> {
		self.inner.exit_discard()
	}

	fn is_empty(&self, address: H160) -> bool {
		self.inner.is_empty(address)
	}

	fn deleted(&self, address: H160) -> bool {
		self.inner.deleted(address)
	}

	fn inc_nonce(&mut self, address: H160) -> Result<(), ExitError> {
		self.inner.inc_nonce(address)
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) {
		self.inner.set_storage(address, index, value)
	}

	fn reset_storage(&mut self, address: H160) {
		self.inner.reset_storage(address)
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
		self.inner.log(address, topics, data)
	}

	fn set_deleted(&mut self, address: H160) {
		self.inner.set_deleted(address)
	}

	fn set_code(&mut self, address: H160, code: Vec<u8>) {
		self.inner.set_code(address, code)
	}

	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
		self.inner.transfer(transfer)
	}

	fn reset_balance(&mut self, address: H160) {
		self.inner.reset_balance(address)
	}

	fn touch(&mut self, address: H160) {
		self.inner.touch(address)
	}

	fn is_cold(&self, address: H160) -> bool {
		self.inner.is_cold(address)
	}

	fn is_storage_cold(&self, address: H160, key: H256) -> bool {
		self.inner.is_storage_cold(address, key)
	}

	fn code_size(&self, address: H160) -> U256 {
		self.inner.code_size(address)
	}

	fn code_hash(&self, address: H160) -> H256 {
		self.inner.code_hash(address)
	}

	fn record_external_operation(&mut self, op: evm::ExternalOperation) -> Result<(), ExitError> {
		self.inner.record_external_operation(op)
	}

	fn record_external_dynamic_opcode_cost(
		&mut self,
		opcode: evm::Opcode,
		gas_cost: evm::gasometer::GasCost,
		target: evm::gasometer::StorageTarget,
	) -> Result<(), ExitError> {
		self.inner.record_external_dynamic_opcode_cost(opcode, gas_cost, target)
	}

	fn record_external_cost(
		&mut self,
		ref_time: Option<u64>,
		proof_size: Option<u64>,
		storage_growth: Option<u64>,
	) -> Result<(), ExitError> {
		self.inner.record_external_cost(ref_time, proof_size, storage_growth)
	}

	fn refund_external_cost(&mut self, ref_time: Option<u64>, proof_size: Option<u64>) {
		self.inner.refund_external_cost(ref_time, proof_size)
	}
}