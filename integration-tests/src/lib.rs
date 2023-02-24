mod call_filter;
mod cross_chain_transfer;
mod dca;
mod dust;
mod dust_removal_whitelist;
mod non_native_fee;
mod omnipool_init;
mod polkadot_test_net;
mod vesting;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Currencies::free_balance($y, &$x), $z);
	}};
}

#[macro_export]
macro_rules! assert_reserved_balance {
	( $who:expr, $asset:expr, $amount:expr) => {{
		assert_eq!(Currencies::reserved_balance($asset, &$who), $amount);
	}};
}
