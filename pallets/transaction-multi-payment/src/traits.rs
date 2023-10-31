use frame_support::sp_runtime::DispatchResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentInfo<Balance, AssetId, Price> {
	Native(Balance),
	NonNative(Balance, AssetId, Price),
}
