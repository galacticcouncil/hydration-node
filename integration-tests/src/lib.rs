// DCA pallet uses dummy router for benchmarks and some tests fail when benchmarking feature is enabled
#![cfg(not(feature = "runtime-benchmarks"))]
mod bonds;
mod call_filter;
mod circuit_breaker;
mod cross_chain_transfer;
mod dca;
mod dust;
mod dust_removal_whitelist;
mod dynamic_fees;
mod evm;
mod exchange_asset;
mod non_native_fee;
mod omnipool_init;
mod omnipool_liquidity_mining;
mod oracle;
mod otc;
mod polkadot_test_net;
mod referrals;
mod router;
mod staking;
mod transact_call_filter;
mod vesting;
mod xcm_defer;
mod xcm_rate_limiter;

#[macro_export]
macro_rules! assert_balance {
	( $who:expr, $asset:expr, $amount:expr) => {{
		assert_eq!(Currencies::free_balance($asset, &$who), $amount);
	}};
}

#[macro_export]
macro_rules! assert_reserved_balance {
	( $who:expr, $asset:expr, $amount:expr) => {{
		assert_eq!(Currencies::reserved_balance($asset, &$who), $amount);
	}};
}
mod xyk;
