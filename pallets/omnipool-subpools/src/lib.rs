#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

#[cfg(test)]
mod tests;
mod types;

use crate::types::{AssetDetail, Balance};
use frame_support::pallet_prelude::*;
use frame_support::require_transactional;
use hydra_dx_math::omnipool_subpools::{MigrationDetails, SubpoolState};
use hydra_dx_math::support::traits::{CheckedDivInner, CheckedMulInner, CheckedMulInto, Convert};
use orml_traits::currency::MultiCurrency;
use sp_runtime::traits::CheckedMul;
use sp_runtime::FixedU128;
use sp_std::prelude::*;

pub use pallet::*;
use pallet_omnipool::types::Position;

type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;
type StableswapPallet<T> = pallet_stableswap::Pallet<T>;

type AssetIdOf<T> = <T as pallet_omnipool::Config>::AssetId;
type StableswapAssetIdOf<T> = <T as pallet_stableswap::Config>::AssetId;
type CurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;
	use hydra_dx_math::omnipool::types::{AssetStateChange, BalanceUpdate};
	use pallet_omnipool::types::Tradability;
	use pallet_stableswap::types::AssetLiquidity;
	use sp_runtime::{ArithmeticError, FixedPointNumber, Permill};
	use std::cmp::min;

	#[pallet::pallet]
	#[pallet::generate_store(pub (crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_omnipool::Config + pallet_stableswap::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Checks that an origin has the authority to manage a subpool.
		type AuthorityOrigin: EnsureOrigin<Self::Origin>;
	}

	#[pallet::storage]
	#[pallet::getter(fn migrated_assets)]
	/// Details of asset migrated from Omnipool to a subpool.
	/// Key is id of migrated asset.
	/// Value is tuple of (Subpool id, AssetDetail).
	pub(super) type MigratedAssets<T: Config> =
		StorageMap<_, Blake2_128Concat, AssetIdOf<T>, (StableswapAssetIdOf<T>, AssetDetail), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn subpools)]
	/// Existing subpool IDs.
	pub(super) type Subpools<T: Config> = StorageMap<_, Blake2_128Concat, StableswapAssetIdOf<T>, (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (crate) fn deposit_event)]
	pub enum Event<T: Config> {
		SubpoolCreated {
			id: StableswapAssetIdOf<T>,
			assets: (AssetIdOf<T>, AssetIdOf<T>),
		},
		AssetMigrated {
			asset_id: AssetIdOf<T>,
			pool_id: StableswapAssetIdOf<T>,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		SubpoolNotFound,
		WithdrawAssetNotSpecified,
		NotStableAsset,
		Math,
		Limit,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId:
			Into<<T as pallet_stableswap::Config>::AssetId> + From<<T as pallet_stableswap::Config>::AssetId>,
	{
		/// Create new subpool by migrating 2 assets from Omnipool to new stabelswap subpool.
		///
		/// New subpools must be created from precisely 2 assets.
		///
		/// TODO: add more desc pls
		///
		#[pallet::weight(0)]
		pub fn create_subpool(
			origin: OriginFor<T>,
			share_asset: AssetIdOf<T>,
			asset_a: AssetIdOf<T>,
			asset_b: AssetIdOf<T>,
			share_asset_weight_cap: Permill,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		) -> DispatchResult {
			<T as Config>::AuthorityOrigin::ensure_origin(origin.clone())?;

			// Load state - return AssetNotFound if it does not exist
			let asset_state_a = OmnipoolPallet::<T>::load_asset_state(asset_a)?;
			let asset_state_b = OmnipoolPallet::<T>::load_asset_state(asset_b)?;

			// Create new subpool
			let pool_id = StableswapPallet::<T>::do_create_pool(
				share_asset.into(),
				&[asset_a.into(), asset_b.into()],
				amplification,
				trade_fee,
				withdraw_fee,
			)?;
			let omnipool_account = OmnipoolPallet::<T>::protocol_account();

			// Move liquidity from omnipool account to subpool
			StableswapPallet::<T>::move_liquidity_to_pool(
				&omnipool_account,
				pool_id,
				&[
					AssetLiquidity::<StableswapAssetIdOf<T>> {
						asset_id: asset_a.into(),
						amount: asset_state_a.reserve,
					},
					AssetLiquidity::<StableswapAssetIdOf<T>> {
						asset_id: asset_b.into(),
						amount: asset_state_b.reserve,
					},
				],
			)?;

			let recalculate_protocol_shares = |q: Balance, b: Balance, s: Balance| -> Result<Balance, DispatchError> {
				q.checked_mul_into(&b)
					.ok_or(ArithmeticError::Overflow)?
					.checked_div_inner(&s)
					.ok_or(ArithmeticError::DivisionByZero)?
					.try_to_inner()
					.ok_or(ArithmeticError::Overflow.into())
			};

			// Deposit pool shares to omnipool account
			let hub_reserve = asset_state_a
				.hub_reserve
				.checked_add(asset_state_b.hub_reserve)
				.ok_or(ArithmeticError::Overflow)?;
			let protocol_shares = recalculate_protocol_shares(
				asset_state_a.hub_reserve,
				asset_state_a.protocol_shares,
				asset_state_a.shares,
			)?
			.checked_add(recalculate_protocol_shares(
				asset_state_a.hub_reserve,
				asset_state_a.protocol_shares,
				asset_state_a.shares,
			)?)
			.ok_or(ArithmeticError::Overflow)?;

			// Amount of share provided to omnipool
			let shares = hub_reserve;

			StableswapPallet::<T>::deposit_shares(&omnipool_account, pool_id, shares)?;

			// Remove assets from omnipool
			OmnipoolPallet::<T>::remove_asset(asset_a)?;
			OmnipoolPallet::<T>::remove_asset(asset_b)?;

			// Add Share token to omnipool as another asset - LRNA is Qi + Qj
			OmnipoolPallet::<T>::add_asset(
				pool_id.into(),
				hub_reserve,
				shares,
				protocol_shares,
				share_asset_weight_cap,
				Tradability::default(),
			)?;

			// Remember some stuff to be able to update LP positions later on
			let asset_a_details = AssetDetail {
				price: asset_state_a.price_as_rational(),
				shares: asset_state_a.shares,
				hub_reserve: asset_state_a.hub_reserve,
				share_tokens: asset_state_a.hub_reserve,
			};
			let asset_b_details = AssetDetail {
				price: asset_state_b.price_as_rational(),
				shares: asset_state_b.shares,
				hub_reserve: asset_state_b.hub_reserve,
				share_tokens: asset_state_b.hub_reserve,
			};

			MigratedAssets::<T>::insert(asset_a, (pool_id, asset_a_details));
			MigratedAssets::<T>::insert(asset_b, (pool_id, asset_b_details));
			Subpools::<T>::insert(share_asset.into(), ());

			Self::deposit_event(Event::SubpoolCreated {
				id: pool_id,
				assets: (asset_a, asset_b),
			});

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn migrate_asset_to_subpool(
			origin: OriginFor<T>,
			pool_id: StableswapAssetIdOf<T>,
			asset_id: AssetIdOf<T>,
		) -> DispatchResult {
			<T as Config>::AuthorityOrigin::ensure_origin(origin.clone())?;

			ensure!(Self::subpools(&pool_id).is_some(), Error::<T>::SubpoolNotFound);

			// Load asset state - returns AssetNotFound if it does not exist
			let asset_state = OmnipoolPallet::<T>::load_asset_state(asset_id)?;
			let subpool_state = OmnipoolPallet::<T>::load_asset_state(pool_id.into())?;
			let omnipool_account = OmnipoolPallet::<T>::protocol_account();

			StableswapPallet::<T>::add_asset_to_existing_pool(pool_id, asset_id.into())?;
			StableswapPallet::<T>::move_liquidity_to_pool(
				&omnipool_account,
				pool_id,
				&[AssetLiquidity::<StableswapAssetIdOf<T>> {
					asset_id: asset_id.into(),
					amount: asset_state.reserve,
				}],
			)?;
			OmnipoolPallet::<T>::remove_asset(asset_id)?;

			let share_issuance = CurrencyOf::<T>::total_issuance(pool_id.into());
			let delta_q = asset_state.hub_reserve;

			//TODO: ask Colin about rounding here
			let delta_ps = (|| -> Option<Balance> {
				let p1 = subpool_state
					.shares
					.checked_mul_into(&asset_state.hub_reserve)?
					.checked_div_inner(&subpool_state.hub_reserve)?;
				let p2 = p1
					.checked_mul_inner(&asset_state.protocol_shares)?
					.checked_div_inner(&asset_state.shares)?;
				p2.try_to_inner()
			})()
			.ok_or(ArithmeticError::Overflow)?;

			let delta_s = (|| -> Option<Balance> {
				asset_state
					.hub_reserve
					.checked_mul_into(&subpool_state.shares)?
					.checked_div_inner(&subpool_state.hub_reserve)?
					.try_to_inner()
			})()
			.ok_or(ArithmeticError::Overflow)?;

			let delta_u = (|| -> Option<Balance> {
				asset_state
					.hub_reserve
					.checked_mul_into(&share_issuance)?
					.checked_div_inner(&subpool_state.hub_reserve)?
					.try_to_inner()
			})()
			.ok_or(ArithmeticError::Overflow)?;

			// price = asset price * share_issuance / pool shares
			// price = (hub reserve / reserve ) * share issuance / pool shares
			// price = hub*issuance / reserve * pool shares
			let price_denom = asset_state
				.reserve
				.checked_mul_into(&subpool_state.shares)
				.ok_or(ArithmeticError::Overflow)?
				.fit_to_inner();

			let price_num = asset_state
				.hub_reserve
				.checked_mul_into(&share_issuance)
				.ok_or(ArithmeticError::Overflow)?
				.fit_to_inner();

			OmnipoolPallet::<T>::update_asset_state(
				pool_id.into(),
				AssetStateChange {
					delta_reserve: BalanceUpdate::Increase(0u128),
					delta_hub_reserve: BalanceUpdate::Increase(delta_q),
					delta_shares: BalanceUpdate::Increase(delta_s),
					delta_protocol_shares: BalanceUpdate::Increase(delta_ps),
				},
			)?;
			StableswapPallet::<T>::deposit_shares(&omnipool_account, pool_id, delta_u)?;

			let asset_details = AssetDetail {
				price: (price_num, price_denom),
				shares: asset_state.shares,
				hub_reserve: delta_q,
				share_tokens: delta_u,
			};
			MigratedAssets::<T>::insert(asset_id, (pool_id, asset_details));

			Self::deposit_event(Event::AssetMigrated { asset_id, pool_id });

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn add_liquidity(origin: OriginFor<T>, asset_id: AssetIdOf<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			if let Some((pool_id, _)) = MigratedAssets::<T>::get(&asset_id) {
				let shares = StableswapPallet::<T>::do_add_liquidity(
					&who,
					pool_id,
					&[AssetLiquidity {
						asset_id: asset_id.into(),
						amount,
					}],
				)?;
				OmnipoolPallet::<T>::add_liquidity(origin, pool_id.into(), shares)
			} else {
				OmnipoolPallet::<T>::add_liquidity(origin, asset_id, amount)
			}
		}

		#[pallet::weight(0)]
		pub fn add_liquidity_stable(
			origin: OriginFor<T>,
			asset_id: AssetIdOf<T>,
			amount: Balance,
			mint_nft: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			if let Some((pool_id, _)) = MigratedAssets::<T>::get(&asset_id) {
				let shares = StableswapPallet::<T>::do_add_liquidity(
					&who,
					pool_id,
					&[AssetLiquidity {
						asset_id: asset_id.into(),
						amount,
					}],
				)?;

				if mint_nft {
					OmnipoolPallet::<T>::add_liquidity(origin, pool_id.into(), shares)
				} else {
					Ok(())
				}
			} else {
				Err(Error::<T>::NotStableAsset.into())
			}
		}

		#[pallet::weight(0)]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			position_id: T::PositionItemId,
			share_amount: Balance,
			asset: Option<AssetIdOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let position = OmnipoolPallet::<T>::load_position(position_id, who.clone())?;

			let position = if let Some((pool_id, details)) = MigratedAssets::<T>::get(&position.asset_id) {
				let position = Self::convert_position(pool_id.into(), details, position)?;
				// Store the updated position
				OmnipoolPallet::<T>::set_position(position_id, &position)?;
				position
			} else {
				position
			};

			// Asset should be in isopool, call omnipool::remove_liquidity
			OmnipoolPallet::<T>::remove_liquidity(origin.clone(), position_id, share_amount)?;

			match (Self::subpools(&position.asset_id.into()), asset) {
				(Some(_), Some(withdraw_asset)) => {
					let received = CurrencyOf::<T>::free_balance(position.asset_id, &who);
					StableswapPallet::<T>::remove_liquidity_one_asset(
						origin,
						position.asset_id.into(),
						withdraw_asset.into(),
						received,
					)
				}
				(Some(_), None) => Err(Error::<T>::WithdrawAssetNotSpecified.into()),
				_ => Ok(()),
			}
		}

		#[pallet::weight(0)]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: AssetIdOf<T>,
			asset_out: AssetIdOf<T>,
			amount: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			match (MigratedAssets::<T>::get(asset_in), MigratedAssets::<T>::get(asset_out)) {
				(None, None) => {
					// both assets are omnipool assets
					OmnipoolPallet::<T>::sell(origin, asset_in, asset_out, amount, min_buy_amount)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) if pool_id_in == pool_id_out => {
					// both assets are migrated stable assets and in the same subpool
					StableswapPallet::<T>::sell(
						origin,
						pool_id_in,
						asset_in.into(),
						asset_out.into(),
						amount,
						min_buy_amount,
					)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) => {
					// both assets are migrated stable assets but in the different subpools
					Self::resolve_sell_between_subpools(
						&who,
						asset_in,
						asset_out,
						pool_id_in,
						pool_id_out,
						amount,
						min_buy_amount,
					)
				}
				(Some((pool_id_in, _)), None) => {
					// Selling stable asset and buy omnipool asset
					Self::resolve_mixed_trade_iso_out_given_stable_in(
						&who,
						asset_in,
						asset_out,
						pool_id_in,
						amount,
						min_buy_amount,
					)
				}
				(None, Some((pool_id_out, _))) => {
					// Sell omnipool asset and buy stable asset
					Self::resolve_mixed_trade_stable_out_given_asset_in(
						&who,
						asset_in,
						asset_out,
						pool_id_out,
						amount,
						min_buy_amount,
					)
				}
			}
		}

		#[pallet::weight(0)]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: AssetIdOf<T>,
			asset_in: AssetIdOf<T>,
			amount: Balance,
			max_sell_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			match (MigratedAssets::<T>::get(asset_in), MigratedAssets::<T>::get(asset_out)) {
				(None, None) => {
					// both assets are omnipool assets
					OmnipoolPallet::<T>::buy(origin, asset_out, asset_in, amount, max_sell_amount)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) if pool_id_in == pool_id_out => {
					// both assets are migrated stable assets and in the same subpool
					StableswapPallet::<T>::buy(
						origin,
						pool_id_in,
						asset_out.into(),
						asset_in.into(),
						amount,
						max_sell_amount,
					)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) => {
					// both assets are migrated stable assets but in the different subpools
					Self::resolve_buy_between_subpools(
						&who,
						asset_in,
						asset_out,
						pool_id_in,
						pool_id_out,
						amount,
						max_sell_amount,
					)
				}
				(Some((pool_id_in, _)), None) => {
					// Buy omnipool asset and sell stable asset
					Self::resolve_mixed_trade_stable_in_given_asset_out(
						&who,
						asset_in,
						asset_out,
						pool_id_in,
						amount,
						max_sell_amount,
					)
				}
				(None, Some((pool_id_out, _))) => {
					// Buy stablea _sset and sell omnipool asset
					Self::resolve_mixed_trade_iso_in_given_stable_out(
						&who,
						asset_in,
						asset_out,
						pool_id_out,
						amount,
						max_sell_amount,
					)
				}
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T>
where
	<T as pallet_omnipool::Config>::AssetId:
		Into<<T as pallet_stableswap::Config>::AssetId> + From<<T as pallet_stableswap::Config>::AssetId>,
{
	/// Convert LP Omnipool position to Stableswap subpool position.
	///
	/// New position has asset_id to subpool id.
	fn convert_position(
		pool_id: <T as pallet_omnipool::Config>::AssetId,
		migration_details: AssetDetail,
		position: Position<Balance, <T as pallet_omnipool::Config>::AssetId>,
	) -> Result<Position<Balance, <T as pallet_omnipool::Config>::AssetId>, DispatchError> {
		let converted = hydra_dx_math::omnipool_subpools::convert_position(
			(&position).into(),
			MigrationDetails {
				price: migration_details.price,
				shares: migration_details.shares,
				hub_reserve: migration_details.hub_reserve,
				share_tokens: migration_details.share_tokens,
			},
		)
		.ok_or(Error::<T>::Math)?;

		Ok(Position {
			asset_id: pool_id,
			amount: converted.amount,
			shares: converted.shares,
			price: converted.price,
		})
	}

	/// Resolve buy trade between two different Stableswap subpools.
	#[require_transactional]
	fn resolve_buy_between_subpools(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,
		asset_out: AssetIdOf<T>,
		subpool_id_in: StableswapAssetIdOf<T>,
		subpool_id_out: StableswapAssetIdOf<T>,
		amount_out: Balance,
		max_limit: Balance,
	) -> DispatchResult {
		let subpool_in = StableswapPallet::<T>::get_pool(subpool_id_in)?;
		let subpool_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;

		let idx_in = subpool_in
			.find_asset(asset_in.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;
		let idx_out = subpool_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let share_asset_state_in = OmnipoolPallet::<T>::load_asset_state(subpool_id_in.into())?;
		let share_asset_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;

		let share_issuance_in = CurrencyOf::<T>::total_issuance(subpool_id_in.into());
		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let result = hydra_dx_math::omnipool_subpools::calculate_buy_between_subpools(
			SubpoolState {
				reserves: &subpool_in.balances::<T>(),
				amplification: subpool_in.amplification as u128,
			},
			SubpoolState {
				reserves: &subpool_out.balances::<T>(),
				amplification: subpool_out.amplification as u128,
			},
			idx_in,
			idx_out,
			amount_out,
			&(&share_asset_state_in).into(),
			&(&share_asset_state_out).into(),
			share_issuance_in,
			share_issuance_out,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.asset_in.amount <= max_limit, Error::<T>::Limit);

		// Update subpools - transfer between subpool and who
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&subpool_in.pool_account::<T>(),
			*result.asset_in.amount,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_out.pool_account::<T>(),
			who,
			*result.asset_out.amount,
		)?;

		// Update ispools - mint/burn share asset
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_out.delta_reserve,
		)?;

		<T as pallet_omnipool::Config>::Currency::deposit(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_in.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(
			subpool_id_in.into(),
			subpool_id_out.into(),
			result.iso_pool,
		)?;

		Ok(())
	}

	/// Resolve sell trade between two different Stableswap subpools.
	#[require_transactional]
	fn resolve_sell_between_subpools(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,
		asset_out: AssetIdOf<T>,
		subpool_id_in: StableswapAssetIdOf<T>,
		subpool_id_out: StableswapAssetIdOf<T>,
		amount_in: Balance,
		min_limit: Balance,
	) -> DispatchResult {
		let subpool_in = StableswapPallet::<T>::get_pool(subpool_id_in)?;
		let subpool_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;

		let idx_in = subpool_in
			.find_asset(asset_in.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;
		let idx_out = subpool_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let share_asset_state_in = OmnipoolPallet::<T>::load_asset_state(subpool_id_in.into())?;
		let share_asset_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;

		let share_issuance_in = CurrencyOf::<T>::total_issuance(subpool_id_in.into());
		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let result = hydra_dx_math::omnipool_subpools::calculate_sell_between_subpools(
			SubpoolState {
				reserves: &subpool_in.balances::<T>(),
				amplification: subpool_in.amplification as u128,
			},
			SubpoolState {
				reserves: &subpool_out.balances::<T>(),
				amplification: subpool_out.amplification as u128,
			},
			idx_in,
			idx_out,
			amount_in,
			&(&share_asset_state_in).into(),
			&(&share_asset_state_out).into(),
			share_issuance_in,
			share_issuance_out,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.asset_out.amount >= min_limit, Error::<T>::Limit);

		// Update subpools - transfer between subpool and who
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&subpool_in.pool_account::<T>(),
			*result.asset_in.amount,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_out.pool_account::<T>(),
			who,
			*result.asset_out.amount,
		)?;

		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_out.delta_reserve,
		)?;
		<T as pallet_omnipool::Config>::Currency::deposit(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_in.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(
			subpool_id_in.into(),
			subpool_id_out.into(),
			result.iso_pool,
		)?;

		Ok(())
	}

	/// Resolve sell trade between subpool and Omnipool where asset in is stable asset and asset out is omnipool asset.
	#[require_transactional]
	fn resolve_mixed_trade_iso_out_given_stable_in(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                // stable asset
		asset_out: AssetIdOf<T>,               // omnipool asset
		subpool_id_in: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_in: Balance,
		min_limit: Balance,
	) -> DispatchResult {
		if asset_out == <T as pallet_omnipool::Config>::HubAssetId::get() {
			// LRNA is not allowed to be bought
			return Err(pallet_omnipool::Error::<T>::NotAllowed.into());
		}

		let asset_state_out = OmnipoolPallet::<T>::load_asset_state(asset_out)?;
		let share_state_in = OmnipoolPallet::<T>::load_asset_state(subpool_id_in.into())?;
		let subpool_state_in = StableswapPallet::<T>::get_pool(subpool_id_in)?;

		let share_issuance_in = CurrencyOf::<T>::total_issuance(subpool_id_in.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_state_in.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let idx_in = subpool_state_in
			.find_asset(asset_in.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_iso_out_given_stable_in(
			SubpoolState {
				reserves: &subpool_state_in.balances::<T>(),
				amplification: subpool_state_in.amplification as u128,
			},
			idx_in,
			&(&asset_state_out).into(),
			&(&share_state_in).into(),
			share_issuance_in,
			amount_in,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.isopool.asset_out.delta_reserve >= min_limit, Error::<T>::Limit);

		debug_assert_eq!(
			*result.subpool.amount, amount_in,
			"Returned amount is not equal to amount_in"
		);

		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&subpool_state_in.pool_account::<T>(),
			*result.subpool.amount,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			who,
			*result.isopool.asset_out.delta_reserve,
		)?;

		//TODO: VERIFY only this update subpool share state in omnipool. - might need to burn or mint some shares!
		<T as pallet_omnipool::Config>::Currency::deposit(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_in.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(subpool_id_in.into(), asset_out, result.isopool)?;

		Ok(())
	}

	/// Handle sell trade between subpool and omnipool where asset in is omnipool asset and asset out is stable asset.
	#[require_transactional]
	fn resolve_mixed_trade_stable_out_given_asset_in(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                 // omnipool asset
		asset_out: AssetIdOf<T>,                // stable asset
		subpool_id_out: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_in: Balance,
		min_limit: Balance,
	) -> DispatchResult {
		if asset_in == <T as pallet_omnipool::Config>::HubAssetId::get() {
			return Self::resolve_mixed_trade_stable_out_given_hub_asset_in(
				who,
				asset_in,
				asset_out,
				subpool_id_out,
				amount_in,
				min_limit,
			);
		}

		let asset_state_in = OmnipoolPallet::<T>::load_asset_state(asset_in)?;
		let share_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;
		let subpool_state_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;

		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_state_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let idx_out = subpool_state_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_stable_out_given_iso_in(
			SubpoolState {
				reserves: &subpool_state_out.balances::<T>(),
				amplification: subpool_state_out.amplification as u128,
			},
			idx_out,
			&(&asset_state_in).into(),
			&(&share_state_out).into(),
			share_issuance_out,
			amount_in,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.subpool.amount >= min_limit, Error::<T>::Limit);

		debug_assert_eq!(
			*result.isopool.asset_in.delta_reserve, amount_in,
			"Returned amount is not equal to amount_in"
		);

		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_in.delta_reserve,
		)?;

		dbg!(*result.subpool.amount);
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_state_out.pool_account::<T>(),
			who,
			*result.subpool.amount,
		)?;

		//TODO: VERIFY only update subpool share state in omnipool. - might need to burn or mint some shares!
		// TODO: part of omnipool pallet ?
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_out.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(asset_in, subpool_id_out.into(), result.isopool)?;

		Ok(())
	}

	/// Handle sell trade between subpool and omnipool where asset in is hub asset and asset out is stable asset.
	#[require_transactional]
	fn resolve_mixed_trade_stable_out_given_hub_asset_in(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                 // omnipool asset
		asset_out: AssetIdOf<T>,                // stable asset
		subpool_id_out: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_in: Balance,
		min_limit: Balance,
	) -> DispatchResult {
		ensure!(
			asset_in == <T as pallet_omnipool::Config>::HubAssetId::get(),
			pallet_omnipool::Error::<T>::NotAllowed
		);

		let share_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;
		let subpool_state_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;
		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let withdraw_fee = subpool_state_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();
		let current_hub_asset_liquidity = CurrencyOf::<T>::free_balance(
			<T as pallet_omnipool::Config>::HubAssetId::get(),
			&OmnipoolPallet::<T>::protocol_account(),
		);

		let idx_out = subpool_state_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_stable_out_given_hub_asset_in(
			SubpoolState {
				reserves: &subpool_state_out.balances::<T>(),
				amplification: subpool_state_out.amplification as u128,
			},
			idx_out,
			&(&share_state_out).into(),
			share_issuance_out,
			amount_in,
			asset_fee,
			withdraw_fee,
			current_imbalance.value,
			current_hub_asset_liquidity,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.subpool.amount >= min_limit, Error::<T>::Limit);

		debug_assert_eq!(
			*result.isopool.asset.delta_hub_reserve, amount_in,
			"Returned amount is not equal to amount_in"
		);

		//TODO: VERIFY is this delta_hub_reserve to tansfer ??! and burn the delta_reserve?
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset.delta_hub_reserve,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_state_out.pool_account::<T>(),
			who,
			*result.subpool.amount,
		)?;

		//TODO: VERIFY only update subpool share state in omnipool. - might need to burn or mint some shares!
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_hub_asset_trade(subpool_id_out.into(), result.isopool)?;

		Ok(())
	}

	/// Handle buy itrade between subpool and omnipool where asset in is stable asset and asset out is omnipool asset.
	#[require_transactional]
	fn resolve_mixed_trade_stable_in_given_asset_out(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                // stable asset
		asset_out: AssetIdOf<T>,               // omnipool asset
		subpool_id_in: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_out: Balance,
		max_limit: Balance,
	) -> DispatchResult {
		if asset_out == <T as pallet_omnipool::Config>::HubAssetId::get() {
			// LRNA is not allowed to be bought
			return Err(pallet_omnipool::Error::<T>::NotAllowed.into());
		}

		let asset_state = OmnipoolPallet::<T>::load_asset_state(asset_out)?;
		let share_state = OmnipoolPallet::<T>::load_asset_state(subpool_id_in.into())?;
		let subpool_state = StableswapPallet::<T>::get_pool(subpool_id_in)?;

		let share_issuance_in = CurrencyOf::<T>::total_issuance(subpool_id_in.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_state.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let idx_in = subpool_state
			.find_asset(asset_in.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_stable_in_given_iso_out(
			SubpoolState {
				reserves: &subpool_state.balances::<T>(),
				amplification: subpool_state.amplification as u128,
			},
			idx_in,
			&(&asset_state).into(),
			&(&share_state).into(),
			share_issuance_in,
			amount_out,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		dbg!(*result.subpool.amount);

		ensure!(*result.subpool.amount <= max_limit, Error::<T>::Limit);

		debug_assert_eq!(
			*result.isopool.asset_out.delta_reserve, amount_out,
			"Returned amount is not equal to amount_out"
		);

		// Update subpools - transfer between subpool and who
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&subpool_state.pool_account::<T>(),
			*result.subpool.amount,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			who,
			*result.isopool.asset_out.delta_reserve,
		)?;

		//TODO: VERIFY only update subpool share state in omnipool. - might need to burn or mint some shares!
		<T as pallet_omnipool::Config>::Currency::deposit(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_in.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(subpool_id_in.into(), asset_out, result.isopool)?;

		Ok(())
	}

	/// Resolve buy trade between subpool and omnipool where asset in is omnipool asset and asset out is stable asset.
	#[require_transactional]
	fn resolve_mixed_trade_iso_in_given_stable_out(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                 // omnipool asset
		asset_out: AssetIdOf<T>,                // stable asset
		subpool_id_out: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_out: Balance,
		max_limit: Balance,
	) -> DispatchResult {
		if asset_in == <T as pallet_omnipool::Config>::HubAssetId::get() {
			return Self::resolve_mixed_trade_hub_asset_in_given_stable_out(
				who,
				asset_in,
				asset_out,
				subpool_id_out,
				amount_out,
				max_limit,
			);
		}

		let asset_state_in = OmnipoolPallet::<T>::load_asset_state(asset_in)?;
		let share_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;
		let subpool_state_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;

		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let protocol_fee = <T as pallet_omnipool::Config>::ProtocolFee::get();
		let withdraw_fee = subpool_state_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();

		let idx_out = subpool_state_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_iso_in_given_stable_out(
			SubpoolState {
				reserves: &subpool_state_out.balances::<T>(),
				amplification: subpool_state_out.amplification as u128,
			},
			idx_out,
			&(&asset_state_in).into(),
			&(&share_state_out).into(),
			share_issuance_out,
			amount_out,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.isopool.asset_in.delta_reserve <= max_limit, Error::<T>::Limit);

		dbg!(*result.subpool.amount);

		debug_assert_eq!(
			*result.subpool.amount, amount_out,
			"Returned amount is not equal to amount_out"
		);

		// Update subpools - transfer between subpool and who
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_in.delta_reserve,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_state_out.pool_account::<T>(),
			who,
			*result.subpool.amount,
		)?;

		//TODO: VERIFY only update subpool share state in omnipool. - might need to burn or mint some shares!
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset_out.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_trade_result(asset_in, subpool_id_out.into(), result.isopool)?;

		Ok(())
	}

	/// Resolve buy trade between subpool and omnipool where asset in is hub asset and asset out is stable asset.
	#[require_transactional]
	fn resolve_mixed_trade_hub_asset_in_given_stable_out(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,                 // omnipool asset
		asset_out: AssetIdOf<T>,                // stable asset
		subpool_id_out: StableswapAssetIdOf<T>, // pool id in which the stable asset is
		amount_out: Balance,
		max_limit: Balance,
	) -> DispatchResult {
		ensure!(
			asset_in == <T as pallet_omnipool::Config>::HubAssetId::get(),
			pallet_omnipool::Error::<T>::NotAllowed
		);

		let share_state_out = OmnipoolPallet::<T>::load_asset_state(subpool_id_out.into())?;
		let subpool_state_out = StableswapPallet::<T>::get_pool(subpool_id_out)?;

		let share_issuance_out = CurrencyOf::<T>::total_issuance(subpool_id_out.into());

		let asset_fee = <T as pallet_omnipool::Config>::AssetFee::get();
		let withdraw_fee = subpool_state_out.withdraw_fee;
		let current_imbalance = OmnipoolPallet::<T>::current_imbalance();
		let current_hub_asset_liquidity = CurrencyOf::<T>::free_balance(
			<T as pallet_omnipool::Config>::HubAssetId::get(),
			&OmnipoolPallet::<T>::protocol_account(),
		);

		let idx_out = subpool_state_out
			.find_asset(asset_out.into())
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let result = hydra_dx_math::omnipool_subpools::calculate_hub_asset_in_given_stable_out(
			SubpoolState {
				reserves: &subpool_state_out.balances::<T>(),
				amplification: subpool_state_out.amplification as u128,
			},
			idx_out,
			&(&share_state_out).into(),
			share_issuance_out,
			amount_out,
			asset_fee,
			withdraw_fee,
			current_imbalance.value,
			current_hub_asset_liquidity,
		)
		.ok_or(Error::<T>::Math)?;

		ensure!(*result.isopool.asset.delta_reserve <= max_limit, Error::<T>::Limit);

		debug_assert_eq!(
			*result.subpool.amount, amount_out,
			"Returned amount is not equal to amount_out"
		);

		// Update subpools - transfer between subpool and who
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_in.into(),
			who,
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset.delta_hub_reserve,
		)?;
		<T as pallet_stableswap::Config>::Currency::transfer(
			asset_out.into(),
			&subpool_state_out.pool_account::<T>(),
			who,
			*result.subpool.amount,
		)?;

		//TODO: update subpool share state in omnipool. - might need to burn or mint some shares!
		//TODO: VERIFY only update subpool share state in omnipool. - might need to burn or mint some shares!
		// TODO: COLIN : here we transfer the hub reserve from WHOP and burn the delta_reserve ??!
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.isopool.asset.delta_reserve,
		)?;

		OmnipoolPallet::<T>::update_omnipool_state_given_hub_asset_trade(subpool_id_out.into(), result.isopool)?;

		Ok(())
	}
}
