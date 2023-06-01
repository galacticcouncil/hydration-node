use crate::types::Balance;

pub struct MigrationDetails {
	pub price: (Balance, Balance),
	pub shares: Balance,
	pub hub_reserve: Balance,
	pub share_tokens: Balance,
}
