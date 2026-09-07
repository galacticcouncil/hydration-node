use crate::tests::mock::*;
use crate::*;
use frame_support::assert_err;
use frame_support::assert_ok;
use ice_support::AssetId;
use ice_support::Partial;
use ice_support::Price;
use ice_support::SwapData;
use pretty_assertions::assert_eq;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn should_work_when_price_wasnt_computed_yet_and_reverse_price_is_missing() {
	let asset_in = HDX;
	let asset_out = DOT;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		partial: Partial::No,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
	assert_ok!(ICE::validate_price_consistency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		partial: Partial::No,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
	assert_ok!(ICE::validate_price_consistency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}

#[test]
fn should_fail_when_not_resolved_at_execution_price() {
	let asset_in = HDX;
	let asset_out = DOT;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out: amount_out + 2,
		partial: Partial::No,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
	exec_prices.insert((asset_in, asset_out), Ratio::new(amount_out, amount_in));

	assert_err!(
		ICE::validate_price_consistency(&mut exec_prices, &resolve),
		Error::<Test>::PriceInconsistency
	);

	assert_eq!(exec_prices.len(), 1);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out))
			.expect("execution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}

#[test]
fn should_work_when_not_resolved_within_execution_price_tolerance() {
	let asset_in = HDX;
	let asset_out = DOT;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		//NOTE: we have hadrcoded +-1 in case of rounding error
		amount_out: amount_out - 1,
		partial: Partial::No,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
	exec_prices.insert((asset_in, asset_out), Ratio::new(amount_out, amount_in));

	assert_ok!(ICE::validate_price_consistency(&mut exec_prices, &resolve),);

	assert_eq!(exec_prices.len(), 1);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out))
			.expect("execution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}
