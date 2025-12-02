use alloy_sol_types::sol;
use frame_support::{traits::Currency, weights::Weight};

pub type Balance = u128;
pub type AssetId = u32;
pub type Bytes32 = [u8; 32];
pub type EvmAddress = [u8; 20];
pub type BalanceOf<T> =
	<<T as pallet_signet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const ECDSA: &[u8] = b"ecdsa";
pub const ETHEREUM: &[u8] = b"ethereum";

pub trait WeightInfo {
	fn initialize() -> Weight;
	fn request_fund() -> Weight;
	fn set_faucet_balance() -> Weight;
	fn pause() -> Weight;
	fn unpause() -> Weight;
}

// Solidity interface for the external EVM gas faucet contract.
//
// The pallet builds a transaction calling `fund(address,uint256)` using this ABI.
sol! {
	#[sol(abi)]
	interface IGasFaucet {
		function fund(address to, uint256 amount) external;
	}
}
