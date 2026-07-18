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
	use frame_support::{transactional, PalletId};
	use frame_system::pallet_prelude::*;
	use hydradx_traits::fee_processor::{Convert, FeeDestination, FeeReceiver};
	use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use sp_runtime::traits::AccountIdConversion;
	use sp_runtime::{Permill, Rounding, Saturating};
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
		type FeeReceivers: FeeReceiver<Self::AccountId, Self::AssetId, Balance, Error = DispatchError>;

		/// Fee receivers for direct HDX fees (may differ from FeeReceivers).
		/// For example, HDX fees may skip certain receivers and redistribute their share.
		type HdxFeeReceivers: FeeReceiver<Self::AccountId, Self::AssetId, Balance, Error = DispatchError>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	/// Assets pending conversion to HDX.
	#[pallet::storage]
	#[pallet::getter(fn pending_conversions)]
	pub type PendingConversions<T: Config> = CountedStorageMap<_, Blake2_128Concat, T::AssetId, (), OptionQuery>;

	/// HDX held in the pot for a `hold_until_ed` receiver whose account is still
	/// below the existential deposit. The HDX physically lives in
	/// `pot_account_id()`; this map only earmarks how much of it belongs to each
	/// destination. Flushed to the destination once `balance + held + slice ≥ ED`.
	#[pallet::storage]
	#[pallet::getter(fn held_fees)]
	pub type HeldFees<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Fee received from trade.
		FeeReceived {
			asset: T::AssetId,
			amount: Balance,
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

			// Budget conversions against BOTH weight dimensions: each `do_convert` runs a real
			// Omnipool sell with non-trivial proof size, so gating on `ref_time` alone could
			// overweight the block's PoV. A zero-cost dimension imposes no limit.
			let fits = |budget: u64, cost: u64| if cost == 0 { u64::MAX } else { budget / cost };
			let max_conversions = fits(remaining_weight.ref_time(), convert_weight.ref_time())
				.min(fits(remaining_weight.proof_size(), convert_weight.proof_size()))
				.min(T::MaxConversionsPerBlock::get() as u64);

			if max_conversions == 0 {
				return Weight::zero();
			}

			let mut used_weight = Weight::zero();

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
		/// Splits the fee among the configured receivers. Two kinds of receivers:
		///
		/// - Raw-asset receivers (`accepts_raw_asset()` — e.g. referrals): their slice
		///   is transferred in the *original* asset directly to their destination, then
		///   `on_raw_fee_received` notifies them so they handle conversion/accounting.
		/// - HDX-target receivers: their combined slice is taken into the pallet pot.
		///   On the HDX path it is distributed immediately; on the non-HDX path it is
		///   marked for `on_idle` conversion to HDX and distributed afterwards.
		///
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

			let is_hdx = asset == T::HdxAssetId::get();
			let destinations = if is_hdx {
				T::HdxFeeReceivers::destinations()
			} else {
				T::FeeReceivers::destinations()
			};

			let pot = Self::pot_account_id();
			let mut total_taken: Balance = 0;

			// Raw-asset receivers: each reports how much of its slice it actually wants
			// (per destination). Transfer exactly that in the original asset and leave any
			// unconsumed remainder with `source` — nothing is socialized.
			let raw_takes = if is_hdx {
				T::HdxFeeReceivers::on_raw_fee_received(trader.clone(), asset, amount)?
			} else {
				T::FeeReceivers::on_raw_fee_received(trader.clone(), asset, amount)?
			};
			for (dest, used) in raw_takes {
				if used > 0 {
					T::Currency::transfer(asset, &source, &dest, used, Preservation::Expendable)?;
					total_taken = total_taken.saturating_add(used);
				}
			}

			// HDX-target receivers: combined slice into the pot.
			let convert_pct = Self::convert_percentage(&destinations);
			let take = convert_pct.mul_floor(amount);
			if take > 0 {
				T::Currency::transfer(asset, &source, &pot, take, Preservation::Expendable)?;
				if is_hdx {
					Self::distribute_proportionally(&pot, take, destinations, convert_pct)?;
				} else {
					PendingConversions::<T>::insert(asset, ());
				}
				total_taken = total_taken.saturating_add(take);
			}

			if total_taken == 0 {
				return Ok(None);
			}

			Self::deposit_event(Event::FeeReceived {
				asset,
				amount: total_taken,
				trader: Some(trader),
			});

			Ok(Some((total_taken, pot)))
		}

		/// Internal conversion: swap pot's asset balance to HDX, distribute the proceeds
		/// proportionally to the HDX-target receivers based on their relative weights.
		///
		/// Transactional: the `on_idle` caller establishes no storage layer, so without this
		/// a distribution failure on a later receiver would strand the already-swapped HDX
		/// with earlier receivers paid. The layer keeps the swap-and-distribute atomic.
		#[transactional]
		fn do_convert(asset_id: T::AssetId) -> DispatchResult {
			ensure!(asset_id != T::HdxAssetId::get(), Error::<T>::AlreadyHdx);

			let pot = Self::pot_account_id();
			let balance = T::Currency::balance(asset_id, &pot);

			let hdx_received = T::Convert::convert(pot.clone(), asset_id, T::HdxAssetId::get(), balance)?;

			PendingConversions::<T>::remove(asset_id);

			let destinations = T::FeeReceivers::destinations();
			let convert_pct = Self::convert_percentage(&destinations);
			Self::distribute_proportionally(&pot, hdx_received, destinations, convert_pct)?;

			Self::deposit_event(Event::Converted {
				asset_id,
				amount_in: balance,
				hdx_out: hdx_received,
			});

			Ok(())
		}

		/// Sum of percentages of the HDX-target (non-raw) receivers.
		fn convert_percentage(destinations: &[FeeDestination<T::AccountId>]) -> Permill {
			destinations
				.iter()
				.filter(|d| !d.accepts_raw)
				.fold(Permill::zero(), |acc, d| acc.saturating_add(d.percentage))
		}

		/// Distribute `total` HDX among the HDX-target `destinations` proportionally to
		/// each destination's percentage relative to `total_pct`. Raw-asset receivers are
		/// skipped — they were already paid in the original asset.
		///
		/// `total` is already sitting in `source` (the pot). For a `hold_until_ed`
		/// destination whose account would still be below ED after receiving its
		/// slice, the slice is left in the pot and tracked in `HeldFees` instead of
		/// transferred — avoiding a `Token::BelowMinimum` revert. The buffer is
		/// flushed (held + new slice) the moment `balance + held + slice ≥ ED`.
		fn distribute_proportionally(
			source: &T::AccountId,
			total: Balance,
			destinations: Vec<FeeDestination<T::AccountId>>,
			total_pct: Permill,
		) -> DispatchResult {
			if total == 0 || total_pct.is_zero() {
				return Ok(());
			}
			let hdx = T::HdxAssetId::get();
			let ed = T::Currency::minimum_balance(hdx);
			let denom = total_pct.deconstruct() as u128;
			for FeeDestination {
				account,
				percentage,
				accepts_raw,
				hold_until_ed,
			} in destinations
			{
				if accepts_raw {
					continue;
				}
				let numer = percentage.deconstruct() as u128;
				let slice = multiply_by_rational_with_rounding(total, numer, denom, Rounding::Down)
					.ok_or(Error::<T>::Arithmetic)?;
				if slice == 0 {
					continue;
				}

				if hold_until_ed {
					// `slice` is already in the pot; flush the accumulated buffer
					// only if the account would then reach ED, else keep holding.
					let held = HeldFees::<T>::get(&account);
					let pending = held.saturating_add(slice);
					let balance = T::Currency::balance(hdx, &account);
					if balance.saturating_add(pending) >= ed {
						T::Currency::transfer(hdx, source, &account, pending, Preservation::Expendable)?;
						HeldFees::<T>::remove(&account);
					} else {
						HeldFees::<T>::insert(&account, pending);
					}
				} else {
					T::Currency::transfer(hdx, source, &account, slice, Preservation::Expendable)?;
				}
			}
			Ok(())
		}
	}
}
