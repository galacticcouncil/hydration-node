pub trait AMMShares<AccountId> {
	fn total_shares(id: &AccountId) -> u128;
}
