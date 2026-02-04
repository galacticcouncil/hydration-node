pub trait WithdrawFuseControl {
	fn set_withdraw_fuse_active(value: bool);
}

impl WithdrawFuseControl for () {
	fn set_withdraw_fuse_active(_value: bool) {}
}
