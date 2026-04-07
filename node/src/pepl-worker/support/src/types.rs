use primitives::BlockNumber;
use primitives::EvmAddress;
use sp_core::RuntimeDebug;
use sp_core::U256;

/// User's data. The state is not automatically updated. Any change in the chain can invalidate the data stored in the struct.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Borrower {
	//TODO: mek sure we need this
	pub address: EvmAddress,
	configuration: UserConfiguration,
	reserves: Vec<UserReserve>, // the order of reserves is given by fetch_reserves_list()
	emode_id: U256,
	pub update_at: BlockNumber,
}

/// The configuration of the user across all the reserves.
/// Bitmap of the users collaterals and borrows. It is divided into pairs of bits, one pair per asset.
/// The first bit indicates if the user uses an asset as collateral, the second whether the user borrows an asset.
/// The corresponding assets are in the same position as `fetch_reserves_list()`.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
struct UserConfiguration(U256);
impl UserConfiguration {
	/// Returns `true` if the user uses the asset as collateral.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_collateral(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(2) << (2 * asset_index);
		!(self.0 & bit_mask).is_zero()
	}

	/// Returns `true` if the user uses the asset as debt.
	/// The asset index is the position of the asset in the `fetch_reserves_list()` array.
	pub fn is_debt(&self, asset_index: usize) -> bool {
		let bit_mask = U256::from(1) << (2 * asset_index);
		!(self.0 & bit_mask).is_zero()
	}
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct EModeCategory {
	// ltv: u16,
	liquidation_threshold: u16,
	liquidation_bonus: u16,
	// address priceSource;
	// string label;
}

impl EModeCategory {
	pub fn new(data: &[ethabi::Token]) -> Option<Self> {
		let data_tuple = data.first()?.clone().into_tuple()?;

		Some(Self {
			#[allow(clippy::get_first)]
			liquidation_threshold: data_tuple.get(1)?.clone().into_uint()?.try_into().ok()?,
			liquidation_bonus: data_tuple.get(2)?.clone().into_uint()?.try_into().ok()?,
		})
	}
}

/// Collateral and debt amounts of a reserve in the base currency.
#[derive(Default, Eq, PartialEq, Clone, RuntimeDebug)]
pub struct UserReserve {
	pub collateral: U256,
	pub debt: U256,
}
