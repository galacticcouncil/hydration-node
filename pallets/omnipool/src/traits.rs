use crate::types::AssetReserveState;
use frame_support::ensure;
use frame_support::traits::Contains;
use frame_support::weights::Weight;
use hydra_dx_math::ema::EmaPrice;
use hydra_dx_math::omnipool::types::AssetStateChange;
use sp_runtime::traits::{CheckedAdd, CheckedMul, Get, Saturating, Zero};
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128, Permill};
use sp_std::fmt::Debug;

/// Asset In/Out information used in hooks.
pub struct AssetInfo<AssetId, Balance>
where
	Balance: Default + Clone,
{
	pub asset_id: AssetId,
	pub before: AssetReserveState<Balance>,
	pub after: AssetReserveState<Balance>,
	pub delta_changes: AssetStateChange<Balance>,
	pub safe_withdrawal: bool,
}

impl<AssetId, Balance> AssetInfo<AssetId, Balance>
where
	Balance: Default + Clone,
{
	pub fn new(
		asset_id: AssetId,
		before_state: &AssetReserveState<Balance>,
		after_state: &AssetReserveState<Balance>,
		delta_changes: &AssetStateChange<Balance>,
		safe_withdrawal: bool,
	) -> Self {
		Self {
			asset_id,
			before: (*before_state).clone(),
			after: (*after_state).clone(),
			delta_changes: (*delta_changes).clone(),
			safe_withdrawal,
		}
	}
}

pub trait OmnipoolHooks<Origin, AccountId, AssetId, Balance>
where
	Balance: Default + Clone,
{
	type Error;
	fn on_liquidity_changed(origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error>;
	fn on_trade(
		origin: Origin,
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error>;

	fn on_hub_asset_trade(origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error>;

	fn on_liquidity_changed_weight() -> Weight;
	fn on_trade_weight() -> Weight;

	/// Returns used amount
	fn on_trade_fee(
		fee_account: AccountId,
		trader: AccountId,
		asset: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error>;
}

// Default implementation for no-op hooks.
impl<Origin, AccountId, AssetId, Balance> OmnipoolHooks<Origin, AccountId, AssetId, Balance> for ()
where
	Balance: Default + Clone + Zero,
{
	type Error = DispatchError;

	fn on_liquidity_changed(_: Origin, _: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		Ok(Weight::zero())
	}

	fn on_trade(
		_: Origin,
		_: AssetInfo<AssetId, Balance>,
		_: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error> {
		Ok(Weight::zero())
	}

	fn on_hub_asset_trade(_: Origin, _: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		Ok(Weight::zero())
	}

	fn on_liquidity_changed_weight() -> Weight {
		Weight::zero()
	}

	fn on_trade_weight() -> Weight {
		Weight::zero()
	}

	fn on_trade_fee(
		_fee_account: AccountId,
		_trader: AccountId,
		_asset: AssetId,
		_amount: Balance,
	) -> Result<Balance, Self::Error> {
		Ok(Balance::zero())
	}
}

pub trait ExternalPriceProvider<AssetId, Price> {
	type Error;
	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<Price, Self::Error>;

	fn get_price_weight() -> Weight;
}

#[allow(clippy::result_unit_err)]
pub trait ShouldAllow<AccountId, AssetId, Price> {
	fn ensure_price(who: &AccountId, asset_a: AssetId, asset_b: AssetId, current_price: Price) -> Result<(), ()>;
}

#[impl_trait_for_tuples::impl_for_tuples(5)]
impl<AccountId, AssetId, Price> ShouldAllow<AccountId, AssetId, Price> for Tuple
where
	AccountId: Debug,
	AssetId: Debug + Copy,
	Price: Debug + Copy,
{
	fn ensure_price(who: &AccountId, asset_a: AssetId, asset_b: AssetId, current_price: Price) -> Result<(), ()> {
		for_tuples!( #(
			match Tuple::ensure_price(who, asset_a, asset_b, current_price) {
				Ok(()) => (),
				Err(_) => {
					log::trace!(
						target: "omnipool::should_allow_price_change",
						"did not pass the price check: who: {:?}, asset_a: {:?}, asset_b: {:?}, current_prie: {:?}",
						who,
						asset_a,
						asset_b,
						current_price,
					);
					return Err(());
				},
			}
		)* );

		Ok(())
	}
}

/// Ensures that the price is within the bounds of the current spot price and the external oracle price.
pub struct EnsurePriceWithin<AccountId, AssetId, ExternalOracle, MaxAllowed, WhitelistedAccounts>(
	sp_std::marker::PhantomData<(AccountId, AssetId, ExternalOracle, MaxAllowed, WhitelistedAccounts)>,
);

impl<AccountId, AssetId, ExternalOracle, MaxAllowed, WhitelistedAccounts> ShouldAllow<AccountId, AssetId, EmaPrice>
	for EnsurePriceWithin<AccountId, AssetId, ExternalOracle, MaxAllowed, WhitelistedAccounts>
where
	ExternalOracle: ExternalPriceProvider<AssetId, EmaPrice>,
	MaxAllowed: Get<Permill>,
	WhitelistedAccounts: Contains<AccountId>,
{
	fn ensure_price(who: &AccountId, asset_a: AssetId, asset_b: AssetId, current_price: EmaPrice) -> Result<(), ()> {
		if WhitelistedAccounts::contains(who) {
			return Ok(());
		}

		let max_allowed = FixedU128::from(MaxAllowed::get());

		let oracle_price = ExternalOracle::get_price(asset_a, asset_b).map_err(|_| ())?;
		let external_price = FixedU128::checked_from_rational(oracle_price.n, oracle_price.d).ok_or(())?;
		let current_spot_price = FixedU128::checked_from_rational(current_price.n, current_price.d).ok_or(())?;

		let max_allowed_difference = max_allowed
			.checked_mul(&current_spot_price.checked_add(&external_price).ok_or(())?)
			.ok_or(())?;

		let diff = if current_spot_price >= external_price {
			current_spot_price.saturating_sub(external_price)
		} else {
			external_price.saturating_sub(current_spot_price)
		};

		ensure!(
			diff.checked_mul(&FixedU128::from(2)).ok_or(())? <= max_allowed_difference,
			()
		);
		Ok(())
	}
}
