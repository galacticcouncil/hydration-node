use codec::{Decode, Encode};
use frame_support::sp_runtime::app_crypto::sp_core::{H160, U256};
use frame_support::sp_runtime::{DispatchResult, RuntimeDebug};
use sp_std::vec::Vec;
pub trait InspectEvmAccounts<AccountId> {
	/// Returns `True` if the account is EVM truncated account.
	fn is_evm_account(account_id: AccountId) -> bool;

	/// get the EVM address from the substrate address.
	fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress;

	/// Get the truncated address from the EVM address.
	fn truncated_account_id(evm_address: EvmAddress) -> AccountId;

	/// Return the Substrate address bound to the EVM account. If not bound, returns `None`.
	fn bound_account_id(evm_address: EvmAddress) -> Option<AccountId>;

	/// Get the Substrate address from the EVM address.
	/// Returns the truncated version of the address if the address wasn't bind.
	fn account_id(evm_address: EvmAddress) -> AccountId;

	/// Returns `True` if the address is allowed to deploy smart contracts.
	fn can_deploy_contracts(evm_address: EvmAddress) -> bool;

	/// Returns `True` if the address is allowed to manage balances and tokens.
	fn is_approved_contract(address: EvmAddress) -> bool;
}

pub type EvmAddress = H160;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
pub struct CallContext {
	pub contract: EvmAddress,
	/// msg.sender
	pub sender: EvmAddress,
	/// tx.origin
	pub origin: EvmAddress,
}

impl CallContext {
	pub fn new(contract: EvmAddress, sender: EvmAddress, origin: EvmAddress) -> Self {
		Self {
			contract,
			sender,
			origin,
		}
	}

	pub fn new_call(contract: EvmAddress, sender: EvmAddress) -> Self {
		Self {
			contract,
			sender,
			origin: sender,
		}
	}

	pub fn new_view(contract: EvmAddress) -> Self {
		Self {
			contract,
			sender: EvmAddress::default(),
			origin: EvmAddress::default(),
		}
	}
}

pub trait EVM<EvmResult> {
	fn call(context: CallContext, data: Vec<u8>, value: U256, gas: u64) -> EvmResult;
	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> EvmResult;
}

/// ERC20 interface adapter
pub trait ERC20 {
	type Balance;

	fn name(context: CallContext) -> Option<Vec<u8>>;
	fn symbol(context: CallContext) -> Option<Vec<u8>>;
	fn decimals(context: CallContext) -> Option<u8>;
	fn total_supply(context: CallContext) -> Self::Balance;
	fn balance_of(context: CallContext, address: EvmAddress) -> Self::Balance;
	fn allowance(context: CallContext, owner: EvmAddress, spender: EvmAddress) -> Self::Balance;
	fn approve(context: CallContext, spender: EvmAddress, value: Self::Balance) -> DispatchResult;
	fn transfer(context: CallContext, to: EvmAddress, value: Self::Balance) -> DispatchResult;
	fn transfer_from(context: CallContext, from: EvmAddress, to: EvmAddress, value: Self::Balance) -> DispatchResult;
}

/// A mapping between AssetId and Erc20 EVM address.
pub trait Erc20Mapping<AssetId> {
	fn encode_evm_address(asset_id: AssetId) -> Option<EvmAddress>;

	fn decode_evm_address(evm_address: EvmAddress) -> Option<AssetId>;
}

/// Money market liquidation interface adapter
pub trait Liquidation {
	type Balance;

	fn liquidate(context: CallContext, to: EvmAddress, value: Self::Balance) -> DispatchResult;
}
