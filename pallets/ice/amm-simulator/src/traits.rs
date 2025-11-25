pub type AssetId = u32;
pub type Balance = u128;

pub enum Snapshot<O, S> {
	Omnipool(O),
	Stableswap(S),
}

pub struct SimResult {
	amount_in: Balance,
	amount_out: Balance,
}

pub trait AmmSimulator {
	type Snapshot;
	type Error;

	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		limit: Balance,
		use_snapshot: &Self::Snapshot,
	) -> Result<(SimResult, Self::Snapshot), Self::Error>;

	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		limit: Balance,
		use_snapshot: &Self::Snapshot,
	) -> Result<(SimResult, Self::Snapshot), Self::Error>;
}
