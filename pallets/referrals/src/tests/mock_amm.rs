

use frame_support::pallet_prelude::*;
pub use pallet::*;

type Balance = u128;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::{ensure_signed, pallet_prelude::OriginFor};

	#[pallet::pallet]
	#[pallet::generate_store(pub (crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type AssetId: frame_support::traits::tokens::AssetId + MaybeSerializeDeserialize;

		type TradeHooks: Hooks<Self::AccountId, Self::AssetId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (crate) fn deposit_event)]
	pub enum Event<T: Config> {
	}

	#[pallet::error]
	pub enum Error<T> {
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn trade(origin: OriginFor<T>, asset_in: T::AssetId, asset_out: T::AssetId, amount: Balance ) -> DispatchResult{
			let who= ensure_signed(origin)?;
			let result = T::TradeHooks::simulate_trade(&who, asset_in, asset_out, amount);
			let fee_result = T::TradeHooks::on_trade_fee(&who, &who, result.fee_asset, result.fee);
			Ok(())
		}
	}
}

pub struct TradeResult<AssetId> {
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub fee: Balance,
	pub fee_asset: AssetId,
}

pub struct OnFeeResult {
	pub(crate) unused: Balance,
}

pub trait Hooks<AccountId, AssetId>{
	fn simulate_trade(who: &AccountId, asset_in: AssetId, asset_out: AssetId, amount: Balance) -> TradeResult<AssetId>;
	fn on_trade_fee(source: &AccountId, trader: &AccountId, fee_asset: AssetId, fee: Balance) -> OnFeeResult;
}