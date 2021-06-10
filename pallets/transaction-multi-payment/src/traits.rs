#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentSwapResult {
	NATIVE,
	ERROR,
	SWAPPED,
	TRANSFERRED,
}

pub trait CurrencySwap<AccountId, Balance> {
	fn swap(who: &AccountId, fee: Balance) -> Result<PaymentSwapResult, frame_support::sp_runtime::DispatchError>;
}
