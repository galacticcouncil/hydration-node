#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentSwapResult {
	Native,
	Error,
	Swapped,
	Transferred,
}

pub trait CurrencySwap<AccountId, Balance> {
	fn swap(who: &AccountId, fee: Balance) -> Result<PaymentSwapResult, frame_support::sp_runtime::DispatchError>;
}
