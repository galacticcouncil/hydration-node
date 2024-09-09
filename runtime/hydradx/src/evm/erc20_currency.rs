use crate::evm::executor::{CallResult, Executor};
use crate::evm::{EvmAccounts, EvmAddress};
use ethabi::ethereum_types::BigEndianHash;
use evm::ExitReason::Succeed;
use evm::ExitSucceed::Returned;
use evm::{ExitReason, ExitSucceed};
use frame_support::{dispatch::DispatchResult, fail, pallet_prelude::*};
use hydradx_traits::evm::{CallContext, InspectEvmAccounts, ERC20, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::MultiCurrency;
use pallet_currencies::{Config, Error};
use primitives::Balance;
use sp_core::crypto::AccountId32;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{CheckedConversion, Zero};
use sp_runtime::{DispatchError, SaturatedConversion};
use sp_std::vec::Vec;
use scale_info::prelude::format;
use sp_std::boxed::Box;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	Name = "name()",
	Symbol = "symbol()",
	Decimals = "decimals()",
	TotalSupply = "totalSupply()",
	BalanceOf = "balanceOf(address)",
	Allowance = "allowance(address,address)",
	Transfer = "transfer(address,uint256)",
	Approve = "approve(address,uint256)",
	TransferFrom = "transferFrom(address,address,uint256)",
}
type BalanceOf<T> = <<T as pallet_evm::Config>::Currency as frame_support::traits::Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

pub struct Erc20Currency<T>(sp_std::marker::PhantomData<T>);

impl<T> ERC20 for Erc20Currency<T>
where
	T: pallet_evm::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	type Balance = Balance;

	fn name(context: CallContext) -> Option<Vec<u8>> {
		let data = Into::<u32>::into(Function::Name).to_be_bytes().to_vec();

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_string(value.as_slice().to_vec()),
			_ => None,
		}
	}

	fn symbol(context: CallContext) -> Option<Vec<u8>> {
		let data = Into::<u32>::into(Function::Symbol).to_be_bytes().to_vec();

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_string(value.as_slice().to_vec()),
			_ => None,
		}
	}

	fn decimals(context: CallContext) -> Option<u8> {
		let data = Into::<u32>::into(Function::Decimals).to_be_bytes().to_vec();

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_integer(value)?.checked_into(),
			_ => None,
		}
	}

	fn total_supply(context: CallContext) -> Balance {
		let data = Into::<u32>::into(Function::TotalSupply).to_be_bytes().to_vec();

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_integer(value).unwrap_or_default().saturated_into(),
			_ => Default::default(),
		}
	}

	fn balance_of(context: CallContext, address: EvmAddress) -> Balance {
		let mut data = Into::<u32>::into(Function::BalanceOf).to_be_bytes().to_vec();
		// address
		data.extend_from_slice(H256::from(address).as_bytes());

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_integer(value).unwrap_or_default().saturated_into(),
			_ => Default::default(),
		}
	}

	fn allowance(context: CallContext, owner: EvmAddress, spender: EvmAddress) -> Balance {
		let mut data = Into::<u32>::into(Function::Allowance).to_be_bytes().to_vec();
		// owner
		data.extend_from_slice(H256::from(owner).as_bytes());
		// spender
		data.extend_from_slice(H256::from(spender).as_bytes());

		let (exit_reason, value) = Executor::<T>::view(context, data, 100_000);
		match exit_reason {
			Succeed(Returned) => decode_integer(value).unwrap_or_default().saturated_into(),
			_ => Default::default(),
		}
	}

	fn approve(context: CallContext, spender: EvmAddress, value: Balance) -> DispatchResult {
		let mut data = Into::<u32>::into(Function::Approve).to_be_bytes().to_vec();
		// spender
		data.extend_from_slice(H256::from(spender).as_bytes());
		// amount
		data.extend_from_slice(H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes());

		handle_result(Executor::<T>::call(context, data, U256::zero(), 200_000))
	}

	// Calls the transfer method on an ERC20 contract using the given context.
	fn transfer(context: CallContext, to: H160, value: Balance) -> DispatchResult {
		let mut data = Into::<u32>::into(Function::Transfer).to_be_bytes().to_vec();
		// to
		data.extend_from_slice(H256::from(to).as_bytes());
		// amount
		data.extend_from_slice(H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes());

		handle_result(Executor::<T>::call(context, data, U256::zero(), 200_000))
	}

	fn transfer_from(context: CallContext, from: EvmAddress, to: EvmAddress, value: Balance) -> DispatchResult {
		let mut data = Into::<u32>::into(Function::TransferFrom).to_be_bytes().to_vec();
		// from
		data.extend_from_slice(H256::from(from).as_bytes());
		// to
		data.extend_from_slice(H256::from(to).as_bytes());
		// amount
		data.extend_from_slice(H256::from_uint(&U256::from(value.saturated_into::<u128>())).as_bytes());

		handle_result(Executor::<T>::call(context, data, U256::zero(), 200_000))
	}
}

fn decode_string(output: Vec<u8>) -> Option<Vec<u8>> {
	// output is 32-byte aligned and consists of 3 parts:
	// - part 1: 32 byte, the offset of its description is passed in the position of
	// the corresponding parameter or return value.
	// - part 2: 32 byte, string length
	// - part 3: string data
	if output.len() < 64 || output.len() % 32 != 0 {
		return None;
	}

	let offset = U256::from_big_endian(&output[0..32]);
	let length = U256::from_big_endian(&output[offset.as_usize()..offset.as_usize() + 32]);
	if output.len() < offset.as_usize() + 32 + length.as_usize() {
		return None;
	}

	let mut data = Vec::new();
	data.extend_from_slice(&output[offset.as_usize() + 32..offset.as_usize() + 32 + length.as_usize()]);

	Some(data.to_vec())
}

fn decode_integer(value: Vec<u8>) -> Option<U256> {
	if value.len() != 32 {
		return None;
	}
	U256::checked_from(value.as_slice())
}

fn handle_result(result: CallResult) -> DispatchResult {
	let (exit_reason, value) = result;
	match exit_reason {
		ExitReason::Succeed(ExitSucceed::Returned) => Ok(()),
		ExitReason::Succeed(ExitSucceed::Stopped) => Ok(()),
		_ => Err(DispatchError::Other(&*Box::leak(
			format!("evm:0x{}", hex::encode(value)).into_boxed_str(),
		))),
	}
}

impl<T> MultiCurrency<T::AccountId> for Erc20Currency<T>
where
	T: Config + pallet_evm::Config,
	pallet_evm_accounts::Pallet<T>: InspectEvmAccounts<T::AccountId>,
	T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
	type CurrencyId = EvmAddress;
	type Balance = Balance;

	fn minimum_balance(_contract: Self::CurrencyId) -> Self::Balance {
		Zero::zero()
	}

	fn total_issuance(contract: Self::CurrencyId) -> Self::Balance {
		<Self as ERC20>::total_supply(CallContext {
			contract,
			sender: Default::default(),
			origin: Default::default(),
		})
	}

	fn total_balance(contract: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		Self::free_balance(contract, who)
	}

	fn free_balance(contract: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		<Self as ERC20>::balance_of(CallContext::new_view(contract), EvmAccounts::<T>::evm_address(who))
	}

	fn ensure_can_withdraw(
		contract: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		ensure!(Self::free_balance(contract, who) >= amount, Error::<T>::BalanceTooLow);
		Ok(())
	}

	fn transfer(
		contract: Self::CurrencyId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		let sender = <pallet_evm_accounts::Pallet<T>>::evm_address(from);
		<Self as ERC20>::transfer(
			CallContext {
				contract,
				sender,
				origin: sender,
			},
			EvmAccounts::<T>::evm_address(to),
			amount,
		)
	}

	fn deposit(_contract: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> sp_runtime::DispatchResult {
		fail!(Error::<T>::NotSupported)
	}

	fn withdraw(
		_contract: Self::CurrencyId,
		_who: &T::AccountId,
		_amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		fail!(Error::<T>::NotSupported)
	}

	fn can_slash(_contract: Self::CurrencyId, _who: &T::AccountId, value: Self::Balance) -> bool {
		value.is_zero()
	}

	fn slash(_contract: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> Self::Balance {
		Default::default()
	}
}
