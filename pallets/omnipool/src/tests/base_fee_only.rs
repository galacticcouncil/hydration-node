use super::*;
use crate::types::SlipFeeConfig;
use hydra_dx_math::omnipool::types::{HubTradeSlipFees, SignedBalance, TradeSlipFees};
use sp_runtime::Permill;

const EXEMPT: AccountId = LP3;
const TOKEN: AssetId = 100;

const ASSET_FEE: Permill = Permill::from_percent(5);
const PROTOCOL_FEE: Permill = Permill::from_percent(3);
const BASE_ASSET_FEE: Permill = Permill::from_parts(2_500);
const BASE_PROTOCOL_FEE: Permill = Permill::from_parts(500);
const MAX_SLIP_FEE: Permill = Permill::from_percent(50);

fn ext() -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, TOKEN, 2000 * ONE),
			(LP1, TOKEN, 5000 * ONE),
			(EXEMPT, TOKEN, 5000 * ONE),
			(EXEMPT, LRNA, 100 * ONE),
		])
		.with_registered_asset(TOKEN)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(TOKEN, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_asset_fee(ASSET_FEE)
		.with_protocol_fee(PROTOCOL_FEE)
		.with_base_fees(BASE_ASSET_FEE, BASE_PROTOCOL_FEE)
		.with_base_fee_only(EXEMPT)
		.build()
}

fn enable_slip_fee() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: MAX_SLIP_FEE,
		})
	));
}

fn stored_slip(asset_in: AssetId, asset_out: AssetId) -> TradeSlipFees {
	TradeSlipFees {
		asset_in_hub_reserve: SlipFeeHubReserveAtBlockStart::<Test>::get(asset_in).expect("snapshot exists"),
		asset_in_delta: SlipFeeDelta::<Test>::get(asset_in),
		asset_out_hub_reserve: SlipFeeHubReserveAtBlockStart::<Test>::get(asset_out).expect("snapshot exists"),
		asset_out_delta: SlipFeeDelta::<Test>::get(asset_out),
		asset_in_max_slip_fee: MAX_SLIP_FEE,
		asset_out_max_slip_fee: MAX_SLIP_FEE,
	}
}

fn expected_sell(
	amount: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
	slip: Option<&TradeSlipFees>,
) -> hydra_dx_math::omnipool::types::TradeStateChange<Balance> {
	hydra_dx_math::omnipool::calculate_sell_state_changes(
		&(&Omnipool::load_asset_state(TOKEN).unwrap()).into(),
		&(&Omnipool::load_asset_state(HDX).unwrap()).into(),
		amount,
		asset_fee,
		protocol_fee,
		Permill::zero(),
		slip,
	)
	.expect("sell math succeeds")
}

fn last_sell_fees() -> (Balance, Balance) {
	System::events()
		.into_iter()
		.rev()
		.find_map(|record| match record.event {
			RuntimeEvent::Omnipool(Event::SellExecuted {
				asset_fee_amount,
				protocol_fee_amount,
				..
			}) => Some((asset_fee_amount, protocol_fee_amount)),
			_ => None,
		})
		.expect("SellExecuted emitted")
}

fn sell_received(who: AccountId, amount: Balance) -> Balance {
	let before = Tokens::free_balance(HDX, &who);
	assert_ok!(Omnipool::sell(RuntimeOrigin::signed(who), TOKEN, HDX, amount, 0));
	Tokens::free_balance(HDX, &who) - before
}

#[test]
fn sell_should_charge_base_fees_when_signer_is_base_fee_only() {
	ext().execute_with(|| {
		let expected = expected_sell(50 * ONE, BASE_ASSET_FEE, BASE_PROTOCOL_FEE, None);

		assert_eq!(sell_received(EXEMPT, 50 * ONE), *expected.asset_out.delta_reserve);
		assert_eq!(last_sell_fees(), (expected.fee.asset_fee, expected.fee.protocol_fee));
	});
}

#[test]
fn sell_should_charge_no_slip_fee_when_signer_is_base_fee_only() {
	ext().execute_with(|| {
		enable_slip_fee();
		sell_received(LP1, 50 * ONE);

		let expected = expected_sell(50 * ONE, BASE_ASSET_FEE, BASE_PROTOCOL_FEE, None);

		assert_eq!(sell_received(EXEMPT, 50 * ONE), *expected.asset_out.delta_reserve);
		assert_eq!(last_sell_fees(), (expected.fee.asset_fee, expected.fee.protocol_fee));
	});
}

#[test]
fn sell_should_charge_full_fees_when_signer_is_not_base_fee_only() {
	ext().execute_with(|| {
		enable_slip_fee();
		sell_received(LP1, 50 * ONE);

		let slip = stored_slip(TOKEN, HDX);
		let expected = expected_sell(50 * ONE, ASSET_FEE, PROTOCOL_FEE, Some(&slip));

		assert_eq!(sell_received(LP1, 50 * ONE), *expected.asset_out.delta_reserve);
		assert_eq!(last_sell_fees(), (expected.fee.asset_fee, expected.fee.protocol_fee));
	});
}

#[test]
fn sell_should_record_slip_fee_delta_when_signer_is_base_fee_only() {
	ext().execute_with(|| {
		enable_slip_fee();
		let token_before = Omnipool::load_asset_state(TOKEN).unwrap().hub_reserve;

		sell_received(EXEMPT, 50 * ONE);

		let hub_in = token_before - Omnipool::load_asset_state(TOKEN).unwrap().hub_reserve;
		let (_, protocol_fee) = last_sell_fees();
		assert_eq!((hub_in, protocol_fee), (31_707_317_073_170, 15_853_658_536));
		assert_eq!(SlipFeeDelta::<Test>::get(TOKEN), SignedBalance::Negative(hub_in));
		assert_eq!(
			SlipFeeDelta::<Test>::get(HDX),
			SignedBalance::Positive(hub_in - protocol_fee)
		);
	});
}

#[test]
fn sell_should_snapshot_block_start_hub_reserve_when_signer_is_base_fee_only() {
	ext().execute_with(|| {
		enable_slip_fee();
		let token_hub_reserve = Omnipool::load_asset_state(TOKEN).unwrap().hub_reserve;
		let hdx_hub_reserve = Omnipool::load_asset_state(HDX).unwrap().hub_reserve;

		sell_received(EXEMPT, 50 * ONE);

		assert_eq!(
			SlipFeeHubReserveAtBlockStart::<Test>::get(TOKEN),
			Some(token_hub_reserve)
		);
		assert_eq!(SlipFeeHubReserveAtBlockStart::<Test>::get(HDX), Some(hdx_hub_reserve));
	});
}

#[test]
fn sell_should_charge_next_trader_slip_on_exempt_flow_when_same_block() {
	ext().execute_with(|| {
		enable_slip_fee();
		sell_received(EXEMPT, 50 * ONE);

		let slip = stored_slip(TOKEN, HDX);
		assert_eq!(slip.asset_in_delta, SignedBalance::Negative(31_707_317_073_170));
		assert_eq!(slip.asset_out_delta, SignedBalance::Positive(31_691_463_414_634));
		let expected = expected_sell(50 * ONE, ASSET_FEE, PROTOCOL_FEE, Some(&slip));

		assert_eq!(sell_received(LP1, 50 * ONE), *expected.asset_out.delta_reserve);
		assert_eq!(last_sell_fees(), (expected.fee.asset_fee, expected.fee.protocol_fee));
	});
}

#[test]
fn buy_should_charge_full_fees_when_signer_is_base_fee_only() {
	ext().execute_with(|| {
		enable_slip_fee();
		sell_received(LP1, 50 * ONE);

		let slip = stored_slip(TOKEN, HDX);
		let expected = hydra_dx_math::omnipool::calculate_buy_state_changes(
			&(&Omnipool::load_asset_state(TOKEN).unwrap()).into(),
			&(&Omnipool::load_asset_state(HDX).unwrap()).into(),
			10 * ONE,
			ASSET_FEE,
			PROTOCOL_FEE,
			Permill::zero(),
			Some(&slip),
		)
		.expect("buy math succeeds");

		let before = Tokens::free_balance(TOKEN, &EXEMPT);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(EXEMPT),
			HDX,
			TOKEN,
			10 * ONE,
			u128::MAX
		));

		assert_eq!(
			before - Tokens::free_balance(TOKEN, &EXEMPT),
			*expected.asset_in.delta_reserve
		);
	});
}

#[test]
fn sell_should_charge_full_fees_when_base_fee_only_signer_sells_hub_asset() {
	ext().execute_with(|| {
		enable_slip_fee();
		sell_received(LP1, 50 * ONE);

		let slip = HubTradeSlipFees {
			asset_hub_reserve: SlipFeeHubReserveAtBlockStart::<Test>::get(HDX).expect("snapshot exists"),
			asset_delta: SlipFeeDelta::<Test>::get(HDX),
			max_slip_fee: MAX_SLIP_FEE,
		};
		let expected = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
			&(&Omnipool::load_asset_state(HDX).unwrap()).into(),
			10 * ONE,
			ASSET_FEE,
			Some(&slip),
		)
		.expect("hub sell math succeeds");

		let before = Tokens::free_balance(HDX, &EXEMPT);
		assert_ok!(Omnipool::sell(RuntimeOrigin::signed(EXEMPT), LRNA, HDX, 10 * ONE, 0));

		assert_eq!(
			Tokens::free_balance(HDX, &EXEMPT) - before,
			*expected.asset.delta_reserve
		);
	});
}
