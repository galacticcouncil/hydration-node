use super::*;
use crate::types::SlipFeeConfig;
use codec::{Decode, Encode};
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};
use hydra_dx_math::omnipool::types::{HubTradeSlipFees, SignedBalance, TradeSlipFees};
use hydradx_traits::router::{PoolType, TradeExecution};
use sp_runtime::{DispatchError, Percent, Permill};

const TOKEN: AssetId = 100;
const SELL_AMOUNT: Balance = 50 * ONE;
const BUY_AMOUNT: Balance = 10 * ONE;
const HUB_ASSET_FEE: Permill = Permill::from_parts(2_500);

fn pct(level: u8) -> Percent {
	Percent::from_percent(level)
}

/// Same pool as the scenarios in `slip_fee.rs`: HDX, DAI and token 100 at 0.65.
fn ext(asset_fee: Permill) -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, TOKEN, 2000 * ONE),
			(LP1, TOKEN, 5000 * ONE),
			(LP1, HDX, 5000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(TOKEN)
		.with_asset_fee(asset_fee)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(TOKEN, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
}

/// Written directly: the hub scenarios use a 100% cap, above what `set_slip_fee` accepts.
fn enable_slip_fee(max_slip_fee: Permill) {
	SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee });
}

fn set_level(asset_id: AssetId, level: u8) {
	assert_ok!(Omnipool::set_asset_slip_fee_level(
		RuntimeOrigin::root(),
		asset_id,
		pct(level)
	));
}

/// Q₀ and cap for one side, computed here instead of through `apply_slip_fee_level`, so the
/// pallet wiring is checked against an independent value.
fn expected_side(q0: Balance, cap: Permill, level: u8) -> (Balance, Permill) {
	match level {
		0 => (q0, Permill::zero()),
		_ => (q0 * 100 / level as Balance, cap),
	}
}

fn last_protocol_fee() -> Balance {
	System::events()
		.into_iter()
		.rev()
		.find_map(|record| match record.event {
			RuntimeEvent::Omnipool(Event::SellExecuted {
				protocol_fee_amount, ..
			})
			| RuntimeEvent::Omnipool(Event::BuyExecuted {
				protocol_fee_amount, ..
			}) => Some(protocol_fee_amount),
			_ => None,
		})
		.expect("trade event")
}

#[derive(Clone, Copy, Debug)]
enum Trade {
	/// Sell token 100 for HDX.
	Sell,
	/// Buy HDX with token 100.
	Buy,
	/// Sell hub asset for token 100.
	SellHub,
	/// Buy token 100 with hub asset.
	BuyForHub,
}

impl Trade {
	fn is_hub(self) -> bool {
		matches!(self, Trade::SellHub | Trade::BuyForHub)
	}

	fn asset_fee(self) -> Permill {
		if self.is_hub() {
			HUB_ASSET_FEE
		} else {
			Permill::zero()
		}
	}

	fn cap(self) -> Permill {
		if self.is_hub() {
			Permill::from_percent(100)
		} else {
			Permill::from_percent(50)
		}
	}

	/// The amount that moves with the slip fee: received for sells, spent for buys.
	fn execute(self) -> Balance {
		match self {
			Trade::Sell => {
				let before = Tokens::free_balance(HDX, &LP1);
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), TOKEN, HDX, SELL_AMOUNT, 0));
				Tokens::free_balance(HDX, &LP1) - before
			}
			Trade::Buy => {
				let before = Tokens::free_balance(TOKEN, &LP1);
				assert_ok!(Omnipool::buy(
					RuntimeOrigin::signed(LP1),
					HDX,
					TOKEN,
					BUY_AMOUNT,
					u128::MAX
				));
				before - Tokens::free_balance(TOKEN, &LP1)
			}
			Trade::SellHub => {
				let before = Tokens::free_balance(TOKEN, &LP3);
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, TOKEN, SELL_AMOUNT, 0));
				Tokens::free_balance(TOKEN, &LP3) - before
			}
			Trade::BuyForHub => {
				let before = Tokens::free_balance(LRNA, &LP3);
				assert_ok!(Omnipool::buy(
					RuntimeOrigin::signed(LP3),
					TOKEN,
					LRNA,
					BUY_AMOUNT,
					u128::MAX
				));
				before - Tokens::free_balance(LRNA, &LP3)
			}
		}
	}

	/// (amount, protocol fee) from the math layer, with the level applied by hand. No prior flow.
	fn math(self, level_in: u8, level_out: u8) -> (Balance, Balance) {
		let cap = self.cap();
		let asset_fee = self.asset_fee();
		let token = Omnipool::load_asset_state(TOKEN).unwrap();
		if self.is_hub() {
			let (asset_hub_reserve, max_slip_fee) = expected_side(token.hub_reserve, cap, level_out);
			let slip = HubTradeSlipFees {
				asset_hub_reserve,
				asset_delta: SignedBalance::zero(),
				max_slip_fee,
			};
			let changes = match self {
				Trade::SellHub => hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
					&(&token).into(),
					SELL_AMOUNT,
					asset_fee,
					Some(&slip),
				),
				_ => hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
					&(&token).into(),
					BUY_AMOUNT,
					asset_fee,
					Some(&slip),
				),
			}
			.unwrap();
			let amount = match self {
				Trade::SellHub => *changes.asset.delta_reserve,
				_ => *changes.asset.delta_hub_reserve,
			};
			return (amount, changes.fee.protocol_fee);
		}
		let hdx = Omnipool::load_asset_state(HDX).unwrap();
		let (asset_in_hub_reserve, asset_in_max_slip_fee) = expected_side(token.hub_reserve, cap, level_in);
		let (asset_out_hub_reserve, asset_out_max_slip_fee) = expected_side(hdx.hub_reserve, cap, level_out);
		let slip = TradeSlipFees {
			asset_in_hub_reserve,
			asset_in_delta: SignedBalance::zero(),
			asset_out_hub_reserve,
			asset_out_delta: SignedBalance::zero(),
			asset_in_max_slip_fee,
			asset_out_max_slip_fee,
		};
		let (in_state, out_state) = ((&token).into(), (&hdx).into());
		match self {
			Trade::Sell => {
				let changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
					&in_state,
					&out_state,
					SELL_AMOUNT,
					asset_fee,
					Permill::zero(),
					Permill::zero(),
					Some(&slip),
				)
				.unwrap();
				(*changes.asset_out.delta_reserve, changes.fee.protocol_fee)
			}
			_ => {
				let changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
					&in_state,
					&out_state,
					BUY_AMOUNT,
					asset_fee,
					Permill::zero(),
					Permill::zero(),
					Some(&slip),
				)
				.unwrap();
				(*changes.asset_in.delta_reserve, changes.fee.protocol_fee)
			}
		}
	}
}

/// Runs `trade` in a fresh pool with the slip fee enabled. `levels` = `None` leaves storage untouched.
/// Returns (amount, protocol fee from the event, math-layer amount and fee).
fn run(trade: Trade, levels: Option<(u8, u8)>) -> ((Balance, Balance), (Balance, Balance)) {
	let mut result = Default::default();
	ext(trade.asset_fee()).execute_with(|| {
		enable_slip_fee(trade.cap());
		let (level_in, level_out) = levels.unwrap_or((100, 100));
		if levels.is_some() {
			if !trade.is_hub() {
				set_level(TOKEN, level_in);
				set_level(HDX, level_out);
			} else {
				set_level(TOKEN, level_out);
			}
		}
		let math = trade.math(level_in, level_out);
		let amount = trade.execute();
		result = ((amount, last_protocol_fee()), math);
	});
	result
}

fn run_without_slip_fee(trade: Trade) -> Balance {
	let mut amount = 0;
	ext(trade.asset_fee()).execute_with(|| {
		set_level(TOKEN, 50);
		if !trade.is_hub() {
			set_level(HDX, 50);
		}
		amount = trade.execute();
	});
	amount
}

/// Level configurations: sell side only, buy side only, both. Hub trades have one side.
fn level_pairs(trade: Trade) -> Vec<(u8, u8)> {
	let levels = [100u8, 50, 10, 0];
	if trade.is_hub() {
		return levels.iter().map(|l| (100, *l)).collect();
	}
	let mut pairs = Vec::new();
	for family in [|l| (l, 100), |l| (100, l), |l| (l, l)] as [fn(u8) -> (u8, u8); 3] {
		pairs.extend(levels.iter().map(|l| family(*l)));
	}
	pairs
}

fn assert_matches_math_layer(trade: Trade) {
	for (level_in, level_out) in level_pairs(trade) {
		let (pallet, math) = run(trade, Some((level_in, level_out)));
		assert_eq!(pallet, math, "{trade:?} at {level_in}%/{level_out}%");
	}
}

fn assert_pinned(trade: Trade, pinned: &[(u8, u8, Balance, Balance)]) {
	let actual: Vec<(u8, u8, Balance, Balance)> = level_pairs(trade)
		.into_iter()
		.map(|(i, o)| {
			let ((amount, fee), _) = run(trade, Some((i, o)));
			(i, o, amount, fee)
		})
		.collect();
	assert_eq!(actual, pinned);
}

/// Within each group of four (100%, 50%, 10%, 0%) the trade must get strictly better.
fn assert_improves_as_level_falls(pinned: &[(u8, u8, Balance, Balance)], is_sell: bool) {
	for group in pinned.chunks(4) {
		for w in group.windows(2) {
			if is_sell {
				assert!(w[1].2 > w[0].2, "{w:?}");
			} else {
				assert!(w[1].2 < w[0].2, "{w:?}");
			}
			assert!(w[1].3 < w[0].3, "{w:?}");
		}
	}
}

// ---------------------------------------------------------------------------
// Extrinsic
// ---------------------------------------------------------------------------

#[test]
fn set_asset_slip_fee_level_should_store_level_when_level_is_zero() {
	ext(Permill::zero()).execute_with(|| {
		set_level(TOKEN, 0);
		assert_eq!(SlipFeeLevel::<Test>::get(TOKEN), pct(0));
		assert!(SlipFeeLevel::<Test>::contains_key(TOKEN));
	});
}

#[test]
fn set_asset_slip_fee_level_should_remove_entry_when_level_is_full() {
	ext(Permill::zero()).execute_with(|| {
		set_level(TOKEN, 50);
		set_level(TOKEN, 100);
		assert!(!SlipFeeLevel::<Test>::contains_key(TOKEN));
		assert_eq!(SlipFeeLevel::<Test>::get(TOKEN), pct(100));
	});
}

#[test]
fn set_asset_slip_fee_level_should_emit_event_with_level() {
	ext(Permill::zero()).execute_with(|| {
		set_level(TOKEN, 33);
		expect_last_events(vec![Event::AssetSlipFeeLevelSet {
			asset_id: TOKEN,
			level: pct(33),
		}
		.into()]);
	});
}

#[test]
fn set_asset_slip_fee_level_should_fail_when_asset_is_not_in_pool() {
	ext(Permill::zero()).execute_with(|| {
		assert_noop!(
			Omnipool::set_asset_slip_fee_level(RuntimeOrigin::root(), 1_234, pct(50)),
			Error::<Test>::AssetNotFound
		);
	});
}

#[test]
fn set_asset_slip_fee_level_should_fail_when_asset_is_hub_asset() {
	ext(Permill::zero()).execute_with(|| {
		assert_noop!(
			Omnipool::set_asset_slip_fee_level(RuntimeOrigin::root(), LRNA, pct(50)),
			Error::<Test>::AssetNotFound
		);
	});
}

#[test]
fn set_asset_slip_fee_level_should_fail_when_origin_is_not_authority() {
	ext(Permill::zero()).execute_with(|| {
		assert_noop!(
			Omnipool::set_asset_slip_fee_level(RuntimeOrigin::signed(LP1), TOKEN, pct(50)),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_asset_slip_fee_level_call_should_fail_to_decode_when_level_is_above_one_hundred_percent() {
	let mut encoded = crate::Call::<Test>::set_asset_slip_fee_level {
		asset_id: TOKEN,
		level: pct(50),
	}
	.encode();
	// The level is the last byte: `Percent` encodes as a single `u8`.
	*encoded.last_mut().unwrap() = 101;
	assert!(crate::Call::<Test>::decode(&mut &encoded[..]).is_err());
}

#[test]
fn remove_token_should_clear_slip_fee_level() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let position_id = <NextPositionId<Test>>::get() - 1;
			set_level(1_000, 50);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::FROZEN
			));
			assert_ok!(Omnipool::sacrifice_position(RuntimeOrigin::signed(LP2), position_id));
			assert_ok!(Omnipool::remove_token(RuntimeOrigin::root(), 1_000, LP1));
			assert!(!SlipFeeLevel::<Test>::contains_key(1_000));
		});
}

#[test]
fn remove_asset_should_clear_slip_fee_level_when_asset_has_level() {
	ext(Permill::zero()).execute_with(|| {
		set_level(TOKEN, 50);
		assert_ok!(Omnipool::remove_asset(TOKEN));
		assert!(!SlipFeeLevel::<Test>::contains_key(TOKEN));
	});
}

// ---------------------------------------------------------------------------
// Defaults change nothing
// ---------------------------------------------------------------------------

#[test]
fn slip_fee_level_should_be_full_when_asset_has_no_entry() {
	ext(Permill::zero()).execute_with(|| {
		assert!(!SlipFeeLevel::<Test>::contains_key(TOKEN));
		assert_eq!(SlipFeeLevel::<Test>::get(TOKEN), Percent::one());
	});
}

#[test]
fn sell_should_charge_full_slip_fee_when_asset_has_no_level_set() {
	let (pallet, math) = run(Trade::Sell, None);
	assert_eq!(pallet, math);
	assert_eq!(pallet, (30_724_665_785_933, 887_959_842_171));
}

#[test]
fn sell_should_charge_same_slip_fee_when_level_is_set_to_full() {
	let mut entry_exists = true;
	let mut explicit = Default::default();
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Trade::Sell.cap());
		set_level(TOKEN, 100);
		set_level(HDX, 100);
		entry_exists = SlipFeeLevel::<Test>::contains_key(TOKEN) || SlipFeeLevel::<Test>::contains_key(HDX);
		explicit = (Trade::Sell.execute(), last_protocol_fee());
	});
	assert!(!entry_exists);
	assert_eq!(explicit, run(Trade::Sell, None).0);
}

#[test]
fn sell_should_charge_same_slip_fee_when_level_is_lowered_and_restored() {
	let mut restored = Default::default();
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Trade::Sell.cap());
		set_level(TOKEN, 50);
		set_level(HDX, 0);
		set_level(TOKEN, 100);
		set_level(HDX, 100);
		restored = (Trade::Sell.execute(), last_protocol_fee());
	});
	assert_eq!(restored, run(Trade::Sell, None).0);
}

#[test]
fn sell_should_charge_no_slip_fee_when_global_slip_fee_is_disabled() {
	let mut plain = 0;
	ext(Permill::zero()).execute_with(|| {
		plain = Trade::Sell.execute();
	});
	assert_eq!(run_without_slip_fee(Trade::Sell), plain);
}

// ---------------------------------------------------------------------------
// Boundaries
// ---------------------------------------------------------------------------

#[test]
fn sell_should_use_new_level_when_level_changes_within_a_block() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Trade::Sell.cap());
		let raw_q0_token = Omnipool::load_asset_state(TOKEN).unwrap().hub_reserve;
		let raw_q0_hdx = Omnipool::load_asset_state(HDX).unwrap().hub_reserve;
		Trade::Sell.execute();

		set_level(TOKEN, 50);
		let token = Omnipool::load_asset_state(TOKEN).unwrap();
		let hdx = Omnipool::load_asset_state(HDX).unwrap();
		let slip = TradeSlipFees {
			asset_in_hub_reserve: raw_q0_token * 2,
			asset_in_delta: SlipFeeDelta::<Test>::get(TOKEN),
			asset_out_hub_reserve: raw_q0_hdx,
			asset_out_delta: SlipFeeDelta::<Test>::get(HDX),
			asset_in_max_slip_fee: Trade::Sell.cap(),
			asset_out_max_slip_fee: Trade::Sell.cap(),
		};
		let expected = hydra_dx_math::omnipool::calculate_sell_state_changes(
			&(&token).into(),
			&(&hdx).into(),
			SELL_AMOUNT,
			Permill::zero(),
			Permill::zero(),
			Permill::zero(),
			Some(&slip),
		)
		.unwrap();

		assert_eq!(Trade::Sell.execute(), *expected.asset_out.delta_reserve);
		assert_eq!(SlipFeeHubReserveAtBlockStart::<Test>::get(TOKEN), Some(raw_q0_token));
	});
}

#[test]
fn sell_should_record_slip_fee_delta_when_level_is_zero() {
	let tracking = |level: u8| {
		let mut state = Default::default();
		ext(Permill::zero()).execute_with(|| {
			enable_slip_fee(Trade::Sell.cap());
			set_level(TOKEN, level);
			set_level(HDX, level);
			Trade::Sell.execute();
			state = (
				SlipFeeDelta::<Test>::get(TOKEN),
				SlipFeeDelta::<Test>::get(HDX),
				SlipFeeHubReserveAtBlockStart::<Test>::get(TOKEN),
				SlipFeeHubReserveAtBlockStart::<Test>::get(HDX),
			);
		});
		state
	};
	let (delta_token, delta_hdx, q0_token, q0_hdx) = tracking(0);
	let (_, _, full_q0_token, full_q0_hdx) = tracking(100);
	assert!(delta_token.is_negative());
	assert!(delta_hdx.is_positive());
	// The deltas differ in size from a 100% run because the trade itself differs; the snapshot does not.
	assert_eq!((q0_token, q0_hdx), (full_q0_token, full_q0_hdx));
	assert_eq!(q0_token, Some(1_300 * ONE));
}

#[test]
fn router_quote_should_match_execution_when_level_is_lowered_for_sell() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Trade::Sell.cap());
		set_level(TOKEN, 50);
		set_level(HDX, 10);
		let quote = <Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
			PoolType::Omnipool,
			TOKEN,
			HDX,
			SELL_AMOUNT,
		)
		.unwrap();
		assert_eq!(Trade::Sell.execute(), quote);
	});
}

#[test]
fn router_quote_should_match_execution_when_level_is_lowered_for_buy() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Trade::Buy.cap());
		set_level(TOKEN, 50);
		set_level(HDX, 10);
		let quote = <Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_in_given_out(
			PoolType::Omnipool,
			TOKEN,
			HDX,
			BUY_AMOUNT,
		)
		.unwrap();
		assert_eq!(Trade::Buy.execute(), quote);
	});
}

#[test]
fn router_quote_should_match_execution_when_level_is_lowered_for_hub_asset_sell() {
	ext(HUB_ASSET_FEE).execute_with(|| {
		enable_slip_fee(Trade::SellHub.cap());
		set_level(TOKEN, 50);
		let quote = <Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
			PoolType::Omnipool,
			LRNA,
			TOKEN,
			SELL_AMOUNT,
		)
		.unwrap();
		assert_eq!(quote, 72_545_454_545_453);
		assert_eq!(Trade::SellHub.execute(), quote);
	});
}

#[test]
fn router_quote_should_match_execution_when_level_is_lowered_for_hub_asset_buy() {
	ext(HUB_ASSET_FEE).execute_with(|| {
		enable_slip_fee(Trade::BuyForHub.cap());
		set_level(TOKEN, 50);
		let quote = <Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_in_given_out(
			PoolType::Omnipool,
			LRNA,
			TOKEN,
			BUY_AMOUNT,
		)
		.unwrap();
		assert_eq!(quote, 6_565_656_565_658);
		assert_eq!(Trade::BuyForHub.execute(), quote);
	});
}

// ---------------------------------------------------------------------------
// Same inputs, different levels
// ---------------------------------------------------------------------------

/// (level_in, level_out, amount, protocol fee) in `level_pairs` order.
const SELL_PINNED: [(u8, u8, Balance, Balance); 12] = [
	(100, 100, 30_724_665_785_933, 887_959_842_171),
	(50, 100, 31_120_952_779_938, 489_210_571_399),
	(10, 100, 31_430_962_105_069, 177_252_943_016),
	(0, 100, 31_507_513_330_101, 100_217_632_373),
	(100, 100, 30_724_665_785_933, 887_959_842_171),
	(100, 50, 30_771_938_662_482, 840_394_907_128),
	(100, 10, 30_809_861_829_619, 802_237_119_229),
	(100, 0, 30_819_357_230_998, 792_682_926_829),
	(100, 100, 30_724_665_785_933, 887_959_842_171),
	(50, 50, 31_169_453_935_143, 440_405_883_224),
	(10, 10, 31_520_125_814_270, 87_525_277_337),
	(0, 0, 31_607_099_440_796, 0),
];
const BUY_PINNED: [(u8, u8, Balance, Balance); 12] = [
	(100, 100, 15_658_737_521_062, 89_099_724_413),
	(50, 100, 15_596_232_147_651, 49_098_787_954),
	(10, 100, 15_547_282_355_729, 17_771_148_673),
	(0, 100, 15_535_187_199_009, 10_030_070_150),
	(100, 100, 15_658_737_521_062, 89_099_724_413),
	(100, 50, 15_650_771_711_402, 84_002_063_094),
	(100, 10, 15_644_404_897_363, 79_927_638_293),
	(100, 0, 15_642_814_003_263, 78_909_546_054),
	(100, 100, 15_658_737_521_062, 89_099_724_413),
	(50, 50, 15_588_330_532_813, 44_041_894_177),
	(10, 10, 15_533_154_685_886, 8_729_222_683),
	(0, 0, 15_519_515_791_109, 0),
];
const SELL_HUB_PINNED: [(u8, u8, Balance, Balance); 4] = [
	(100, 100, 71_250_000_000_000, 1_851_851_851_851),
	(100, 50, 72_545_454_545_453, 943_396_226_415),
	(100, 10, 73_616_236_162_361, 191_570_881_226),
	(100, 0, 73_888_888_888_888, 0),
];
const BUY_FOR_HUB_PINNED: [(u8, u8, Balance, Balance); 4] = [
	(100, 100, 6_582_278_481_014, 33_160_093_104),
	(100, 50, 6_565_656_565_658, 16_538_177_748),
	(100, 10, 6_552_419_354_840, 3_300_966_930),
	(100, 0, 6_549_118_387_910, 0),
];

#[test]
fn sell_should_equal_unset_run_when_levels_are_full() {
	assert_eq!(run(Trade::Sell, Some((100, 100))), run(Trade::Sell, None));
}

#[test]
fn buy_should_equal_unset_run_when_levels_are_full() {
	assert_eq!(run(Trade::Buy, Some((100, 100))), run(Trade::Buy, None));
}

#[test]
fn sell_hub_asset_should_equal_unset_run_when_level_is_full() {
	assert_eq!(run(Trade::SellHub, Some((100, 100))), run(Trade::SellHub, None));
}

#[test]
fn buy_for_hub_asset_should_equal_unset_run_when_level_is_full() {
	assert_eq!(run(Trade::BuyForHub, Some((100, 100))), run(Trade::BuyForHub, None));
}

#[test]
fn sell_should_match_math_layer_when_levels_are_lowered() {
	assert_matches_math_layer(Trade::Sell);
}

#[test]
fn buy_should_match_math_layer_when_levels_are_lowered() {
	assert_matches_math_layer(Trade::Buy);
}

#[test]
fn sell_hub_asset_should_match_math_layer_when_level_is_lowered() {
	assert_matches_math_layer(Trade::SellHub);
}

#[test]
fn buy_for_hub_asset_should_match_math_layer_when_level_is_lowered() {
	assert_matches_math_layer(Trade::BuyForHub);
}

#[test]
fn sell_should_return_pinned_amounts_when_levels_are_lowered() {
	assert_pinned(Trade::Sell, &SELL_PINNED);
	assert_improves_as_level_falls(&SELL_PINNED, true);
}

#[test]
fn buy_should_return_pinned_amounts_when_levels_are_lowered() {
	assert_pinned(Trade::Buy, &BUY_PINNED);
	assert_improves_as_level_falls(&BUY_PINNED, false);
}

#[test]
fn sell_hub_asset_should_return_pinned_amounts_when_level_is_lowered() {
	assert_pinned(Trade::SellHub, &SELL_HUB_PINNED);
	assert_improves_as_level_falls(&SELL_HUB_PINNED, true);
}

#[test]
fn buy_for_hub_asset_should_return_pinned_amounts_when_level_is_lowered() {
	assert_pinned(Trade::BuyForHub, &BUY_FOR_HUB_PINNED);
	assert_improves_as_level_falls(&BUY_FOR_HUB_PINNED, false);
}

#[test]
fn sell_should_equal_disabled_slip_fee_when_both_levels_are_zero() {
	assert_eq!(run(Trade::Sell, Some((0, 0))).0 .0, run_without_slip_fee(Trade::Sell));
}

#[test]
fn buy_should_equal_disabled_slip_fee_when_both_levels_are_zero() {
	assert_eq!(run(Trade::Buy, Some((0, 0))).0 .0, run_without_slip_fee(Trade::Buy));
}

#[test]
fn sell_hub_asset_should_equal_disabled_slip_fee_when_level_is_zero() {
	assert_eq!(
		run(Trade::SellHub, Some((100, 0))).0 .0,
		run_without_slip_fee(Trade::SellHub)
	);
}

#[test]
fn buy_for_hub_asset_should_equal_disabled_slip_fee_when_level_is_zero() {
	assert_eq!(
		run(Trade::BuyForHub, Some((100, 0))).0 .0,
		run_without_slip_fee(Trade::BuyForHub)
	);
}

// ---------------------------------------------------------------------------
// Sequences at 50%
// ---------------------------------------------------------------------------

#[test]
fn consecutive_trades_should_have_increasing_fee_when_level_is_half() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Permill::from_percent(50));
		set_level(TOKEN, 50);
		set_level(HDX, 50);
		let first = Trade::Sell.execute();
		let second = Trade::Sell.execute();
		assert_eq!((first, second), (31_169_453_935_143, 29_102_432_677_487));
		assert!(second < first);
	});
}

#[test]
fn opposing_flow_should_reduce_slip_fee_when_level_is_half() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Permill::from_percent(50));
		set_level(TOKEN, 50);
		set_level(HDX, 50);
		Trade::Sell.execute();
		let first_fee = last_protocol_fee();
		let delta_after_first = SlipFeeDelta::<Test>::get(TOKEN);

		let before = Tokens::free_balance(TOKEN, &LP1);
		assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, TOKEN, SELL_AMOUNT, 0));
		let opposing_out = Tokens::free_balance(TOKEN, &LP1) - before;
		let opposing_fee = last_protocol_fee();

		assert_eq!(
			(first_fee, opposing_out, opposing_fee),
			(440_405_883_224, 77_254_357_498_556, 396_868_210_084)
		);
		assert!(SlipFeeDelta::<Test>::get(TOKEN).abs() < delta_after_first.abs());
	});
}

#[test]
fn max_cap_should_be_respected_during_trade_sequence_when_level_is_half() {
	let total_out = |cap: Option<Permill>| {
		let mut total = 0;
		ext(Permill::zero()).execute_with(|| {
			if let Some(cap) = cap {
				enable_slip_fee(cap);
			}
			set_level(TOKEN, 50);
			set_level(HDX, 50);
			for _ in 0..5 {
				total += Trade::Sell.execute();
			}
		});
		total
	};
	let no_slip = total_out(None);
	let with_slip = total_out(Some(Permill::from_parts(5_000)));
	assert_eq!((no_slip, with_slip), (142_387_732_749_172, 141_156_985_971_147));
	assert!(with_slip < no_slip);
	assert!(with_slip > no_slip * 99 / 100);
}

#[test]
fn on_finalize_should_keep_slip_fee_level_when_block_ends() {
	ext(Permill::zero()).execute_with(|| {
		enable_slip_fee(Permill::from_percent(50));
		set_level(TOKEN, 50);
		Trade::Sell.execute();
		Omnipool::on_finalize(1);
		assert_eq!(SlipFeeLevel::<Test>::get(TOKEN), pct(50));
		assert!(SlipFeeDelta::<Test>::get(TOKEN).is_zero());
	});
}
