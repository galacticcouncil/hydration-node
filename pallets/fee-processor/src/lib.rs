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
	use hydradx_traits::gigahdx::{Convert, FeeReceiver};
	use hydradx_traits::price::PriceProvider;
	use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use sp_runtime::traits::AccountIdConversion;
	use sp_runtime::{Permill, Rounding};
	use sp_std::vec::Vec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

		/// Minimum amount for conversion (prevent dust conversions).
		#[pallet::constant]
		type MinConversionAmount: Get<Balance>;

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
		/// Amount too low for conversion.
		AmountTooLow,
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
						Self::deposit_event(Event::ConversionFailed { asset_id, reason: e });
						PendingConversions::<T>::remove(asset_id);
					}
				}
				used_weight = used_weight.saturating_add(convert_weight);
			}

			used_weight
		}
	}

	impl<T: Config> Pallet<T> {
		/// Pallet account holding fees pending conversion.
		pub fn pot_account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Process a trade fee from Omnipool.
		///
		/// If HDX: distribute directly from source to receiver pots.
		/// If non-HDX: transfer to pallet pot, calculate HDX equivalent via spot price,
		/// execute optimistic callbacks, mark for conversion.
		/// If LRNA: skip.
		pub fn process_trade_fee(
			source: T::AccountId,
			trader: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
		) -> Result<Option<(Balance, T::AccountId)>, DispatchError> {
			// Skip LRNA fees - in reality there should not be any
			if asset == T::LrnaAssetId::get() {
				log::warn!(target: "fee", "Unexpected LRNA trade fee — skipping");
				return Ok(None);
			}

			if asset == T::HdxAssetId::get() {
				log::trace!(target:"fee", "HDX fee");
				// Already HDX — transfer to our pot first (so Omnipool can track),
				// then distribute from our pot to receiver pots.
				let pot = Self::pot_account_id();
				T::Currency::transfer(asset, &source, &pot, amount, Preservation::Expendable)?;
				// — execute pre-deposit callbacks with trader context (HDX-specific receivers)
				T::HdxFeeReceivers::on_pre_fee_deposit(trader.clone(), amount)?;
				log::trace!(target:"fee", "distributing to pots");
				Self::distribute_to_pots(&pot, amount, T::HdxFeeReceivers::destinations())?;
				// — execute post-deposit callbacks (HDX-specific receivers)
				T::HdxFeeReceivers::on_fee_received(amount)?;

				log::trace!(target:"fee", "distributing to pots done");
				Self::deposit_event(Event::FeeReceived {
					asset,
					amount,
					hdx_equivalent: amount,
					trader: Some(trader),
				});

				Ok(Some((amount, pot)))
			} else {
				// Non-HDX asset — transfer to pot, calculate spot price, mark for conversion
				let pot = Self::pot_account_id();

				T::Currency::transfer(asset, &source, &pot, amount, Preservation::Expendable)?;

				// Calculate HDX equivalent using SPOT PRICE (not actual swap).
				// If price is unavailable (e.g. oracle not yet populated), skip
				// optimistic callbacks — the actual conversion will happen on_idle.
				let hdx_equivalent = Self::calculate_hdx_equivalent(asset, amount).unwrap_or(0);

				if hdx_equivalent > 0 {
					// Execute OPTIMISTIC pre-deposit callbacks with trader context
					T::FeeReceivers::on_pre_fee_deposit(trader.clone(), hdx_equivalent)?;
				}

				// Mark for conversion (on_idle will convert and distribute)
				PendingConversions::<T>::insert(asset, ());

				Self::deposit_event(Event::FeeReceived {
					asset,
					amount,
					hdx_equivalent,
					trader: Some(trader),
				});

				Ok(Some((amount, pot)))
			}
		}

		/// Calculate HDX equivalent using spot price from oracle.
		fn calculate_hdx_equivalent(asset: T::AssetId, amount: Balance) -> Result<Balance, DispatchError> {
			let price =
				T::PriceProvider::get_price(asset, T::HdxAssetId::get()).ok_or(Error::<T>::PriceNotAvailable)?;

			let hdx_amount = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Down)
				.ok_or(Error::<T>::Arithmetic)?;

			Ok(hdx_amount)
		}

		/// Internal conversion: swap asset to HDX via Convert trait, distribute to pots.
		fn do_convert(asset_id: T::AssetId) -> DispatchResult {
			ensure!(asset_id != T::HdxAssetId::get(), Error::<T>::AlreadyHdx);

			let pot = Self::pot_account_id();
			let balance = T::Currency::balance(asset_id, &pot);

			ensure!(balance >= T::MinConversionAmount::get(), Error::<T>::AmountTooLow);

			// Execute swap
			let hdx_received = T::Convert::convert(pot.clone(), asset_id, T::HdxAssetId::get(), balance)?;

			// Remove from pending
			PendingConversions::<T>::remove(asset_id);

			// Distribute immediately to pots (non-HDX path uses FeeReceivers)
			Self::distribute_to_pots(&pot, hdx_received, T::FeeReceivers::destinations())?;
			// Post-deposit callbacks
			T::FeeReceivers::on_fee_received(hdx_received)?;

			Self::deposit_event(Event::Converted {
				asset_id,
				amount_in: balance,
				hdx_out: hdx_received,
			});

			Ok(())
		}

		/// Distribute HDX to all receiver pots according to their percentages.
		fn distribute_to_pots(
			source: &T::AccountId,
			total: Balance,
			destinations: Vec<(T::AccountId, Permill)>,
		) -> DispatchResult {
			for (dest, percentage) in destinations {
				let balance = T::Currency::balance(T::HdxAssetId::get(), source);
				let amount = percentage.mul_floor(total);
				if amount > 0 {
					// Use Expendable: fee source account may be fully drained
					T::Currency::transfer(T::HdxAssetId::get(), source, &dest, amount, Preservation::Expendable)?;
				}
			}

			Ok(())
		}
	}
}
