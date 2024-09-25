use std::marker::PhantomData;
use evm::executor::stack::{IsPrecompileResult, PrecompileSet, StackExecutor, StackSubstateMetadata};
use evm::ExitFatal::Other;
use evm::ExitReason;
use fp_evm::{CallInfo, CreateInfo, ExecutionInfoV2, FeeCalculator, TransactionValidationError, Vicinity, WeightInfo};
use frame_support::storage::with_transaction;
use frame_support::traits::Get;
use hydradx_traits::evm::{CallContext, EVM};
use pallet_evm::runner::stack::{SubstrateStackState};
use pallet_evm::{AccountCodes, AccountCodesMetadata, AddressMapping, CodeMetadata, Config, Error, Log, OnChargeEVMTransaction, Runner, RunnerError};
use primitive_types::{H160, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec;
use sp_std::vec::Vec;
use evm::backend::{Backend, Basic};
use evm::executor::stack::{StackState};
use evm::{ExitError, Transfer};
use frame_support::weights::Weight;
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
	pub inner: SubstrateStackState<'vicinity, 'config, T>,
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
		let meta = {
			//The original (super) code_size logic is copied, with the only difference that we use our custom code(address) function
			if let Some(meta) = <AccountCodesMetadata<T>>::get(address) {
				meta
			} else {
				let code = self.code(address);

				// If code is empty we return precomputed hash for empty code.
				// We don't store it as this address could get code deployed in the future.
				if code.is_empty() {
					const EMPTY_CODE_HASH: [u8; 32] = hex_literal::hex!(
                    "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                );
					CodeMetadata {
						size: 0,
						hash: EMPTY_CODE_HASH.into(),
					}
				} else {
					let size = code.len() as u64;
					let hash = H256::from(sp_io::hashing::keccak_256(&code));

					let meta = CodeMetadata { size, hash };

					<AccountCodesMetadata<T>>::insert(address, meta);
					meta
				}
			}
		};

		U256::from(meta.size)
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

use pallet_evm::{runner::stack::Runner as StackRunner};


use pallet_evm::runner::Runner as RunnerT;

#[derive(Default)]
pub struct CustomRunner<T: Config> {
	_marker: PhantomData<T>,
}

impl<T: Config> CustomRunner<T>
	where
		BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	#[allow(clippy::let_and_return)]
	fn execute<'config, 'precompiles, F, R>(
		source: H160,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		config: &'config evm::Config,
		precompiles: &'precompiles T::PrecompilesType,
		is_transactional: bool,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
		f: F,
	) -> Result<ExecutionInfoV2<R>, RunnerError<Error<T>>>
		where
			F: FnOnce(
				&mut StackExecutor<
					'config,
					'precompiles,
					CustomSubstrateStackState<'_, 'config, T>,
					T::PrecompilesType,
				>,
			) -> (ExitReason, R),
			R: Default,
	{
		let (base_fee, weight) = T::FeeCalculator::min_gas_price();

		#[cfg(not(feature = "forbid-evm-reentrancy"))]
			let res = Self::execute_inner(
			source,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			config,
			precompiles,
			is_transactional,
			f,
			base_fee,
			weight,
			weight_limit,
			proof_size_base_cost,
		);

		#[cfg(feature = "forbid-evm-reentrancy")]
			let res = IN_EVM::using_once(&mut false, || {
			IN_EVM::with(|in_evm| {
				if *in_evm {
					return Err(RunnerError {
						error: Error::<T>::Reentrancy,
						weight,
					});
				}
				*in_evm = true;
				Ok(())
			})
				.unwrap_or(Ok(()))?;

			sp_core::defer! {
                IN_EVM::with(|in_evm| {
                    *in_evm = false;
                });
            }

			Self::execute_inner(
				source,
				value,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				config,
				precompiles,
				is_transactional,
				f,
				base_fee,
				weight,
				weight_limit,
				proof_size_base_cost,
			)
		});

		res
	}

	fn execute_inner<'config, 'precompiles, F, R>(
		source: H160,
		value: U256,
		mut gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		config: &'config evm::Config,
		precompiles: &'precompiles T::PrecompilesType,
		is_transactional: bool,
		f: F,
		base_fee: U256,
		weight: Weight,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
	) -> Result<ExecutionInfoV2<R>, RunnerError<Error<T>>>
		where
			F: FnOnce(
				&mut StackExecutor<
					'config,
					'precompiles,
					CustomSubstrateStackState<'_, 'config, T>,
					T::PrecompilesType,
				>,
			) -> (ExitReason, R),
			R: Default,
	{
		let maybe_weight_info = WeightInfo::new_from_weight_limit(weight_limit, proof_size_base_cost)
			.map_err(|_| RunnerError {
				error: Error::<T>::GasLimitTooLow,
				weight,
			})?;

		match precompiles.is_precompile(source, gas_limit) {
			IsPrecompileResult::Answer { extra_cost, .. } => {
				gas_limit = gas_limit.saturating_sub(extra_cost);
			}
			IsPrecompileResult::OutOfGas => {
				return Ok(ExecutionInfoV2 {
					exit_reason: ExitError::OutOfGas.into(),
					value: Default::default(),
					used_gas: fp_evm::UsedGas {
						standard: gas_limit.into(),
						effective: gas_limit.into(),
					},
					weight_info: maybe_weight_info,
					logs: Default::default(),
				})
			}
		};

		//TODO: we should use the self::code()?!
		if is_transactional && !<AccountCodes<T>>::get(source).is_empty() {
			return Err(RunnerError {
				error: Error::<T>::TransactionMustComeFromEOA,
				weight,
			});
		}

		let total_fee_per_gas = if is_transactional {
			match (max_fee_per_gas, max_priority_fee_per_gas) {
				(Some(max_fee), _) if max_fee.is_zero() => U256::zero(),
				(Some(_), None) => base_fee,
				(Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
					let actual_priority_fee_per_gas = max_fee_per_gas
						.saturating_sub(base_fee)
						.min(max_priority_fee_per_gas);
					base_fee.saturating_add(actual_priority_fee_per_gas)
				}
				_ => {
					return Err(RunnerError {
						error: Error::<T>::GasPriceTooLow,
						weight,
					})
				}
			}
		} else {
			Default::default()
		};

		let total_fee = total_fee_per_gas
			.checked_mul(U256::from(gas_limit))
			.ok_or(RunnerError {
				error: Error::<T>::FeeOverflow,
				weight,
			})?;

		let fee = T::OnChargeTransaction::withdraw_fee(&source, total_fee)
			.map_err(|e| RunnerError { error: e, weight })?;

		let vicinity = Vicinity {
			gas_price: base_fee,
			origin: source,
		};

		let storage_growth_ratio = T::GasLimitStorageGrowthRatio::get();
		let storage_limit = if storage_growth_ratio > 0 {
			Some(gas_limit.saturating_div(storage_growth_ratio))
		} else {
			None
		};

		let metadata = StackSubstateMetadata::new(gas_limit, config);
		let state = SubstrateStackState::new(&vicinity, metadata, maybe_weight_info, storage_limit);
		let custom_state = CustomSubstrateStackState::new(state);
		let mut executor = StackExecutor::new_with_precompiles(custom_state, config, precompiles);

		let (reason, retv) = f(&mut executor);

		let storage_gas = 0;
		/*
		let storage_gas = match &executor.state().inner.storage_meter {
			Some(storage_meter) => storage_meter.storage_to_gas(storage_growth_ratio),
			None => 0,
		};*/

		let pov_gas = match executor.state().inner.weight_info() {
			Some(weight_info) => weight_info
				.proof_size_usage
				.unwrap_or_default()
				.saturating_mul(T::GasLimitPovSizeRatio::get()),
			None => 0,
		};

		let used_gas = executor.used_gas();
		let effective_gas = U256::from(sp_std::cmp::max(
			sp_std::cmp::max(used_gas, pov_gas),
			storage_gas,
		));

		let actual_fee = effective_gas.saturating_mul(total_fee_per_gas);
		let actual_base_fee = effective_gas.saturating_mul(base_fee);

		log::debug!(
            target: "evm",
            "Execution {:?} [source: {:?}, value: {}, gas_limit: {}, actual_fee: {}, used_gas: {}, effective_gas: {}, base_fee: {}, total_fee_per_gas: {}, is_transactional: {}]",
            reason,
            source,
            value,
            gas_limit,
            actual_fee,
            used_gas,
            effective_gas,
            base_fee,
            total_fee_per_gas,
            is_transactional
        );

		let actual_priority_fee = T::OnChargeTransaction::correct_and_deposit_fee(
			&source,
			actual_fee,
			actual_base_fee,
			fee,
		)
			.map_err(|e| RunnerError { error: e, weight })?;
		T::OnChargeTransaction::pay_priority_fee(actual_priority_fee);

		let state = executor.into_state();

		//TODO: ADJUST
		/*for address in &state.inner.substate.deletes {
			log::debug!(
                target: "evm",
                "Deleting account at {:?}",
                address
            );
			pallet_evm::Pallet::<T>::remove_account(address)
		}*/

		//TODO: adjust
		/*for log in &state.inner.substate.logs {
			log::trace!(
                target: "evm",
                "Inserting log for {:?}, topics ({}) {:?}, data ({}): {:?}]",
                log.address,
                log.topics.len(),
                log.topics,
                log.data.len(),
                log.data
            );
			//TODO: we cant deposit from pallet evm as privde, so we need to do sth else
			/*
			pallet_evm::Pallet::<T>::deposit_event(Event::<T>::Log {
				log: Log {
					address: log.address,
					topics: log.topics.clone(),
					data: log.data.clone(),
				},
			});*/
		}*/

		Ok(ExecutionInfoV2 {
			value: retv,
			exit_reason: reason,
			used_gas: fp_evm::UsedGas {
				standard: used_gas.into(),
				effective: effective_gas,
			},
			weight_info: state.inner.weight_info(),
			logs: vec![] //state.inner.substate.logs, //TODO: adjust
		})
	}
}

impl<T: Config> RunnerT<T> for CustomRunner<T>
	where
		BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	type Error = Error<T>;

	fn validate(
		source: H160,
		target: Option<H160>,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		is_transactional: bool,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
		evm_config: &evm::Config,
	) -> Result<(), RunnerError<Self::Error>> {
		let (base_fee, weight) = T::FeeCalculator::min_gas_price();
		let (source_account, _) = pallet_evm::Pallet::<T>::account_basic(&source);

		fp_evm::CheckEvmTransaction::<Self::Error>::new(
			fp_evm::CheckEvmTransactionConfig {
				evm_config,
				block_gas_limit: T::BlockGasLimit::get(),
				base_fee,
				chain_id: T::ChainId::get(),
				is_transactional,
			},
			fp_evm::CheckEvmTransactionInput {
				chain_id: Some(T::ChainId::get()),
				to: target,
				input,
				nonce: nonce.unwrap_or(source_account.nonce),
				gas_limit: gas_limit.into(),
				gas_price: None,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				value,
				access_list,
			},
			weight_limit,
			proof_size_base_cost,
		)
			.validate_in_block_for(&source_account)
			.and_then(|v| v.with_base_fee())
			.and_then(|v| v.with_balance_for(&source_account))
			.map_err(|error| RunnerError { error, weight })?;
		Ok(())
	}

	fn call(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		is_transactional: bool,
		validate: bool,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
		config: &evm::Config,
	) -> Result<CallInfo, RunnerError<Self::Error>> {
		if validate {
			Self::validate(
				source,
				Some(target),
				input.clone(),
				value,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.clone(),
				is_transactional,
				weight_limit,
				proof_size_base_cost,
				config,
			)?;
		}
		let precompiles = T::PrecompilesValue::get();
		Self::execute(
			source,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			config,
			&precompiles,
			is_transactional,
			weight_limit,
			proof_size_base_cost,
			|executor| executor.transact_call(source, target, value, input, gas_limit, access_list),
		)
	}

	fn create(
		source: H160,
		init: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		is_transactional: bool,
		validate: bool,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
		config: &evm::Config,
	) -> Result<CreateInfo, RunnerError<Self::Error>> {
		if validate {
			Self::validate(
				source,
				None,
				init.clone(),
				value,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.clone(),
				is_transactional,
				weight_limit,
				proof_size_base_cost,
				config,
			)?;
		}
		let precompiles = T::PrecompilesValue::get();
		Self::execute(
			source,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			config,
			&precompiles,
			is_transactional,
			weight_limit,
			proof_size_base_cost,
			|executor| {
				let address = executor.create_address(evm::CreateScheme::Legacy { caller: source });
				let (reason, _) = executor.transact_create(source, value, init, gas_limit, access_list);
				(reason, address)
			},
		)
	}

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
		is_transactional: bool,
		validate: bool,
		weight_limit: Option<Weight>,
		proof_size_base_cost: Option<u64>,
		config: &evm::Config,
	) -> Result<CreateInfo, RunnerError<Self::Error>> {
		if validate {
			Self::validate(
				source,
				None,
				init.clone(),
				value,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.clone(),
				is_transactional,
				weight_limit,
				proof_size_base_cost,
				config,
			)?;
		}
		let precompiles = T::PrecompilesValue::get();
		let code_hash = H256::from(sp_io::hashing::keccak_256(&init));
		Self::execute(
			source,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			config,
			&precompiles,
			is_transactional,
			weight_limit,
			proof_size_base_cost,
			|executor| {
				let address = executor.create_address(evm::CreateScheme::Create2 {
					caller: source,
					code_hash,
					salt,
				});
				let (reason, _) = executor.transact_create2(source, value, init, salt, gas_limit, access_list);
				(reason, address)
			},
		)
	}
}