use orml_traits::{
	currency::{OnDeposit, OnTransfer},
	Handler,
};

pub trait WithdrawFuseControl {
	fn set_withdraw_fuse_active(value: bool);
}

impl WithdrawFuseControl for () {
	fn set_withdraw_fuse_active(_value: bool) {}
}

pub trait AssetWithdrawHandler<AccountId, AssetId, Balance> {
	type OnWithdraw: Handler<(AssetId, Balance)>;
	type OnDeposit: OnDeposit<AccountId, AssetId, Balance>;
	type OnTransfer: OnTransfer<AccountId, AssetId, Balance>;
}
