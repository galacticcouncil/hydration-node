use crate::evm::ExtendedAddressMapping;
use evm::executor::stack::{StackExecutor, StackSubstateMetadata};
use evm::ExitFatal::Other;
use evm::ExitReason;
use fp_evm::Vicinity;
use frame_support::storage::with_transaction;
use frame_support::traits::Get;
use hydradx_traits::evm::{CallContext, EVM};
use pallet_evm::runner::stack::SubstrateStackState;
use pallet_evm::{AccountProvider, AddressMapping, Config};
use primitive_types::{H160, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec;
use sp_std::vec::Vec;

pub struct Executor<R>(sp_std::marker::PhantomData<R>);

pub type CallResult = (ExitReason, Vec<u8>);

pub type BalanceOf<T> =
	<<T as pallet_evm::Config>::Currency as frame_support::traits::Currency<pallet_evm::AccountIdOf<T>>>::Balance;
pub type NonceIdOf<T> = <<T as Config>::AccountProvider as AccountProvider>::Nonce;

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
	T: Config + frame_system::Config + pallet_dispatcher::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
	NonceIdOf<T>: Into<T::Nonce>,
{
	fn call(context: CallContext, data: Vec<u8>, value: U256, gas: u64) -> CallResult {
		let extra_gas = pallet_dispatcher::Pallet::<T>::extra_gas();
		let gas_limit = gas.saturating_add(extra_gas);
		log::trace!(target: "evm::executor", "Call with extra gas {:?}", extra_gas);

		Self::execute(context.origin, gas_limit, |executor| {
			let result = executor.transact_call(context.sender, context.contract, value, data, gas_limit, vec![]);
			log::trace!(target: "evm::executor", "Call executed - used gas {:?}", executor.used_gas());

			if extra_gas > 0 {
				let extra_gas_used = executor.used_gas().saturating_sub(gas);
				log::trace!(target: "evm::executor", "Used extra gas -{:?}", extra_gas_used);
				pallet_dispatcher::Pallet::<T>::decrease_extra_gas(extra_gas_used);
			}

			result
		})
	}

	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> CallResult {
		with_transaction(|| {
			let extra_gas = pallet_dispatcher::Pallet::<T>::extra_gas();
			let gas_limit = gas.saturating_add(extra_gas);
			log::trace!(target: "evm::executor", "Call with extra gas {:?}", extra_gas);

			let result = Self::execute(context.origin, gas_limit, |executor| {
				executor.transact_call(context.sender, context.contract, U256::zero(), data, gas_limit, vec![])
			});
			TransactionOutcome::Rollback(Ok::<CallResult, DispatchError>(result))
		})
		.unwrap_or((ExitReason::Fatal(Other("TransactionalError".into())), Vec::new()))
	}
}
