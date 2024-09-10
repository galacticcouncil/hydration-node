use evm::executor::stack::{StackExecutor, StackSubstateMetadata};
use evm::ExitFatal::Other;
use evm::ExitReason;
use fp_evm::Vicinity;
use frame_support::storage::with_transaction;
use frame_support::traits::Get;
use hydradx_traits::evm::{CallContext, EVM};
use pallet_evm::runner::stack::SubstrateStackState;
use pallet_evm::{AddressMapping, Config};
use primitive_types::{H160, U256};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec;
use sp_std::vec::Vec;

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
