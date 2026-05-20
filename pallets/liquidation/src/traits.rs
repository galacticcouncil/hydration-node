// SPDX-License-Identifier: Apache-2.0

//! Traits the runtime implements to wire the protocol-funded gigahdx
//! liquidation path into `pallet-liquidation`.

use primitives::{AssetId, Balance, EvmAddress};
use sp_runtime::{DispatchError, DispatchResult};

/// Single integration seam for the protocol-funded gigahdx liquidation path.
///
/// Bundles the asset/account wiring, the live pool-contract lookup, the seize
/// ledger ops and the conviction-lock release that `liquidate_gigahdx` needs,
/// so the pallet carries one `Config` knob instead of six. Wired in the
/// runtime to `pallet_gigahdx` / `pallet_gigahdx_rewards` via an adapter.
pub trait GigaHdxSupport<AccountId> {
	/// Asset id of the GIGAHDX aToken. `liquidate` routes to the gigahdx
	/// branch when `collateral_asset` equals this.
	fn gigahdx_asset_id() -> AssetId;

	/// Asset id of the stHDX underlying held inside the AAVE reserve.
	/// Aave's `liquidationCall` is keyed by the underlying, not the aToken.
	fn sthdx_asset_id() -> AssetId;

	/// Sub-account that holds seized GIGAHDX + matching HDX after a gigahdx
	/// liquidation. Governance disposes later.
	fn liquidation_account() -> AccountId;

	/// Address of the AAVE Pool contract for the GIGAHDX market. Read live
	/// from `pallet_gigahdx` storage.
	fn pool_contract() -> Option<EvmAddress>;

	/// Fold the borrower's accrued GIGAHDX yield into `Stakes.hdx` so the
	/// snapshot reflects the position's true HDX value.
	fn realize_yield(borrower: &AccountId) -> DispatchResult;

	/// Read `(hdx, gigahdx)` before any state mutation.
	fn snapshot_stake(borrower: &AccountId) -> Result<(Balance, Balance), DispatchError>;

	/// Zero `Stakes[borrower].gigahdx` so the lock-manager precompile lets
	/// Aave's internal aToken transfer through.
	fn on_pre_seize(borrower: &AccountId) -> Result<Balance, DispatchError>;

	/// After Aave has moved some aToken, shift the matching HDX, restore the
	/// borrower's residual gigahdx, and refresh locks on both accounts.
	fn on_seize(
		borrower: &AccountId,
		recipient: &AccountId,
		seize_hdx: Balance,
		seize_gigahdx: Balance,
		orig_gigahdx: Balance,
	) -> DispatchResult;

	/// Saturating reduction of the borrower's `pyconvot` lock by `amount`,
	/// so the `on_seize` transfer isn't blocked by a conviction lock.
	fn force_release_vote_lock(borrower: &AccountId, amount: Balance) -> Result<(), DispatchError>;

	/// Borrower's current `debt_asset` debt on the GIGAHDX pool (variable +
	/// stable, interest included). Upper bound used to clamp `debt_to_cover`
	/// so the protocol never borrows more than the position actually owes.
	fn borrower_pool_debt(borrower: &AccountId, debt_asset: AssetId) -> Result<Balance, DispatchError>;

	/// Remove every conviction vote whose staked amount exceeds the borrower's
	/// post-seize residual stake (`max_remaining_hdx`), so the protocol does
	/// not carry governance weight no longer backed by stake. Returns the
	/// number of votes removed.
	fn clear_conflicting_votes(borrower: &AccountId, max_remaining_hdx: Balance) -> Result<u32, DispatchError>;
}
