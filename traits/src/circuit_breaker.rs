pub trait WithdrawFuseControl {
	fn set_withdraw_fuse_active(value: bool);
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl WithdrawFuseControl for () {
	fn set_withdraw_fuse_active(value: bool) {}
}
