use crate::evm::evm_fee::FeeCurrencyOverrideOrDefault;
use crate::evm::runner::WrapRunner;
use crate::evm::{EvmAccounts, WethAssetId};
use crate::types::ShortOraclePrice;
use crate::{DotAssetId, Runtime, XykPaymentAssetSupport};
use core::marker::PhantomData;
use evm::ExitFatal::Other;
use evm::{
	executor::stack::{StackExecutor, StackState as StackStateT, StackSubstateMetadata},
	Context as EvmContext, ExitError, ExitReason,
};
use fp_evm::{CallInfo, Vicinity};
use frame_support::storage::with_transaction;
use frame_support::traits::Get;
use frame_system;
use hydradx_adapters::price::ConvertBalance;
use hydradx_traits::evm::{CallContext, EVM};
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_evm::runner::stack::SubstrateStackState;
use pallet_evm::{
	self, runner::Runner as EvmRunnerT, AccountProvider as EvmAccountProviderT, AddressMapping as EvmAddressMappingT,
	Config as EvmConfigT, Pallet as EvmPallet,
}; // Import the pallet_evm crate
use pallet_evm::{AccountProvider, AddressMapping, Config};
use pallet_evm::{Error, Runner, RunnerError};
use pallet_genesis_history::migration::Weight;
use sp_core::{H160, H256, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec;
use sp_std::vec::Vec;

pub type CallResult = (ExitReason, Vec<u8>);

pub type BalanceOf<T> =
	<<T as pallet_evm::Config>::Currency as frame_support::traits::Currency<pallet_evm::AccountIdOf<T>>>::Balance;
pub type NonceIdOf<T> = <<T as Config>::AccountProvider as AccountProvider>::Nonce;

pub struct Executor<R>(sp_std::marker::PhantomData<R>);

type EVMRunner = WrapRunner<
	Runtime,
	pallet_evm::runner::stack::Runner<Runtime>, // Evm runner that we wrap
	hydradx_adapters::price::FeeAssetBalanceInCurrency<
		Runtime,
		ConvertBalance<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>,
		FeeCurrencyOverrideOrDefault<WethAssetId, EvmAccounts<Runtime>>, // Get account's fee payment asset
		FungibleCurrencies<Runtime>,                                     // Account balance inspector
	>,
>;

impl<T> Executor<T>
where
	T: Config + frame_system::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
	NonceIdOf<T>: Into<T::Nonce>,
{
	pub fn execute<'config, F>(origin: H160, gas: u64, f: F) -> CallResult
	where
		F: for<'precompiles> FnOnce(
			&mut StackExecutor<'config, 'precompiles, SubstrateStackState<'_, 'config, T>, T::PrecompilesType>,
		) -> (ExitReason, Vec<u8>),
	{
		let gas_price = U256::one();
		let vicinity = Vicinity { gas_price, origin };

		let config = <T as Config>::config();
		let precompiles = T::PrecompilesValue::get();
		let metadata = StackSubstateMetadata::new(gas, config);
		let state = SubstrateStackState::new(&vicinity, metadata, None, None);
		let account = T::AddressMapping::into_account_id(origin);
		let nonce = T::AccountProvider::account_nonce(&account.clone().into());
		let mut executor = StackExecutor::new_with_precompiles(state, config, &precompiles);
		let result = f(&mut executor);
		frame_system::Account::<T>::mutate(account.clone(), |a| a.nonce = nonce.into());
		result
	}
}

impl<T> EVM<CallResult> for Executor<T>
where
	T: frame_system::Config + pallet_evm::Config + pallet_dispatcher::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256> + Default,
	NonceIdOf<T>: Into<T::Nonce>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
{
	fn call(context: CallContext, data: Vec<u8>, value: U256, gas: u64) -> CallResult {
		let extra_gas = pallet_dispatcher::Pallet::<T>::extra_gas();
		let gas_limit = gas.saturating_add(extra_gas);
		log::trace!(target: "evm::executor", "Call with extra gas {:?}", extra_gas);

		let source_h160 = context.sender;
		let source_account_id = T::AddressMapping::into_account_id(source_h160);
		let original_nonce = frame_system::Pallet::<T>::account_nonce(source_account_id.clone());

		let evm_config = <T as pallet_evm::Config>::config();

		let call_info_result = EVMRunner::call(
			source_h160,
			context.contract,
			data,
			value,
			gas_limit,
			Some(U256::zero()), // max_fee_per_gas
			None,                // max_priority_fee_per_gas
			None,                // nonce
			vec![],
			true,  // is_transactional
			false, // validate
			None,  // weight_limit
			None,  // proof_size_base_cost
			evm_config,
		);

		frame_system::Account::<T>::mutate(source_account_id.clone(), |a| a.nonce = original_nonce.into());

		match call_info_result {
			Ok(info) => {
				log::trace!(target: "evm::executor", "Call executed - used gas {:?}", info.used_gas);
				if extra_gas > 0 {
					//TODO: this can panic, double check how to  convert to u64
					let extra_gas_used = info.used_gas.standard.as_u64().saturating_sub(gas); //TODO: maybe we need effective her
					log::trace!(target: "evm::executor", "Used extra gas -{:?}", extra_gas_used);
					pallet_dispatcher::Pallet::<T>::decrease_extra_gas(extra_gas_used);
				}
				(info.exit_reason, info.value)
			}
			Err(runner_error) => {
				log::error!(target: "evm_executor", "EVM call failed: {:?}", runner_error.error);
				// Map RunnerError to a generic EVM execution failure
				let exit_reason = ExitReason::Error(ExitError::Other(sp_std::borrow::Cow::Borrowed("EVM Call failed")));
				(exit_reason, Vec::new())
			}
		}
	}

	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> CallResult {
		let extra_gas = pallet_dispatcher::Pallet::<T>::extra_gas();
		let gas_limit = gas.saturating_add(extra_gas);
		log::trace!(target: "evm::executor", "View call with extra gas {:?}", extra_gas);

		let mut extra_gas_used = 0u64;

		let result = with_transaction(|| {
			let result = Self::execute(context.origin, gas_limit, |executor| {
				let result =
					executor.transact_call(context.sender, context.contract, U256::zero(), data, gas_limit, vec![]);
				if extra_gas > 0 {
					extra_gas_used = executor.used_gas().saturating_sub(gas);
					log::trace!(target: "evm::executor", "View used extra gas -{:?}", extra_gas_used);
				}
				result
			});
			TransactionOutcome::Rollback(Ok::<CallResult, DispatchError>(result))
		})
		.unwrap_or((ExitReason::Fatal(Other("TransactionalError".into())), Vec::new()));

		if extra_gas_used > 0 {
			log::trace!(target: "evm::executor", "Used extra gas -{:?}", extra_gas_used);
			pallet_dispatcher::Pallet::<T>::decrease_extra_gas(extra_gas_used);
		}
		result
	}
}
