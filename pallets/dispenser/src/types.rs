use frame_support::{traits::Currency, weights::Weight};

pub type Balance = u128;
pub type AssetId = u32;
pub type Bytes32 = [u8; 32];
pub type BalanceOf<T> =
	<<T as pallet_signet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const ECDSA: &[u8] = b"ecdsa";
pub const ETHEREUM: &[u8] = b"ethereum";

/// Fixed signing derivation path — all dispenser requests use the same
/// MPC-derived key so that only one EVM wallet needs to be funded and
/// whitelisted on the faucet contract.
pub const SIGNING_PATH: &[u8] = b"dispenser";

pub trait WeightInfo {
	fn request_fund() -> Weight;
	fn set_config() -> Weight;
	fn pause() -> Weight;
	fn unpause() -> Weight;
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
	fn register_asset(asset_id: AssetId, min_balance: Balance) -> sp_runtime::DispatchResult;
	fn mint(asset_id: AssetId, who: &AccountId, amount: Balance) -> sp_runtime::DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
	fn register_asset(_asset_id: AssetId, _min_balance: Balance) -> sp_runtime::DispatchResult {
		Ok(())
	}
	fn mint(_asset_id: AssetId, _who: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult {
		Ok(())
	}
}
