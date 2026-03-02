use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Currency, weights::Weight};
use primitives::EvmAddress;
use scale_info::TypeInfo;

pub type Balance = u128;
pub type AssetId = u32;
pub type Bytes32 = [u8; 32];
pub type BalanceOf<T> =
	<<T as pallet_signet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const ECDSA: &[u8] = b"ecdsa";
pub const ETHEREUM: &[u8] = b"ethereum";
pub const SIGNING_PATH: &[u8] = b"dispenser";

/// Complete dispenser state: operational flags, tracked faucet balance,
/// and all governance-controlled parameters — stored as a single entry.
#[derive(Encode, Decode, TypeInfo, Clone, Debug, Default, PartialEq, MaxEncodedLen)]
pub struct DispenserConfigData {
	/// If `true`, all user-facing requests are blocked.
	pub paused: bool,
	/// Tracked ETH balance (in wei) currently available in the external faucet.
	pub faucet_balance_wei: Balance,
	/// EVM address of the external gas faucet contract.
	pub faucet_address: EvmAddress,
	/// Minimum remaining ETH (in wei) that must stay in the faucet after a request.
	pub min_faucet_threshold: Balance,
	/// Minimum amount of faucet asset per request.
	pub min_request: Balance,
	/// Maximum amount of faucet asset per request.
	pub max_dispense: Balance,
	/// Flat fee charged in `FeeAsset` per request.
	pub dispenser_fee: Balance,
}

pub trait WeightInfo {
	fn request_fund() -> Weight;
	fn set_faucet_balance() -> Weight;
	fn pause() -> Weight;
	fn unpause() -> Weight;
	fn set_faucet_params() -> Weight;
}
