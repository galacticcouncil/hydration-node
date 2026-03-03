use crate::tests::mock::*;
use crate::*;
use frame_support::assert_err;
use frame_support::assert_ok;
use ice_support::AssetId;
use ice_support::Price;
use ice_support::SwapData;
use ice_support::SwapType;
use num_traits::SaturatingAdd;
use pretty_assertions::assert_eq;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn should_work_when_price_wasnt_computed_yet_and_reverse_price_is_missing() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);

	let swap_type = SwapType::ExactOut;
	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}

#[test]
fn should_work_when_computes_new_price_and_is_within_price_tolerance_or_reverse_trade() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	//Compute new exactIn price
	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	exec_prices.insert(
		(asset_in, asset_out, swap_type.reverse()),
		Ratio::new(amount_out, amount_in),
	);

	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);

	assert_eq!(
		exec_prices.get(&(asset_in, asset_out, swap_type)),
		exec_prices.get(&(asset_in, asset_out, swap_type.reverse()))
	);

	assert_eq!(exec_prices.len(), 2);

	//Compute new exectOut price
	let swap_type = SwapType::ExactOut;
	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	exec_prices.insert(
		(asset_in, asset_out, swap_type.reverse()),
		Ratio::new(amount_out, amount_in),
	);

	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve));

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("excution price to exists"),
		Ratio::new(amount_out, amount_in)
	);

	assert_eq!(
		exec_prices.get(&(asset_in, asset_out, swap_type)),
		exec_prices.get(&(asset_in, asset_out, swap_type.reverse()))
	);

	assert_eq!(exec_prices.len(), 2);
}

#[test]
fn should_not_work_when_computes_new_price_and_is_not_within_price_tolerance_or_reverse_trade() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	let mut reverse_price = Ratio::new(amount_out, amount_in);
	let tolerance =
		reverse_price.saturating_mul(&(BuySellTolerance::get().saturating_add(Permill::from_percent(1))).into());
	reverse_price = reverse_price.saturating_add(&tolerance);
	exec_prices.insert((asset_in, asset_out, swap_type.reverse()), reverse_price);

	assert_err!(
		ICE::validate_price_consitency(&mut exec_prices, &resolve),
		Error::<Test>::PriceToleranceInconsistency
	);

	assert_eq!(exec_prices.len(), 1);

	assert_eq!(exec_prices.get(&(asset_in, asset_out, swap_type)), None);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type.reverse()))
			.expect("execution price to exists"),
		reverse_price
	);
}

#[test]
fn should_fail_when_not_resolved_at_execution_price() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out: amount_out + 2,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	exec_prices.insert((asset_in, asset_out, swap_type), Ratio::new(amount_out, amount_in));

	assert_err!(
		ICE::validate_price_consitency(&mut exec_prices, &resolve),
		Error::<Test>::PriceInconsistency
	);

	assert_eq!(exec_prices.len(), 1);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("execution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}

#[test]
fn should_work_when_not_resolved_within_execution_price_tolerance() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		//NOTE: we have hadrcoded +-1 in case of rounding error
		amount_out: amount_out - 1,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	exec_prices.insert((asset_in, asset_out, swap_type), Ratio::new(amount_out, amount_in));

	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve),);

	assert_eq!(exec_prices.len(), 1);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("execution price to exists"),
		Ratio::new(amount_out, amount_in)
	);
}

#[test]
fn should_work_when_price_and_amount_are_within_tolerances() {
	let asset_in = HDX;
	let asset_out = DOT;
	let swap_type = SwapType::ExactIn;
	let amount_in = 100 * ONE_HDX;
	let amount_out = 200 * ONE_DOT;

	let resolve = IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in,
		amount_out: amount_out + 1,
		swap_type,
		partial: false,
	});

	let mut exec_prices: BTreeMap<(AssetId, AssetId, SwapType), Price> = BTreeMap::new();
	let mut reverse_price = Ratio::new(amount_out, amount_in);
	let tolerance =
		reverse_price.saturating_mul(&(BuySellTolerance::get().saturating_sub(Permill::from_percent(1))).into());
	reverse_price = reverse_price.saturating_add(&tolerance);
	exec_prices.insert((asset_in, asset_out, swap_type.reverse()), reverse_price);

	assert_ok!(ICE::validate_price_consitency(&mut exec_prices, &resolve));

	assert_eq!(exec_prices.len(), 2);

	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type))
			.expect("execution price to exists"),
		Ratio::new(amount_out + 1, amount_in)
	);
	assert_eq!(
		*exec_prices
			.get(&(asset_in, asset_out, swap_type.reverse()))
			.expect("execution price to exists"),
		reverse_price
	);
}
