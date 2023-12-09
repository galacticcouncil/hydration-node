use crate::tests::mock::*;
use crate::types::FeeParams;
use hydra_dx_math::dynamic_fees::{recalculate_asset_fee, recalculate_protocol_fee, types::OracleEntry};
use proptest::prelude::*;
use sp_runtime::traits::{One, Zero};
use sp_runtime::FixedU128;

const MAX_VOLUME: Balance = 100;
const MIN_LIQUIDITY: Balance = 1_000_000_000;
const MAX_LIQUIDITY: Balance = 10_000_000_000;

fn decimals() -> impl Strategy<Value = u32> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18),]
}

fn initial_fee() -> impl Strategy<Value = Fee> {
	(0.01..0.4).prop_map(Fee::from_float)
}

prop_compose! {
	fn entry()(dec in decimals(),
			   amount_in in 0..MAX_VOLUME,
			   amount_out in 0..MAX_VOLUME,
			   liquidity in MIN_LIQUIDITY..MAX_LIQUIDITY
	) -> OracleEntry{
		let one = 10u128.pow(dec);
		OracleEntry{
			amount_in: amount_in * one ,
			amount_out: amount_out * one,
			liquidity: liquidity * one,
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn fees_should_update_correctly_when_volume_out_increasing(previous_fee in initial_fee(),
		entry in entry().prop_filter("only interested when out > in", |v| v.amount_in < v.amount_out)){

		let block_diff = 1u128;
		let params = FeeParams {
			min_fee: Fee::from_percent(1),
			max_fee: Fee::from_percent(40),
			decay: FixedU128::zero(),
			amplification: FixedU128::one(),
		};

		let asset_fee = recalculate_asset_fee(entry.clone(), previous_fee, block_diff, params.into());
		let protocol_fee = recalculate_protocol_fee(entry, previous_fee, block_diff, params.into());

		assert!(
			asset_fee > previous_fee || asset_fee == params.max_fee,
			"Asset fee {previous_fee:?} has not increased - {asset_fee:?}"
		);
		assert!(
			protocol_fee < previous_fee || protocol_fee == params.min_fee,
			"Protocol fee {previous_fee:?} has not decreased - {asset_fee:?}"
		);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn fees_should_update_correctly_when_volume_in_increasing(previous_fee in initial_fee(),
		entry in entry().prop_filter("only interested when out < in", |v| v.amount_in > v.amount_out)){

		let block_diff = 1u128;
		let params = FeeParams {
			min_fee: Fee::from_percent(1),
			max_fee: Fee::from_percent(40),
			decay: FixedU128::zero(),
			amplification: FixedU128::one(),
		};

		let asset_fee = recalculate_asset_fee(entry.clone(), previous_fee, block_diff, params.into());
		let protocol_fee= recalculate_protocol_fee(entry, previous_fee, block_diff, params.into());

		assert!(asset_fee < previous_fee || asset_fee == params.min_fee, "Asset fee {previous_fee:?} has not decreased - {asset_fee:?}");
		assert!(protocol_fee > previous_fee || protocol_fee == params.max_fee, "Protocol fee {previous_fee:?} has not increased - {asset_fee:?}");
	}
}
