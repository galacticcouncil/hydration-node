use sp_arithmetic::{FixedU128, Perquintill};
use sp_std::vec::Vec;

pub type YieldFarmId = u32;
pub type GlobalFarmId = u32;
pub type DepositId = u128;

/// Trait for providing interface for liquidity mining.
pub trait Mutate<AccountId, AssetId, BlockNumber> {
	type Error;

	type AmmPoolId;
	type Balance;
	type Period;
	type LoyaltyCurve;

	/// Create new global farm.
	///
	/// Returns: `(GlobalFarmId, max reward per period)`
	#[allow(clippy::too_many_arguments)]
	fn create_global_farm(
		total_rewards: Self::Balance,
		planned_yielding_periods: Self::Period,
		blocks_per_period: BlockNumber,
		incentivized_asset: AssetId,
		reward_currency: AssetId,
		owner: AccountId,
		yield_per_period: Perquintill,
		min_deposit: Self::Balance,
		price_adjustment: FixedU128,
	) -> Result<(YieldFarmId, Self::Balance), Self::Error>;

	/// Create new global farm without `price_adjustment`.
	///
	/// Returns: `(GlobalFarmId, max reward per period)`
	#[allow(clippy::too_many_arguments)]
	fn create_global_farm_without_price_adjustment(
		total_rewards: Self::Balance,
		planned_yielding_periods: Self::Period,
		blocks_per_period: BlockNumber,
		incentivized_asset: AssetId,
		reward_currency: AssetId,
		owner: AccountId,
		yield_per_period: Perquintill,
		min_deposit: Self::Balance,
	) -> Result<(YieldFarmId, Self::Balance), Self::Error>;

	/// Update price adjustment of the existing global farm.
	fn update_global_farm_price_adjustment(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		price_adjustment: FixedU128,
	) -> Result<(), Self::Error>;

	/// Terminate existing global farm.
	///
	/// Returns: `(reward currency, undistributed rewards, destination account)`
	fn terminate_global_farm(
		who: AccountId,
		global_farm_id: GlobalFarmId,
	) -> Result<(AssetId, Self::Balance, AccountId), Self::Error>;

	/// Crate new yield farm in the global farm.
	///
	/// Returns: `(YieldFarmId)`
	fn create_yield_farm(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		multiplier: FixedU128,
		loyalty_curve: Option<Self::LoyaltyCurve>,
		amm_pool_id: Self::AmmPoolId,
		assets: Vec<AssetId>,
	) -> Result<YieldFarmId, Self::Error>;

	/// Update multiplier of the existing yield farm.
	///
	/// Returns: `(YieldFarmId)`
	fn update_yield_farm_multiplier(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: Self::AmmPoolId,
		multiplier: FixedU128,
	) -> Result<YieldFarmId, Self::Error>;

	/// Stop yield farming for amm pool in the global farm.
	///
	/// Returns: `(YieldFarmId)`
	fn stop_yield_farm(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<YieldFarmId, Self::Error>;

	/// Resume yield farming for amm pool in the global farm.
	fn resume_yield_farm(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
		multiplier: FixedU128,
	) -> Result<(), Self::Error>;

	/// Terminate existing yield farm.
	fn terminate_yield_farm(
		who: AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<(), Self::Error>;

	/// Deposit new LP shares.
	///
	/// Returns: `(DepositId)`
	#[allow(clippy::type_complexity)]
	fn deposit_lp_shares<F: Fn(AssetId, Self::AmmPoolId, Self::Balance) -> Result<Self::Balance, Self::Error>>(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
		shares_amount: Self::Balance,
		get_token_value_of_lp_shares: F,
	) -> Result<DepositId, Self::Error>;

	/// Redeposit already locked LP shares to another yield farm.
	///
	/// Returns: `(redeposited LP shares amount, amm pool id)`
	#[allow(clippy::type_complexity)]
	fn redeposit_lp_shares<F: Fn(AssetId, Self::AmmPoolId, Self::Balance) -> Result<Self::Balance, Self::Error>>(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		deposit_id: DepositId,
		get_token_value_of_lp_shares: F,
	) -> Result<(Self::Balance, Self::AmmPoolId), Self::Error>;

	/// Claim rewards for given deposit.
	///
	/// Returns: `(GlobalFarmId, reward currency, claimed amount, unclaimable amount)`
	#[allow(clippy::type_complexity)]
	fn claim_rewards(
		who: AccountId,
		deposit_id: DepositId,
		yield_farm_id: YieldFarmId,
	) -> Result<(GlobalFarmId, AssetId, Self::Balance, Self::Balance), Self::Error>;

	/// Withdraw LP shares from yield farm. Function attempts to claim rewards for `who` if farm is
	/// claimable.
	///
	/// Returns: `(withdrawn amount, Option<(reward currency, claimed amount, unclaimable amount>, true if deposit was destroyed)`
	#[allow(clippy::type_complexity)]
	fn withdraw_lp_shares(
		who: AccountId,
		deposit_id: DepositId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<(Self::Balance, Option<(AssetId, Self::Balance, Self::Balance)>, bool), Self::Error>;

	/// Returns true if rewards claiming from yield farm is possible.
	fn is_yield_farm_claimable(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> bool;

	/// Returns `Some(global_farm_id)` for given `deposit_id` and `yield_farm_id` or `None`.
	fn get_global_farm_id(deposit_id: DepositId, yield_farm_id: YieldFarmId) -> Option<u32>;
}

/// Implementers of this trait provide `price_adjustment` for given `GlobalFarm`.
pub trait PriceAdjustment<GlobalFarm> {
	type Error;
	type PriceAdjustment;

	/// Returns value of `PriceAdjustment` for given `GlobalFarm`.
	fn get(global_farm: &GlobalFarm) -> Result<Self::PriceAdjustment, Self::Error>;
}
