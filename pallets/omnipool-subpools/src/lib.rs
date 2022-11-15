#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

#[cfg(test)]
mod tests;
mod types;

use crate::types::{AssetDetail, Balance};
use frame_support::pallet_prelude::*;
use hydra_dx_math::omnipool_subpools::SubpoolState;
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
	use pallet_omnipool::types::Tradability;
	use pallet_stableswap::types::AssetLiquidity;
	use sp_runtime::{ArithmeticError, FixedPointNumber, Permill};

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
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		SubpoolNotFound,
		WithdrawAssetNotSpecified,
		NotStableAsset,
		Math,
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
				// TODO: use safe math,consider doing mul first
				// There might be problems with division rounding, so consider using fixed type
				Ok(q * b / s)
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
				price: asset_state_a.price().ok_or(ArithmeticError::DivisionByZero)?,
				shares: asset_state_a.shares,
				hub_reserve: asset_state_a.hub_reserve,
				share_tokens: asset_state_a.hub_reserve,
			};
			let asset_b_details = AssetDetail {
				price: asset_state_b.price().ok_or(ArithmeticError::DivisionByZero)?,
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

			// Move liquidity from omnipool account to subpool
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

			//TODO: use safe math in following calculatations. Also fixed type to avoid rounding2zero errors
			//TODO: refactor delta_ps to have the original forumala like this
			// let delta_ps = subpool_state.shares
			//	* (asset_state.hub_reserve / subpool_state.hub_reserve)
			//	* (asset_state.protocol_shares / asset_state.shares);
			let delta_ps = subpool_state.shares * asset_state.hub_reserve / subpool_state.shares
				* asset_state.protocol_shares
				/ asset_state.shares;
			let delta_s = asset_state.hub_reserve * subpool_state.shares / subpool_state.hub_reserve;
			let delta_u = asset_state.hub_reserve * share_issuance / subpool_state.hub_reserve;

			let price = asset_state
				.price()
				.ok_or(ArithmeticError::DivisionByZero)?
				.checked_mul(
					&FixedU128::checked_from_rational(share_issuance, subpool_state.shares)
						.ok_or(ArithmeticError::DivisionByZero)?,
				)
				.ok_or(ArithmeticError::Overflow)?;

			OmnipoolPallet::<T>::update_asset_state(pool_id.into(), delta_q, delta_s, delta_ps, asset_state.cap)?;

			StableswapPallet::<T>::deposit_shares(&omnipool_account, pool_id, delta_u)?;

			// Remember some stuff to be able to update LP positions later on
			let asset_details = AssetDetail {
				price,
				shares: asset_state.shares,
				hub_reserve: delta_q,
				share_tokens: delta_u,
			};

			MigratedAssets::<T>::insert(asset_id, (pool_id, asset_details));

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
			position_id: T::PositionInstanceId,
			share_amount: Balance,
			asset: Option<AssetIdOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let position = OmnipoolPallet::<T>::load_position(position_id, who.clone())?;

			//TODO: bug?! - we should use `asset` param to get the migrated asset instead of the poistion_asset_id, because it is the share id which is not migrated to subpool
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
			// Figure out where each asset is - isopool or subpool
			// - if both in isopool - call omnipool sell
			// - if both in same subpool - call stableswap::sell
			// - if both in different subpool - handle here according to spec
			// - if mixed - handle here according to spec

			match (MigratedAssets::<T>::get(asset_in), MigratedAssets::<T>::get(asset_out)) {
				(None, None) => {
					// both are is0pool assets
					OmnipoolPallet::<T>::sell(origin, asset_in, asset_out, amount, min_buy_amount)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) if pool_id_in == pool_id_out => {
					// both are same subpool
					StableswapPallet::<T>::sell(
						origin,
						pool_id_in,
						asset_in.into(),
						asset_out.into(),
						amount,
						min_buy_amount,
					)
				}
				(Some((_pool_id_in, _)), Some((_pool_id_out, _))) => {
					// both are subpool but different subpools
					// TODO: add limit
					Self::handle_subpools_sell(&who, asset_in, asset_out, _pool_id_in, _pool_id_out, amount)
				}
				_ => {
					// TODO: Mixed cases - handled here according to spec
					Ok(())
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
			// Figure out where each asset is - isopool or subpool
			// - if both in isopool - call omnipool buy
			// - if both in same subpool - call stableswap buy
			// - if both in different subpool - handle here according to spec
			// - if mixed - handle here according to spec

			match (MigratedAssets::<T>::get(asset_in), MigratedAssets::<T>::get(asset_out)) {
				(None, None) => {
					// both are is0pool assets
					OmnipoolPallet::<T>::buy(origin, asset_out, asset_in, amount, max_sell_amount)
				}
				(Some((pool_id_in, _)), Some((pool_id_out, _))) if pool_id_in == pool_id_out => {
					// both are same subpool
					StableswapPallet::<T>::buy(
						origin,
						pool_id_in,
						asset_out.into(), //TODO: Martin - double chcek: the asset_out and asset_in was the other way around. I think it was a bug, so swapped them. If so, then we can remove this comment
						asset_in.into(),
						amount,
						max_sell_amount,
					)
				}
				(Some((_pool_id_in, _)), Some((_pool_id_out, _))) => {
					// both are subpool but different subpools
					// TODO: add limit
					// TODO: Martin - in the test `buy_should_work_when_assets_are_in_different_subpool` in buy.rs testfile, I got math error, so we should check this
					Self::handle_subpools_buy(&who, asset_in, asset_out, _pool_id_in, _pool_id_out, amount)
				}
				_ => {
					// TODO: Mixed cases - handled here according to spec
					Ok(())
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
	fn convert_position(
		pool_id: <T as pallet_omnipool::Config>::AssetId,
		migration_details: AssetDetail,
		position: Position<Balance, <T as pallet_omnipool::Config>::AssetId>,
	) -> Result<Position<Balance, <T as pallet_omnipool::Config>::AssetId>, DispatchError> {
		Ok(position)
	}

	fn handle_subpools_buy(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,
		asset_out: AssetIdOf<T>,
		subpool_id_in: StableswapAssetIdOf<T>,
		subpool_id_out: StableswapAssetIdOf<T>,
		amount_out: Balance,
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
		//TODO: should be part of omnipool to pdate state according to given changes
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_out.delta_reserve,
		)?;

		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_in.delta_reserve,
		)?;

		let updated_state_in = share_asset_state_in
			.delta_update(&result.iso_pool.asset_in)
			.ok_or(Error::<T>::Math)?;
		let updated_state_out = share_asset_state_out
			.delta_update(&result.iso_pool.asset_out)
			.ok_or(Error::<T>::Math)?;

		OmnipoolPallet::<T>::set_asset_state(subpool_id_in.into(), updated_state_in);
		OmnipoolPallet::<T>::set_asset_state(subpool_id_out.into(), updated_state_out);

		Ok(())
	}

	fn handle_subpools_sell(
		who: &T::AccountId,
		asset_in: AssetIdOf<T>,
		asset_out: AssetIdOf<T>,
		subpool_id_in: StableswapAssetIdOf<T>,
		subpool_id_out: StableswapAssetIdOf<T>,
		amount_out: Balance,
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
			amount_out,
			&(&share_asset_state_in).into(),
			&(&share_asset_state_out).into(),
			share_issuance_in,
			asset_fee,
			protocol_fee,
			withdraw_fee,
			current_imbalance.value,
		)
		.ok_or(Error::<T>::Math)?;

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
		//TODO: should be part of omnipool to pdate state according to given changes
		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_out.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_out.delta_reserve,
		)?;

		<T as pallet_omnipool::Config>::Currency::withdraw(
			subpool_id_in.into(),
			&OmnipoolPallet::<T>::protocol_account(),
			*result.iso_pool.asset_in.delta_reserve,
		)?;

		let updated_state_in = share_asset_state_in
			.delta_update(&result.iso_pool.asset_in)
			.ok_or(Error::<T>::Math)?;
		let updated_state_out = share_asset_state_out
			.delta_update(&result.iso_pool.asset_out)
			.ok_or(Error::<T>::Math)?;

		OmnipoolPallet::<T>::set_asset_state(subpool_id_in.into(), updated_state_in);
		OmnipoolPallet::<T>::set_asset_state(subpool_id_out.into(), updated_state_out);

		Ok(())
	}
}
