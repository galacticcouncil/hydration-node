#![cfg(test)]

use crate::hsm::{arrange_unhealthy_hollar_position, create_stablepool, mint, minter};
use crate::liquidation::{
	arrange_mm_pool, arrange_unhealthy_mm_position, borrow, deposit_hdx_to_protocol_account, get_oracle_price,
	get_user_account_data, supply, update_oracle_price, ORACLE_ADDRESS, ORACLE_CALLER,
};
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::Contains;
use hydra_dx_math::omnipool::types::{TradeSlipFees, TradeStateChange};
use hydra_dx_math::omnipool::{apply_slip_fee_level, calculate_sell_state_changes};
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{
	Broadcast, Currencies, DynamicFees, Liquidation, LiquidationCollateralSale, Omnipool, Router, Runtime, RuntimeCall,
	RuntimeEvent, RuntimeOrigin, System,
};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::router::{PoolType, Route, Trade};
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use pallet_broadcast::types::{Destination, ExecutionType, Filler, IncrementalIdType};
use pallet_dynamic_fees::types::{AssetFeeConfig, FeeEntry, FeeParams};
use pallet_ice_runtime_api::runtime_decl_for_liquidity_state_api::LiquidityStateApi;
use pallet_omnipool::types::{AssetReserveState, SlipFeeConfig};
use primitives::{AssetId, Balance, BlockNumber, EvmAddress};
use sp_core::U256;
use sp_runtime::traits::Dispatchable;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const DAI: AssetId = 2;
const DOT: AssetId = 5;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH: AssetId = 20;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const HOLLAR: AssetId = 222;
const HOLLAR_UNIT: Balance = 1_000_000_000_000_000_000;
const DOT_HOLLAR_POOL: AssetId = 654321;

const BASE_ASSET_FEE: Permill = Permill::from_parts(3_000);
const BASE_PROTOCOL_FEE: Permill = Permill::from_parts(500);
const RAISED_ASSET_FEE: Permill = Permill::from_percent(20);
const RAISED_PROTOCOL_FEE: Permill = Permill::from_percent(2);
const MAX_SLIP_FEE: Permill = Permill::from_percent(5);
const DOT_BASE_ASSET_FEE: Permill = Permill::from_parts(1_500);
const DOT_BASE_PROTOCOL_FEE: Permill = Permill::from_parts(1_000);

#[derive(Debug, PartialEq, Eq)]
struct OmnipoolSwap {
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	protocol_fee: Balance,
	asset_fee: Balance,
}

impl OmnipoolSwap {
	fn from_math(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		change: &TradeStateChange<Balance>,
	) -> Self {
		Self {
			asset_in,
			asset_out,
			amount_in,
			amount_out: *change.asset_out.delta_reserve,
			protocol_fee: change.fee.protocol_fee,
			asset_fee: change.fee.asset_fee,
		}
	}
}

/// Each Omnipool sell emits two `Swapped3` legs: asset in -> hub asset, then hub asset -> asset out.
/// Returns each sell and the part of its asset fee the fee hooks took out of the pool.
fn omnipool_swaps() -> Vec<(OmnipoolSwap, Balance)> {
	let protocol_account = Omnipool::protocol_account();
	let legs: Vec<_> = System::events()
		.into_iter()
		.filter_map(|record| match record.event {
			RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
				filler_type: Filler::Omnipool,
				inputs,
				outputs,
				fees,
				..
			}) => Some((inputs, outputs, fees)),
			_ => None,
		})
		.collect();

	legs.chunks(2)
		.map(|pair| {
			let (inputs, _, protocol_fees) = &pair[0];
			let (_, outputs, trade_fees) = &pair[1];
			let taken = trade_fees
				.iter()
				.filter(|fee| fee.destination != Destination::Account(protocol_account.clone()))
				.map(|fee| fee.amount)
				.sum();
			(
				OmnipoolSwap {
					asset_in: inputs[0].asset,
					asset_out: outputs[0].asset,
					amount_in: inputs[0].amount,
					amount_out: outputs[0].amount,
					protocol_fee: protocol_fees.iter().map(|fee| fee.amount).sum(),
					asset_fee: trade_fees.iter().map(|fee| fee.amount).sum(),
				},
				taken,
			)
		})
		.collect()
}

fn sell_state_changes(
	in_state: &AssetReserveState<Balance>,
	out_state: &AssetReserveState<Balance>,
	amount_in: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
	slip: Option<&TradeSlipFees>,
) -> TradeStateChange<Balance> {
	calculate_sell_state_changes(
		&in_state.into(),
		&out_state.into(),
		amount_in,
		asset_fee,
		protocol_fee,
		<Runtime as pallet_omnipool::Config>::BurnProtocolFee::get(),
		slip,
	)
	.unwrap()
}

fn base_fee_sell(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	in_state: &AssetReserveState<Balance>,
	out_state: &AssetReserveState<Balance>,
) -> OmnipoolSwap {
	let change = sell_state_changes(in_state, out_state, amount_in, BASE_ASSET_FEE, BASE_PROTOCOL_FEE, None);
	OmnipoolSwap::from_math(asset_in, asset_out, amount_in, &change)
}

/// Must be called right before the sell: it reads the slip totals of the block so far.
fn full_fee_sell(asset_in: AssetId, asset_out: AssetId, amount_in: Balance) -> OmnipoolSwap {
	let slip = current_slip_fees(asset_in, asset_out);
	let change = sell_state_changes(
		&state(asset_in),
		&state(asset_out),
		amount_in,
		RAISED_ASSET_FEE,
		RAISED_PROTOCOL_FEE,
		Some(&slip),
	);
	OmnipoolSwap::from_math(asset_in, asset_out, amount_in, &change)
}

fn current_slip_fees(asset_in: AssetId, asset_out: AssetId) -> TradeSlipFees {
	let max_slip_fee = pallet_omnipool::SlipFee::<Runtime>::get().unwrap().max_slip_fee;
	let side = |asset| {
		let q0 = pallet_omnipool::SlipFeeHubReserveAtBlockStart::<Runtime>::get(asset)
			.unwrap_or_else(|| state(asset).hub_reserve);
		apply_slip_fee_level(q0, max_slip_fee, pallet_omnipool::SlipFeeLevel::<Runtime>::get(asset))
	};
	let (asset_in_hub_reserve, asset_in_max_slip_fee) = side(asset_in);
	let (asset_out_hub_reserve, asset_out_max_slip_fee) = side(asset_out);
	TradeSlipFees {
		asset_in_hub_reserve,
		asset_in_delta: pallet_omnipool::SlipFeeDelta::<Runtime>::get(asset_in),
		asset_out_hub_reserve,
		asset_out_delta: pallet_omnipool::SlipFeeDelta::<Runtime>::get(asset_out),
		asset_in_max_slip_fee,
		asset_out_max_slip_fee,
	}
}

fn state(asset: AssetId) -> AssetReserveState<Balance> {
	Omnipool::load_asset_state(asset).unwrap()
}

fn raise_fees(assets: &[AssetId], stamped_at: BlockNumber) {
	for &asset in assets {
		raise_fee(asset, BASE_ASSET_FEE, BASE_PROTOCOL_FEE, stamped_at);
	}
}

fn raise_fee(asset: AssetId, min_asset_fee: Permill, min_protocol_fee: Permill, stamped_at: BlockNumber) {
	assert_ok!(DynamicFees::set_asset_fee(
		RuntimeOrigin::root(),
		asset,
		AssetFeeConfig::Dynamic {
			asset_fee_params: FeeParams {
				min_fee: min_asset_fee,
				max_fee: RAISED_ASSET_FEE,
				decay: FixedU128::from_rational(1, 60_000),
				amplification: FixedU128::from(2),
			},
			protocol_fee_params: FeeParams {
				min_fee: min_protocol_fee,
				max_fee: RAISED_PROTOCOL_FEE,
				decay: FixedU128::from_rational(5, 600_000),
				amplification: FixedU128::from(1),
			},
		},
	));
	stamp_raised_fee(asset, stamped_at);
}

fn stamp_raised_fee(asset: AssetId, stamped_at: BlockNumber) {
	pallet_dynamic_fees::AssetFee::<Runtime>::insert(
		asset,
		FeeEntry {
			asset_fee: RAISED_ASSET_FEE,
			protocol_fee: RAISED_PROTOCOL_FEE,
			timestamp: stamped_at,
		},
	);
}

fn enable_slip_fee() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: MAX_SLIP_FEE,
		})
	));
}

fn ordinary_sell(asset_in: AssetId, asset_out: AssetId, amount: Balance) {
	assert_ok!(Currencies::deposit(asset_in, &CHARLIE.into(), amount));
	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		asset_in,
		asset_out,
		amount,
		0,
	));
}

/// Full fees above base fees on every traded asset, slip fee on and slip totals already moved this block.
fn raise_fees_and_move_slip(assets: &[AssetId]) {
	raise_fees(assets, System::block_number());
	enable_slip_fee();
	ordinary_sell(WETH, DOT, WETH_UNIT);
}

fn omnipool_route(hops: &[(AssetId, AssetId)]) -> Route<AssetId> {
	Route::truncate_from(
		hops.iter()
			.map(|&(asset_in, asset_out)| Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			})
			.collect::<Vec<_>>(),
	)
}

fn liquidate(collateral: AssetId, debt: AssetId, user: EvmAddress, debt_to_cover: Balance, route: Route<AssetId>) {
	assert_ok!(Liquidation::liquidate(
		RuntimeOrigin::signed(BOB.into()),
		collateral,
		debt,
		user,
		debt_to_cover,
		route,
	));
}

fn set_oracle_price(pair: &str, price: u128) {
	let (_, timestamp) = get_oracle_price(pair).unwrap();
	let timestamp = timestamp.as_u128() + 6;
	let mut data = price.to_be_bytes().to_vec();
	data.extend_from_slice(timestamp.to_be_bytes().as_ref());
	update_oracle_price(
		vec![(pair, U256::from_big_endian(&data[0..32]))],
		ORACLE_ADDRESS,
		ORACLE_CALLER,
	);
}

/// Sets the money market WETH price to the Omnipool WETH/DOT spot price, so the seized collateral
/// is worth on the market what the money market pays for it.
fn align_weth_oracle_price_to_omnipool() {
	let weth = state(WETH);
	let dot = state(DOT);
	let dot_price = get_oracle_price("DOT/USD").unwrap().0;
	let weth_price =
		dot_price * U256::from(weth.hub_reserve) * U256::from(dot.reserve) * U256::from(WETH_UNIT / DOT_UNIT)
			/ (U256::from(weth.reserve) * U256::from(dot.hub_reserve));
	set_oracle_price("WETH/USD", weth_price.as_u128());
}

/// ALICE borrows DOT close to the limit against WETH, then WETH drops on the Omnipool and the
/// money market follows it, so the liquidation bonus is the only margin of the liquidation.
fn arrange_position_liquidatable_after_market_drop() -> (EvmAddress, Balance) {
	let (pool_contract, alice_evm_address) = arrange_mm_pool();
	align_weth_oracle_price_to_omnipool();

	supply(
		pool_contract,
		alice_evm_address,
		HydraErc20Mapping::encode_evm_address(WETH),
		10 * WETH_UNIT,
	);
	let available_borrows = get_user_account_data(pool_contract, alice_evm_address)
		.unwrap()
		.available_borrows_base;
	let dot_price = get_oracle_price("DOT/USD").unwrap().0;
	let borrow_dot_amount = (available_borrows * U256::from(DOT_UNIT) / dot_price).as_u128() * 99 / 100;
	borrow(
		pool_contract,
		alice_evm_address,
		HydraErc20Mapping::encode_evm_address(DOT),
		borrow_dot_amount,
	);

	raise_fees(&[WETH, DOT], System::block_number());
	enable_slip_fee();
	ordinary_sell(WETH, DOT, state(WETH).reserve / 80);
	align_weth_oracle_price_to_omnipool();

	let user_data = get_user_account_data(pool_contract, alice_evm_address).unwrap();
	assert!(user_data.health_factor < U256::from(1_000_000_000_000_000_000u128));

	(alice_evm_address, borrow_dot_amount)
}

fn create_dot_hollar_pool(alice_evm_address: EvmAddress, dot_liquidity: Balance, hollar_liquidity: Balance) {
	assert_ok!(Currencies::deposit(DOT, &ALICE.into(), dot_liquidity));
	mint(minter(), alice_evm_address, hollar_liquidity);
	create_stablepool(
		DOT_HOLLAR_POOL,
		vec![DOT, HOLLAR],
		vec![
			AssetAmount::new(DOT, dot_liquidity),
			AssetAmount::new(HOLLAR, hollar_liquidity),
		],
	);
}

fn weth_dot_hollar_route() -> Route<AssetId> {
	Route::truncate_from(vec![
		Trade {
			pool: PoolType::Omnipool,
			asset_in: WETH,
			asset_out: DOT,
		},
		Trade {
			pool: PoolType::Stableswap(DOT_HOLLAR_POOL),
			asset_in: DOT,
			asset_out: HOLLAR,
		},
	])
}

#[test]
fn liquidation_should_succeed_when_full_fees_would_make_it_unprofitable() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (alice_evm_address, borrow_dot_amount) = arrange_position_liquidatable_after_market_drop();
		System::reset_events();

		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DOT)]),
		);

		let liquidated: Vec<_> = System::events()
			.into_iter()
			.filter_map(|record| match record.event {
				RuntimeEvent::Liquidation(event @ pallet_liquidation::Event::Liquidated { .. }) => Some(event),
				_ => None,
			})
			.collect();
		assert_eq!(
			liquidated,
			vec![pallet_liquidation::Event::Liquidated {
				user: alice_evm_address,
				collateral_asset: WETH,
				debt_asset: DOT,
				profit: 1_784_870_366_486,
			}]
		);
	});
}

#[test]
fn liquidation_should_sell_collateral_at_base_fees_when_debt_is_not_hollar() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		raise_fees_and_move_slip(&[WETH, DOT]);
		raise_fee(DOT, DOT_BASE_ASSET_FEE, DOT_BASE_PROTOCOL_FEE, System::block_number());
		let (weth_before, dot_before) = (state(WETH), state(DOT));
		System::reset_events();

		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DOT)]),
		);

		let swaps = omnipool_swaps();
		assert_eq!(swaps.len(), 1);
		let sold = 9_952_380_952_380_952_381;
		let change = sell_state_changes(
			&weth_before,
			&dot_before,
			sold,
			DOT_BASE_ASSET_FEE,
			BASE_PROTOCOL_FEE,
			None,
		);
		assert_eq!(swaps[0].0, OmnipoolSwap::from_math(WETH, DOT, sold, &change));
	});
}

#[test]
fn liquidation_should_sell_collateral_at_base_fees_when_debt_is_hollar() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(crate::hsm::PATH_TO_SNAPSHOT).execute(|| {
		hydradx_run_to_next_block();
		deposit_hdx_to_protocol_account();
		let (alice_evm_address, hollar_borrow_amount) = arrange_unhealthy_hollar_position();
		create_dot_hollar_pool(alice_evm_address, 20_000 * DOT_UNIT, 20_000 * HOLLAR_UNIT);
		raise_fees_and_move_slip(&[WETH, DOT]);
		let (weth_before, dot_before) = (state(WETH), state(DOT));
		System::reset_events();

		liquidate(
			WETH,
			HOLLAR,
			alice_evm_address,
			hollar_borrow_amount,
			weth_dot_hollar_route(),
		);

		let swaps = omnipool_swaps();
		assert_eq!(swaps.len(), 1);
		assert_eq!(
			swaps[0].0,
			base_fee_sell(WETH, DOT, 1_990_476_190_476_190_476, &weth_before, &dot_before)
		);
	});
}

#[test]
fn liquidation_should_apply_base_fees_to_every_omnipool_hop_when_route_has_two_omnipool_hops() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		raise_fees_and_move_slip(&[WETH, DAI, DOT]);
		let (weth_before, dai_before, dot_before) = (state(WETH), state(DAI), state(DOT));
		System::reset_events();

		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DAI), (DAI, DOT)]),
		);

		let swaps = omnipool_swaps();
		assert_eq!(swaps.len(), 2);
		let sold = 9_952_380_952_380_952_381;
		let first_hop = sell_state_changes(&weth_before, &dai_before, sold, BASE_ASSET_FEE, BASE_PROTOCOL_FEE, None);
		assert_eq!(swaps[0].0, OmnipoolSwap::from_math(WETH, DAI, sold, &first_hop));

		let dai_out = *first_hop.asset_out.delta_reserve;
		let taken = swaps[0].1;
		let mut dai_after_first_hop = dai_before
			.delta_update(&first_hop.account_for_fee_taken(taken).asset_out)
			.unwrap();
		dai_after_first_hop.reserve -= taken;
		assert_eq!(
			swaps[1].0,
			base_fee_sell(DAI, DOT, dai_out, &dai_after_first_hop, &dot_before)
		);
	});
}

#[test]
fn liquidation_should_update_stored_dynamic_fee_when_sale_pays_base_fee() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		// The dynamic fee only restamps an asset that has an oracle entry, so trade once and close the block.
		ordinary_sell(WETH, DOT, WETH_UNIT);
		let previous_block = System::block_number();
		hydradx_run_to_next_block();
		let liquidation_block = System::block_number();
		raise_fees_and_move_slip(&[WETH, DOT]);
		stamp_raised_fee(WETH, previous_block);
		stamp_raised_fee(DOT, previous_block);

		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DOT)]),
		);

		assert_eq!(
			DynamicFees::current_fees(WETH).map(|entry| entry.timestamp),
			Some(liquidation_block)
		);
		assert_eq!(
			DynamicFees::current_fees(DOT).map(|entry| entry.timestamp),
			Some(liquidation_block)
		);
	});
}

#[test]
fn router_sell_should_charge_full_fees_when_after_liquidation_in_same_block() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		raise_fees_and_move_slip(&[WETH, DOT]);
		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DOT)]),
		);

		let amount = WETH_UNIT;
		assert_ok!(Currencies::deposit(WETH, &DAVE.into(), amount));
		let expected = full_fee_sell(WETH, DOT, amount);
		System::reset_events();

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(DAVE.into()),
			WETH,
			DOT,
			amount,
			0,
			omnipool_route(&[(WETH, DOT)]),
		));

		let swaps: Vec<_> = omnipool_swaps().into_iter().map(|(swap, _)| swap).collect();
		assert_eq!(swaps, vec![expected]);
	});
}

#[test]
fn router_sell_should_charge_full_fees_when_after_failed_hollar_liquidation_in_same_block() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(crate::hsm::PATH_TO_SNAPSHOT).execute(|| {
		hydradx_run_to_next_block();
		deposit_hdx_to_protocol_account();
		let (alice_evm_address, hollar_borrow_amount) = arrange_unhealthy_hollar_position();
		// Too little HOLLAR in the pool to repay the flash loan, at any Omnipool fee.
		create_dot_hollar_pool(alice_evm_address, 10 * DOT_UNIT, 10 * HOLLAR_UNIT);
		raise_fees_and_move_slip(&[WETH, DOT]);

		let call = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
			collateral_asset: WETH,
			debt_asset: HOLLAR,
			user: alice_evm_address,
			debt_to_cover: hollar_borrow_amount,
			route: weth_dot_hollar_route(),
		});
		assert_eq!(
			call.dispatch(RuntimeOrigin::signed(BOB.into())).map_err(|e| e.error),
			Err(pallet_liquidation::Error::<Runtime>::NotProfitable.into())
		);

		let amount = WETH_UNIT;
		assert_ok!(Currencies::deposit(WETH, &DAVE.into(), amount));
		let expected = full_fee_sell(WETH, DOT, amount);
		System::reset_events();

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(DAVE.into()),
			WETH,
			DOT,
			amount,
			0,
			omnipool_route(&[(WETH, DOT)]),
		));

		let swaps: Vec<_> = omnipool_swaps().into_iter().map(|(swap, _)| swap).collect();
		assert_eq!(swaps, vec![expected]);
	});
}

#[test]
fn omnipool_sell_should_charge_full_fees_when_signed_by_user() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		deposit_hdx_to_protocol_account();
		raise_fees_and_move_slip(&[WETH, DOT]);
		let amount = WETH_UNIT;
		assert_ok!(Currencies::deposit(WETH, &DAVE.into(), amount));
		let expected = full_fee_sell(WETH, DOT, amount);
		System::reset_events();

		assert_ok!(Omnipool::sell(RuntimeOrigin::signed(DAVE.into()), WETH, DOT, amount, 0,));

		let swaps: Vec<_> = omnipool_swaps().into_iter().map(|(swap, _)| swap).collect();
		assert_eq!(swaps, vec![expected]);
	});
}

#[test]
fn liquidation_should_tag_sale_with_liquidation_context_when_successful() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		let next_id = Broadcast::incremental_id();
		System::reset_events();

		liquidate(
			WETH,
			DOT,
			alice_evm_address,
			borrow_dot_amount,
			omnipool_route(&[(WETH, DOT)]),
		);

		let stacks: Vec<_> = System::events()
			.into_iter()
			.filter_map(|record| match record.event {
				RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
					filler_type: Filler::Omnipool,
					operation_stack,
					..
				}) => Some(operation_stack),
				_ => None,
			})
			.collect();
		let expected = vec![
			ExecutionType::Liquidation(next_id),
			ExecutionType::Router(next_id + 1),
			ExecutionType::Omnipool(next_id + 2),
		];
		assert_eq!(stacks, vec![expected.clone(), expected]);
	});
}

/// The liquidity snapshot as the PEPL worker builds it from the runtime API.
fn worker_liquidity_state() -> pepl_worker::routing::LiquidityState {
	let raw = Runtime::liquidity_state();
	let mut state = pepl_worker::routing::decode_liquidity_state(&raw.state, "test").expect("snapshot decodes");
	let omnipool = std::sync::Arc::make_mut(&mut state.0);
	omnipool.fill_slip_fee_levels::<Runtime>(frame_support::storage::unhashed::get_raw);
	omnipool.base_fees = Some(Runtime::omnipool_base_fees().into_iter().collect());
	state
}

#[test]
fn simulator_sell_should_match_liquidation_sale_when_base_fees_are_set() {
	TestNet::reset();
	hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();
		let (_, alice_evm_address, borrow_dot_amount) = arrange_unhealthy_mm_position();
		raise_fees_and_move_slip(&[WETH, DOT]);
		let (weth_before, dot_before) = (state(WETH), state(DOT));
		let sold = 9_952_380_952_380_952_381;
		let (route, simulated_out) =
			pepl_worker::routing::best_sell_route(&worker_liquidity_state(), WETH, DOT, sold).expect("route found");
		assert_eq!(route, omnipool_route(&[(WETH, DOT)]));
		System::reset_events();

		liquidate(WETH, DOT, alice_evm_address, borrow_dot_amount, route);

		let swaps = omnipool_swaps();
		assert_eq!(swaps.len(), 1);
		assert_eq!(simulated_out, swaps[0].0.amount_out);
		assert_eq!(
			simulated_out,
			base_fee_sell(WETH, DOT, sold, &weth_before, &dot_before).amount_out
		);
	});
}

fn rule_matches(signer: &AccountId, stack: &[fn(IncrementalIdType) -> ExecutionType]) -> bool {
	TestNet::reset();
	Hydra::execute_with(|| {
		for &entry in stack {
			assert_ok!(Broadcast::add_to_context(entry));
		}
		LiquidationCollateralSale::contains(signer)
	})
}

#[test]
fn rule_should_match_when_router_signs_and_stack_is_liquidation_router() {
	assert!(rule_matches(
		&Router::router_account(),
		&[ExecutionType::Liquidation, ExecutionType::Router]
	));
}

#[test]
fn rule_should_match_when_liquidation_is_inside_batch() {
	assert!(rule_matches(
		&Router::router_account(),
		&[ExecutionType::Batch, ExecutionType::Liquidation, ExecutionType::Router]
	));
}

#[test]
fn rule_should_not_match_when_signer_is_not_router() {
	assert!(!rule_matches(
		&ALICE.into(),
		&[ExecutionType::Liquidation, ExecutionType::Router]
	));
}

#[test]
fn rule_should_not_match_when_stack_is_empty() {
	assert!(!rule_matches(&Router::router_account(), &[]));
}

#[test]
fn rule_should_not_match_when_stack_is_liquidation_only() {
	assert!(!rule_matches(&Router::router_account(), &[ExecutionType::Liquidation]));
}

#[test]
fn rule_should_not_match_when_stack_is_router_only() {
	assert!(!rule_matches(&Router::router_account(), &[ExecutionType::Router]));
}

#[test]
fn rule_should_not_match_when_nested_router_is_on_top() {
	assert!(!rule_matches(
		&Router::router_account(),
		&[ExecutionType::Liquidation, ExecutionType::Router, ExecutionType::Router]
	));
}

#[test]
fn rule_should_not_match_when_omnipool_entry_is_on_top() {
	assert!(!rule_matches(
		&Router::router_account(),
		&[
			ExecutionType::Liquidation,
			ExecutionType::Router,
			ExecutionType::Omnipool
		]
	));
}

#[test]
fn rule_should_not_match_when_batch_is_on_top() {
	assert!(!rule_matches(
		&Router::router_account(),
		&[ExecutionType::Liquidation, ExecutionType::Router, ExecutionType::Batch]
	));
}

#[test]
fn rule_should_not_match_when_order_is_reversed() {
	assert!(!rule_matches(
		&Router::router_account(),
		&[ExecutionType::Router, ExecutionType::Liquidation]
	));
}
