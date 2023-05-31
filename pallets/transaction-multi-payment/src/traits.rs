use frame_support::sp_runtime::DispatchResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentInfo<Balance, AssetId, Price> {
	Native(Balance),
	NonNative(Balance, AssetId, Price),
}

/// Handler for dealing with fees
pub trait DepositFee<AccountId, AssetId, Balance> {
	fn deposit_fee(who: &AccountId, currency: AssetId, amount: Balance) -> DispatchResult;
}
