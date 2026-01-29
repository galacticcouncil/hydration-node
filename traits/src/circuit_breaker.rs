pub trait WithdrawFuseControl {
	fn set_withdraw_fuse_active(value: bool);
}

// #[cfg(any(feature = "std", feature = "runtime-benchmarks"))]
impl WithdrawFuseControl for () {
	fn set_withdraw_fuse_active(_: bool) {}
}
