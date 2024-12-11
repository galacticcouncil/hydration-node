pub type Balance = u128;
pub type AssetId = u32;
pub type IntentId = u128;

#[derive(Debug, Clone)]
pub struct Intent {
	pub intent_id: IntentId,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partial: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedIntent {
	pub intent_id: IntentId,
	pub amount_in: Balance,
	pub amount_out: Balance,
}
