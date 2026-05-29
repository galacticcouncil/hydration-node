#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

pub type Balance = u128;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::fungibles::{Inspect, Mutate};
	use frame_support::traits::tokens::Preservation;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use hydra_dx_math::ema::EmaPrice;
	use hydradx_traits::fee_processor::{Convert, FeeReceiver};
	use hydradx_traits::price::PriceProvider;
	use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use sp_runtime::traits::AccountIdConversion;
	use sp_runtime::{Permill, Rounding};
	use sp_std::vec::Vec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Asset ID type.
		type AssetId: Member + Parameter + Copy + MaybeSerializeDeserialize + MaxEncodedLen + Ord;

		/// Multi-currency support for transfers.
		type Currency: Mutate<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>
			+ Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>;

		/// Converter for swapping assets to HDX.
		type Convert: Convert<Self::AccountId, Self::AssetId, Balance, Error = DispatchError>;

		/// Spot price provider for calculating HDX equivalent before conversion.
		type PriceProvider: PriceProvider<Self::AssetId, Price = EmaPrice>;

		/// Pallet ID for the fee accumulation account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// HDX asset ID (target asset for conversions).
		#[pallet::constant]
		type HdxAssetId: Get<Self::AssetId>;

		/// LRNA asset ID (hub asset, fees in LRNA are skipped).
		#[pallet::constant]
		type LrnaAssetId: Get<Self::AssetId>;

		/// Maximum conversions per on_idle call.
		#[pallet::constant]
		type MaxConversionsPerBlock: Get<u32>;

		/// Tuple of fee receivers implementing FeeReceiver trait (used for non-HDX path).
		type FeeReceivers: FeeReceiver<Self::AccountId, Balance, Error = DispatchError>;

		/// Fee receivers for direct HDX fees (may differ from FeeReceivers).
		/// For example, HDX fees may skip certain receivers and redistribute their share.
		type HdxFeeReceivers: FeeReceiver<Self::AccountId, Balance, Error = DispatchError>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	/// Assets pending conversion to HDX.
	#[pallet::storage]
	#[pallet::getter(fn pending_conversions)]
	pub type PendingConversions<T: Config> = CountedStorageMap<_, Blake2_128Concat, T::AssetId, (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Fee received from trade.
		FeeReceived {
			asset: T::AssetId,
			amount: Balance,
			hdx_equivalent: Balance,
			trader: Option<T::AccountId>,
		},

		/// Asset converted to HDX and distributed to pots.
		Converted {
			asset_id: T::AssetId,
			amount_in: Balance,
			hdx_out: Balance,
		},

		/// Conversion failed (logged but not blocking).
		ConversionFailed {
			asset_id: T::AssetId,
			reason: DispatchError,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset is already HDX, no conversion needed.
		AlreadyHdx,
		/// Conversion failed.
		ConversionFailed,
		/// Transfer failed.
		TransferFailed,
		/// Spot price not available for asset pair.
		PriceNotAvailable,
		/// Arithmetic overflow/underflow.
		Arithmetic,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Manual conversion trigger. Permissionless.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::convert())]
		pub fn convert(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			ensure_signed(origin)?;

			Self::do_convert(asset_id)
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			let convert_weight = T::WeightInfo::convert();

			if remaining_weight.ref_time() < convert_weight.ref_time() {
				return Weight::zero();
			}

			let mut used_weight = Weight::zero();

			let max_conversions = remaining_weight
				.ref_time()
				.checked_div(convert_weight.ref_time())
				.unwrap_or(0)
				.min(T::MaxConversionsPerBlock::get() as u64);

			for asset_id in PendingConversions::<T>::iter_keys().take(max_conversions as usize) {
				match Self::do_convert(asset_id) {
					Ok(_) => {}
					Err(e) => {
						// Drop the pending entry so we don't waste weight retrying.
						// A subsequent fee for this asset will re-insert it.
						PendingConversions::<T>::remove(asset_id);
						Self::deposit_event(Event::ConversionFailed { asset_id, reason: e });
					}
				}
				used_weight = used_weight.saturating_add(convert_weight);
			}

			used_weight
		}

		fn integrity_test() {
			let non_hdx = T::FeeReceivers::percentage();
			assert!(
				non_hdx <= Permill::one(),
				"pallet-fee-processor: FeeReceivers percentages sum to more than 100% ({non_hdx:?})",
			);

			let hdx = T::HdxFeeReceivers::percentage();
			assert!(
				hdx <= Permill::one(),
				"pallet-fee-processor: HdxFeeReceivers percentages sum to more than 100% ({hdx:?})",
			);
		}
	}

	impl<T: Config> Pallet<T> {
		/// Pallet account holding fees pending conversion.
		pub fn pot_account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Process a trade fee from Omnipool.
		///
		/// Pulls only the portion of the fee specified by the configured receivers
		/// (sum of their percentages). The remainder stays at `source`.
		///
		/// HDX path: take and distribute proportionally to `HdxFeeReceivers`.
		/// Non-HDX path: take into pallet pot, mark for `on_idle` conversion to HDX,
		/// then distribute to `FeeReceivers`.
		/// LRNA: skipped (not expected).
		pub fn process_trade_fee(
			source: T::AccountId,
			trader: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
		) -> Result<Option<(Balance, T::AccountId)>, DispatchError> {
			if asset == T::LrnaAssetId::get() {
				log::warn!(target: "fee", "Unexpected LRNA trade fee — skipping");
				return Ok(None);
			}

			if asset == T::HdxAssetId::get() {
				let total_pct = T::HdxFeeReceivers::percentage();
				let take = total_pct.mul_floor(amount);
				if take == 0 {
					return Ok(None);
				}

				let pot = Self::pot_account_id();
				T::Currency::transfer(asset, &source, &pot, take, Preservation::Expendable)?;

				// Callbacks pass the conceptual full trade-fee amount — tuple impl auto-splits
				// to `their_pct.mul_floor(amount)`, which equals each receiver's actual share.
				T::HdxFeeReceivers::on_pre_fee_deposit(trader.clone(), amount)?;
				Self::distribute_proportionally(&pot, take, T::HdxFeeReceivers::destinations(), total_pct)?;
				T::HdxFeeReceivers::on_fee_received(amount)?;

				Self::deposit_event(Event::FeeReceived {
					asset,
					amount: take,
					hdx_equivalent: take,
					trader: Some(trader),
				});

				Ok(Some((take, pot)))
			} else {
				let total_pct = T::FeeReceivers::percentage();
				let take = total_pct.mul_floor(amount);
				if take == 0 {
					return Ok(None);
				}

				let pot = Self::pot_account_id();
				T::Currency::transfer(asset, &source, &pot, take, Preservation::Expendable)?;

				// Optimistic pre-deposit uses HDX equivalent of the *full* fee amount.
				// Tuple impl auto-splits so each receiver sees `their_pct * hdx_equivalent`,
				// which is what they would receive if conversion were lossless at spot.
				let hdx_equivalent = Self::calculate_hdx_equivalent(asset, amount).unwrap_or(0);
				if hdx_equivalent > 0 {
					T::FeeReceivers::on_pre_fee_deposit(trader.clone(), hdx_equivalent)?;
				}

				PendingConversions::<T>::insert(asset, ());

				Self::deposit_event(Event::FeeReceived {
					asset,
					amount: take,
					hdx_equivalent,
					trader: Some(trader),
				});

				Ok(Some((take, pot)))
			}
		}

		/// Calculate HDX equivalent using spot price from oracle.
		fn calculate_hdx_equivalent(asset: T::AssetId, amount: Balance) -> Result<Balance, DispatchError> {
			// Price convention (see runtime adapter `ConvertBalance`): to convert an amount of
			// `from` into `to`, use `get_price(to, from)` then multiply by `n/d`. Here we convert
			// `asset` → HDX, so the target HDX is the first argument.
			let price =
				T::PriceProvider::get_price(T::HdxAssetId::get(), asset).ok_or(Error::<T>::PriceNotAvailable)?;

			let hdx_amount = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Down)
				.ok_or(Error::<T>::Arithmetic)?;

			Ok(hdx_amount)
		}

		/// Internal conversion: swap pot's asset balance to HDX, distribute the proceeds
		/// proportionally to receivers based on their relative weights.
		fn do_convert(asset_id: T::AssetId) -> DispatchResult {
			ensure!(asset_id != T::HdxAssetId::get(), Error::<T>::AlreadyHdx);

			let pot = Self::pot_account_id();
			let balance = T::Currency::balance(asset_id, &pot);

			let hdx_received = T::Convert::convert(pot.clone(), asset_id, T::HdxAssetId::get(), balance)?;

			PendingConversions::<T>::remove(asset_id);

			let total_pct = T::FeeReceivers::percentage();
			Self::distribute_proportionally(&pot, hdx_received, T::FeeReceivers::destinations(), total_pct)?;

			// Scale received HDX up to the conceptual full-fee amount so the tuple's
			// auto-split (`their_pct * scaled`) equals each receiver's actual transfer.
			let scaled = Self::scale_to_full(hdx_received, total_pct);
			T::FeeReceivers::on_fee_received(scaled)?;

			Self::deposit_event(Event::Converted {
				asset_id,
				amount_in: balance,
				hdx_out: hdx_received,
			});

			Ok(())
		}

		/// Distribute `total` HDX among `destinations` proportionally to each
		/// destination's percentage relative to `total_pct` (sum of all percentages).
		fn distribute_proportionally(
			source: &T::AccountId,
			total: Balance,
			destinations: Vec<(T::AccountId, Permill)>,
			total_pct: Permill,
		) -> DispatchResult {
			if total == 0 || total_pct.is_zero() {
				return Ok(());
			}
			let denom = total_pct.deconstruct() as u128;
			for (dest, pct) in destinations {
				let numer = pct.deconstruct() as u128;
				let amount = multiply_by_rational_with_rounding(total, numer, denom, Rounding::Down)
					.ok_or(Error::<T>::Arithmetic)?;
				if amount > 0 {
					T::Currency::transfer(T::HdxAssetId::get(), source, &dest, amount, Preservation::Expendable)?;
				}
			}
			Ok(())
		}

		/// Scale `actual` (which represents `total_pct` of the conceptual full amount)
		/// back up to the full amount, so tuple auto-split callbacks compute correct
		/// per-receiver shares.
		fn scale_to_full(actual: Balance, total_pct: Permill) -> Balance {
			if total_pct.is_zero() {
				return 0;
			}
			multiply_by_rational_with_rounding(actual, 1_000_000u128, total_pct.deconstruct() as u128, Rounding::Down)
				.unwrap_or(actual)
		}
	}
}
