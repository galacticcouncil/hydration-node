use frame_support::pallet_prelude::*;

use sp_runtime::FixedU128;

/// Balance representation in current pallet.
pub type Balance = u128;

/// Asset details at the time of its migration from Omnipool to Stableswap subpool.
///
/// these details are used for conversion of already existing position to new stabeswap position.
#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetDetail {
	/// Price at the time of migration.
	pub(crate) price: FixedU128,
	/// Amount of asset shares distributed at the time of migration.
	pub(crate) shares: Balance,
	/// Hub asset reserve at the time of migration.
	pub(crate) hub_reserve: Balance,
	/// Share tokens of stabelswap subpool.
	pub(crate) share_tokens: Balance,
}
