#![cfg(test)]

//! Comparison tests between Hydration's stableswap math and Curve's original StableSwap3Pool math.
//!
//! Deploys a Vyper contract (compiled from Curve's math) on EVM via Frontier, then calls both
//! implementations with identical inputs and compares outputs.
//!
//! The Vyper source: scripts/test-contracts/vyper/CurveStableSwapMath.vy
//! Compiled with: vyper 0.4.3

use crate::polkadot_test_net::{Hydra, TestNet};
use crate::utils::contracts::deploy_contract_code;
use ethabi::{encode, short_signature, ParamType, Token};
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Returned;
use hydra_dx_math::stableswap::types::AssetReserve;
use hydra_dx_math::stableswap::*;
use hydradx_runtime::{evm::Executor, EVMAccounts, Runtime};
use hydradx_traits::evm::{CallContext, InspectEvmAccounts, EVM};
use primitives::{AccountId, EvmAddress};
use sp_core::U256;
use sp_runtime::Permill;
use xcm_emulator::{Network, TestExt};

// Vyper compiled bytecode of CurveStableSwapMath.vy (vyper 0.4.3)
const CURVE_MATH_BYTECODE: &str = include_str!("../../scripts/test-contracts/vyper/CurveStableSwapMath.bin");

// Hydration math iteration constants
const D_ITERATIONS: u8 = 64;
const Y_ITERATIONS: u8 = 128;

// Tolerance bounds (in 18-decimal space, i.e. wei)
// Observed maximums: D=2, swap=4, shares=1, withdraw=3, fees=1
const MAX_D_TOLERANCE: u128 = 2;
const _MAX_Y_TOLERANCE: u128 = 4;
const MAX_SWAP_TOLERANCE: u128 = 4;
const MAX_SHARE_TOLERANCE: u128 = 2;
// For fee comparisons: max relative tolerance in basis points (0.01%)
const MAX_FEE_RELATIVE_TOLERANCE_BPS: u128 = 1;

// Curve's fee denominator (10^10)
const _CURVE_FEE_DENOMINATOR: u128 = 10_000_000_000;

// --- Contract deployment ---

fn deployer() -> EvmAddress {
	EVMAccounts::evm_address(&Into::<AccountId>::into(crate::polkadot_test_net::ALICE))
}

fn deploy_curve_math() -> EvmAddress {
	let bytecode_hex = CURVE_MATH_BYTECODE.trim();
	let code = hex::decode(&bytecode_hex[2..]).expect("failed to decode curve math bytecode");
	deploy_contract_code(code, deployer())
}

// --- ABI encoding helpers ---

fn u128_to_token(v: u128) -> Token {
	Token::Uint(U256::from(v))
}

fn u128_array_to_token(arr: &[u128]) -> Token {
	Token::Array(arr.iter().map(|v| u128_to_token(*v)).collect())
}

fn encode_get_d(xp: &[u128], amp: u128) -> Vec<u8> {
	let sig = short_signature(
		"get_D",
		&[ParamType::Array(Box::new(ParamType::Uint(256))), ParamType::Uint(256)],
	);
	let tokens = vec![u128_array_to_token(xp), u128_to_token(amp)];
	let mut data = sig.to_vec();
	data.extend(encode(&tokens));
	data
}

#[allow(dead_code)]
fn encode_get_y(i: usize, j: usize, x: u128, xp: &[u128], amp: u128) -> Vec<u8> {
	let sig = short_signature(
		"get_y",
		&[
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Array(Box::new(ParamType::Uint(256))),
			ParamType::Uint(256),
		],
	);
	let tokens = vec![
		u128_to_token(i as u128),
		u128_to_token(j as u128),
		u128_to_token(x),
		u128_array_to_token(xp),
		u128_to_token(amp),
	];
	let mut data = sig.to_vec();
	data.extend(encode(&tokens));
	data
}

fn encode_get_dy(i: usize, j: usize, dx: u128, balances: &[u128], amp: u128, fee: u128) -> Vec<u8> {
	let sig = short_signature(
		"get_dy",
		&[
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Array(Box::new(ParamType::Uint(256))),
			ParamType::Uint(256),
			ParamType::Uint(256),
		],
	);
	let tokens = vec![
		u128_to_token(i as u128),
		u128_to_token(j as u128),
		u128_to_token(dx),
		u128_array_to_token(balances),
		u128_to_token(amp),
		u128_to_token(fee),
	];
	let mut data = sig.to_vec();
	data.extend(encode(&tokens));
	data
}

fn encode_calc_token_amount(
	old_balances: &[u128],
	new_balances: &[u128],
	amp: u128,
	token_supply: u128,
	fee: u128,
) -> Vec<u8> {
	let sig = short_signature(
		"calc_token_amount",
		&[
			ParamType::Array(Box::new(ParamType::Uint(256))),
			ParamType::Array(Box::new(ParamType::Uint(256))),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
		],
	);
	let tokens = vec![
		u128_array_to_token(old_balances),
		u128_array_to_token(new_balances),
		u128_to_token(amp),
		u128_to_token(token_supply),
		u128_to_token(fee),
	];
	let mut data = sig.to_vec();
	data.extend(encode(&tokens));
	data
}

fn encode_calc_withdraw_one_coin(
	balances: &[u128],
	token_amount: u128,
	i: usize,
	total_supply: u128,
	amp: u128,
	fee: u128,
) -> Vec<u8> {
	let sig = short_signature(
		"calc_withdraw_one_coin",
		&[
			ParamType::Array(Box::new(ParamType::Uint(256))),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
			ParamType::Uint(256),
		],
	);
	let tokens = vec![
		u128_array_to_token(balances),
		u128_to_token(token_amount),
		u128_to_token(i as u128),
		u128_to_token(total_supply),
		u128_to_token(amp),
		u128_to_token(fee),
	];
	let mut data = sig.to_vec();
	data.extend(encode(&tokens));
	data
}

// --- Contract call helpers ---

fn call_view(contract: EvmAddress, data: Vec<u8>) -> Vec<u8> {
	let context = CallContext::new_view(contract);
	let result = Executor::<Runtime>::view(context, data, 5_000_000);
	assert_eq!(
		result.exit_reason,
		Succeed(Returned),
		"EVM call failed: {:?} data: {}",
		result.exit_reason,
		hex::encode(&result.value)
	);
	result.value
}

fn decode_u256(data: &[u8]) -> u128 {
	assert!(data.len() >= 32, "response too short: {} bytes", data.len());
	U256::from_big_endian(&data[..32]).as_u128()
}

fn decode_two_u256(data: &[u8]) -> (u128, u128) {
	assert!(data.len() >= 64, "response too short: {} bytes", data.len());
	let a = U256::from_big_endian(&data[..32]).as_u128();
	let b = U256::from_big_endian(&data[32..64]).as_u128();
	(a, b)
}

// --- Curve contract wrappers ---

fn curve_get_d(contract: EvmAddress, xp: &[u128], amp: u128) -> u128 {
	let data = encode_get_d(xp, amp);
	decode_u256(&call_view(contract, data))
}

#[allow(dead_code)]
fn curve_get_y(contract: EvmAddress, xp: &[u128], i: usize, j: usize, x: u128, amp: u128) -> u128 {
	let data = encode_get_y(i, j, x, xp, amp);
	decode_u256(&call_view(contract, data))
}

fn curve_get_dy(contract: EvmAddress, balances: &[u128], i: usize, j: usize, dx: u128, amp: u128, fee: u128) -> u128 {
	let data = encode_get_dy(i, j, dx, balances, amp, fee);
	decode_u256(&call_view(contract, data))
}

fn curve_calc_token_amount(
	contract: EvmAddress,
	old_balances: &[u128],
	new_balances: &[u128],
	amp: u128,
	token_supply: u128,
	fee: u128,
) -> u128 {
	let data = encode_calc_token_amount(old_balances, new_balances, amp, token_supply, fee);
	decode_u256(&call_view(contract, data))
}

fn curve_calc_withdraw_one_coin(
	contract: EvmAddress,
	balances: &[u128],
	token_amount: u128,
	i: usize,
	total_supply: u128,
	amp: u128,
	fee: u128,
) -> (u128, u128) {
	let data = encode_calc_withdraw_one_coin(balances, token_amount, i, total_supply, amp, fee);
	decode_two_u256(&call_view(contract, data))
}

// --- Hydration math wrappers ---

fn hydra_get_d(xp: &[u128], amp: u128) -> u128 {
	let reserves: Vec<AssetReserve> = xp.iter().map(|v| AssetReserve::new(*v, 18)).collect();
	let pegs: Vec<(u128, u128)> = vec![(1, 1); xp.len()];
	calculate_d::<D_ITERATIONS>(&reserves, amp, &pegs).expect("hydra calculate_d failed")
}

fn hydra_get_dy(balances: &[u128], i: usize, j: usize, dx: u128, amp: u128, fee: Permill) -> u128 {
	let reserves: Vec<AssetReserve> = balances.iter().map(|v| AssetReserve::new(*v, 18)).collect();
	let pegs: Vec<(u128, u128)> = vec![(1, 1); balances.len()];
	if fee == Permill::zero() {
		calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&reserves, i, j, dx, amp, &pegs)
			.expect("hydra calculate_out_given_in failed")
	} else {
		let (amount, _fee) =
			calculate_out_given_in_with_fee::<D_ITERATIONS, Y_ITERATIONS>(&reserves, i, j, dx, amp, fee, &pegs)
				.expect("hydra calculate_out_given_in_with_fee failed");
		amount
	}
}

fn hydra_calc_shares(
	old_balances: &[u128],
	new_balances: &[u128],
	amp: u128,
	share_issuance: u128,
	fee: Permill,
) -> u128 {
	let initial: Vec<AssetReserve> = old_balances.iter().map(|v| AssetReserve::new(*v, 18)).collect();
	let updated: Vec<AssetReserve> = new_balances.iter().map(|v| AssetReserve::new(*v, 18)).collect();
	let pegs: Vec<(u128, u128)> = vec![(1, 1); old_balances.len()];
	let (shares, _fees) =
		calculate_shares::<D_ITERATIONS>(&initial, &updated, amp, share_issuance, fee, &pegs)
			.expect("hydra calculate_shares failed");
	shares
}

fn hydra_calc_withdraw_one_asset(
	balances: &[u128],
	shares: u128,
	i: usize,
	share_issuance: u128,
	amp: u128,
	fee: Permill,
) -> (u128, u128) {
	let reserves: Vec<AssetReserve> = balances.iter().map(|v| AssetReserve::new(*v, 18)).collect();
	let pegs: Vec<(u128, u128)> = vec![(1, 1); balances.len()];
	calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(&reserves, shares, i, share_issuance, amp, fee, &pegs)
		.expect("hydra calculate_withdraw_one_asset failed")
}

// --- Permill <-> Curve fee conversion ---

/// Convert Permill to Curve's FEE_DENOMINATOR (10^10) space.
/// Permill is parts per million (10^6), so multiply by 10^4.
fn permill_to_curve_fee(fee: Permill) -> u128 {
	let parts: u32 = fee.deconstruct();
	(parts as u128) * 10_000
}

// --- Assertion helpers ---

fn assert_parity(label: &str, hydra: u128, curve: u128, max_tolerance: u128, expect_hydra_gte: bool) {
	let diff = hydra.abs_diff(curve);
	assert!(
		diff <= max_tolerance,
		"{}: diff {} exceeds tolerance {} (hydra={}, curve={})",
		label,
		diff,
		max_tolerance,
		hydra,
		curve
	);
	if expect_hydra_gte {
		assert!(
			hydra >= curve,
			"{}: expected hydra ({}) >= curve ({})",
			label,
			hydra,
			curve
		);
	}
}

fn assert_parity_with_fee(label: &str, hydra: u128, curve: u128, max_abs_tolerance: u128) {
	let diff = hydra.abs_diff(curve);
	// Use max of absolute tolerance and relative tolerance
	let relative_tolerance = curve.max(hydra) / 10000 * MAX_FEE_RELATIVE_TOLERANCE_BPS; // 0.01%
	let tolerance = max_abs_tolerance.max(relative_tolerance);
	assert!(
		diff <= tolerance,
		"{}: diff {} exceeds tolerance {} (abs={}, rel={}) (hydra={}, curve={})",
		label,
		diff,
		tolerance,
		max_abs_tolerance,
		relative_tolerance,
		hydra,
		curve
	);
}

// =============================================================================
// D INVARIANT COMPARISON TESTS
// =============================================================================

#[test]
fn curve_comparison_d_balanced_2pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D balanced 2-pool amp=100", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_balanced_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
		];
		let amp = 2000u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D balanced 3-pool amp=2000", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_imbalanced_2pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128, 500_000_000_000_000_000u128];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D imbalanced 2-pool", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_imbalanced_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![
			1_000_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			500_000_000_000_000_000u128,
		];
		let amp = 500u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D imbalanced 3-pool", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_high_amp() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 10_000u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D high amp", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_low_amp() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 1u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D low amp", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_large_reserves() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![
			1_000_000_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000_000_000u128,
		];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D large reserves", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_small_reserves() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000u128, 1_000_000_000_000u128];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D small reserves", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_5pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128; 5];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D 5-pool", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

#[test]
fn curve_comparison_d_extreme_imbalance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let xp = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000u128];
		let amp = 100u128;

		let curve_d = curve_get_d(contract, &xp, amp);
		let hydra_d = hydra_get_d(&xp, amp);

		assert_parity("D extreme imbalance", hydra_d, curve_d, MAX_D_TOLERANCE, true);
	});
}

// =============================================================================
// SWAP OUTPUT COMPARISON TESTS (no fee)
// =============================================================================

fn run_swap_comparison(label: &str, contract: EvmAddress, balances: &[u128], amp: u128, dx_fraction: u128) {
	let dx = balances[0] / dx_fraction;
	let i = 0usize;
	let j = 1usize;

	// Curve: get_dy with fee=0
	let curve_out = curve_get_dy(contract, balances, i, j, dx, amp, 0);

	// Hydration: calculate_out_given_in (no fee)
	let hydra_out = hydra_get_dy(balances, i, j, dx, amp, Permill::zero());

	// Hydration should give slightly less due to +2 bias and -1 rounding
	assert_parity(
		&format!("{} swap {}% no fee", label, 100 / dx_fraction),
		curve_out,
		hydra_out,
		MAX_SWAP_TOLERANCE,
		true, // curve_out >= hydra_out
	);
}

#[test]
fn curve_comparison_swap_balanced_2pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 100u128;

		run_swap_comparison("balanced 2-pool", contract, &balances, amp, 100); // 1%
		run_swap_comparison("balanced 2-pool", contract, &balances, amp, 10); // 10%
		run_swap_comparison("balanced 2-pool", contract, &balances, amp, 10000); // 0.01%
	});
}

#[test]
fn curve_comparison_swap_imbalanced_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![
			1_000_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			500_000_000_000_000_000u128,
		];
		let amp = 500u128;

		run_swap_comparison("imbalanced 3-pool", contract, &balances, amp, 100);
		run_swap_comparison("imbalanced 3-pool", contract, &balances, amp, 10);
		run_swap_comparison("imbalanced 3-pool", contract, &balances, amp, 10000);
	});
}

#[test]
fn curve_comparison_swap_5pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128; 5];
		let amp = 100u128;

		run_swap_comparison("5-pool", contract, &balances, amp, 100);
		run_swap_comparison("5-pool", contract, &balances, amp, 10);
	});
}

// =============================================================================
// SWAP WITH FEE COMPARISON TESTS
// =============================================================================

fn run_swap_with_fee_comparison(
	label: &str,
	contract: EvmAddress,
	balances: &[u128],
	amp: u128,
	dx_fraction: u128,
	fee_permill: Permill,
) {
	let dx = balances[0] / dx_fraction;
	let i = 0usize;
	let j = 1usize;
	let curve_fee = permill_to_curve_fee(fee_permill);

	let curve_out = curve_get_dy(contract, balances, i, j, dx, amp, curve_fee);
	let hydra_out = hydra_get_dy(balances, i, j, dx, amp, fee_permill);

	assert_parity_with_fee(
		&format!("{} swap {}% fee={:?}", label, 100 / dx_fraction, fee_permill),
		hydra_out,
		curve_out,
		MAX_SWAP_TOLERANCE,
	);
}

#[test]
fn curve_comparison_swap_with_fee_004pct() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 100u128;
		let fee = Permill::from_parts(400); // 0.04%

		run_swap_with_fee_comparison("2-pool", contract, &balances, amp, 100, fee);
		run_swap_with_fee_comparison("2-pool", contract, &balances, amp, 10, fee);
	});
}

#[test]
fn curve_comparison_swap_with_fee_03pct() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
		];
		let amp = 2000u128;
		let fee = Permill::from_parts(3000); // 0.3%

		run_swap_with_fee_comparison("3-pool", contract, &balances, amp, 100, fee);
		run_swap_with_fee_comparison("3-pool", contract, &balances, amp, 10, fee);
	});
}

#[test]
fn curve_comparison_swap_with_fee_1pct() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 100u128;
		let fee = Permill::from_parts(10000); // 1%

		run_swap_with_fee_comparison("2-pool 1%", contract, &balances, amp, 100, fee);
		run_swap_with_fee_comparison("2-pool 1%", contract, &balances, amp, 10, fee);
	});
}

// =============================================================================
// ADD LIQUIDITY / SHARE CALCULATION COMPARISON TESTS
// =============================================================================

#[test]
fn curve_comparison_shares_balanced_deposit_no_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let new = vec![1_100_000_000_000_000_000u128, 1_100_000_000_000_000_000u128]; // +10% balanced
		let amp = 100u128;
		let supply = 2_000_000_000_000_000_000u128; // D of balanced pool

		let curve_shares = curve_calc_token_amount(contract, &old, &new, amp, supply, 0);
		let hydra_shares = hydra_calc_shares(&old, &new, amp, supply, Permill::zero());

		assert_parity(
			"shares balanced deposit no fee",
			curve_shares,
			hydra_shares,
			MAX_SHARE_TOLERANCE,
			false,
		);
	});
}

#[test]
fn curve_comparison_shares_single_sided_no_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let new = vec![1_100_000_000_000_000_000u128, 1_000_000_000_000_000_000u128]; // +10% single-sided
		let amp = 100u128;
		let supply = 2_000_000_000_000_000_000u128;

		let curve_shares = curve_calc_token_amount(contract, &old, &new, amp, supply, 0);
		let hydra_shares = hydra_calc_shares(&old, &new, amp, supply, Permill::zero());

		assert_parity(
			"shares single-sided no fee",
			curve_shares,
			hydra_shares,
			MAX_SHARE_TOLERANCE,
			false,
		);
	});
}

#[test]
fn curve_comparison_shares_3pool_imbalanced_no_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![
			1_000_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			500_000_000_000_000_000u128,
		];
		let new = vec![
			1_200_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			600_000_000_000_000_000u128,
		];
		let amp = 500u128;
		let supply = 3_400_000_000_000_000_000u128; // approximate D

		let curve_shares = curve_calc_token_amount(contract, &old, &new, amp, supply, 0);
		let hydra_shares = hydra_calc_shares(&old, &new, amp, supply, Permill::zero());

		assert_parity(
			"shares 3-pool imbalanced no fee",
			curve_shares,
			hydra_shares,
			MAX_SHARE_TOLERANCE,
			false,
		);
	});
}

// --- Shares with fees ---

fn run_shares_with_fee_comparison(
	label: &str,
	contract: EvmAddress,
	old: &[u128],
	new: &[u128],
	amp: u128,
	supply: u128,
	fee: Permill,
) {
	let curve_fee = permill_to_curve_fee(fee);
	let curve_shares = curve_calc_token_amount(contract, old, new, amp, supply, curve_fee);
	let hydra_shares = hydra_calc_shares(old, new, amp, supply, fee);

	assert_parity_with_fee(
		&format!("{} fee={:?}", label, fee),
		hydra_shares,
		curve_shares,
		MAX_SHARE_TOLERANCE,
	);
}

#[test]
fn curve_comparison_shares_with_fee_single_sided() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let new = vec![1_100_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let amp = 100u128;
		let supply = 2_000_000_000_000_000_000u128;

		run_shares_with_fee_comparison("single-sided 0.04%", contract, &old, &new, amp, supply, Permill::from_parts(400));
		run_shares_with_fee_comparison("single-sided 0.3%", contract, &old, &new, amp, supply, Permill::from_parts(3000));
		run_shares_with_fee_comparison("single-sided 1%", contract, &old, &new, amp, supply, Permill::from_parts(10000));
	});
}

#[test]
fn curve_comparison_shares_with_fee_balanced_deposit() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let new = vec![1_100_000_000_000_000_000u128, 1_100_000_000_000_000_000u128];
		let amp = 100u128;
		let supply = 2_000_000_000_000_000_000u128;

		// Balanced deposit should have near-zero fee impact
		run_shares_with_fee_comparison("balanced 0.3%", contract, &old, &new, amp, supply, Permill::from_parts(3000));
		run_shares_with_fee_comparison("balanced 1%", contract, &old, &new, amp, supply, Permill::from_parts(10000));
	});
}

#[test]
fn curve_comparison_shares_with_fee_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let old = vec![
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
		];
		// Single-sided deposit into 3-pool
		let new = vec![
			1_200_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
		];
		let amp = 2000u128;
		let supply = 3_000_000_000_000_000_000u128;

		run_shares_with_fee_comparison("3-pool single-sided 0.04%", contract, &old, &new, amp, supply, Permill::from_parts(400));
		run_shares_with_fee_comparison("3-pool single-sided 0.3%", contract, &old, &new, amp, supply, Permill::from_parts(3000));
		run_shares_with_fee_comparison("3-pool single-sided 1%", contract, &old, &new, amp, supply, Permill::from_parts(10000));
	});
}

// =============================================================================
// SINGLE-ASSET WITHDRAWAL COMPARISON TESTS
// =============================================================================

#[test]
fn curve_comparison_withdraw_no_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let total_supply = 2_000_000_000_000_000_000u128;
		let withdraw_shares = 100_000_000_000_000_000u128; // 5% of supply
		let amp = 100u128;

		let (curve_dy, _) = curve_calc_withdraw_one_coin(contract, &balances, withdraw_shares, 0, total_supply, amp, 0);
		let (hydra_dy, _) = hydra_calc_withdraw_one_asset(&balances, withdraw_shares, 0, total_supply, amp, Permill::zero());

		assert_parity(
			"withdraw no fee amount",
			curve_dy,
			hydra_dy,
			MAX_SWAP_TOLERANCE,
			false,
		);
	});
}

#[test]
fn curve_comparison_withdraw_imbalanced_no_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![
			1_000_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			500_000_000_000_000_000u128,
		];
		let total_supply = 3_400_000_000_000_000_000u128;
		let withdraw_shares = 100_000_000_000_000_000u128;
		let amp = 500u128;

		let (curve_dy, _) = curve_calc_withdraw_one_coin(contract, &balances, withdraw_shares, 0, total_supply, amp, 0);
		let (hydra_dy, _) = hydra_calc_withdraw_one_asset(&balances, withdraw_shares, 0, total_supply, amp, Permill::zero());

		assert_parity(
			"withdraw imbalanced 3-pool no fee",
			curve_dy,
			hydra_dy,
			MAX_SWAP_TOLERANCE,
			false,
		);
	});
}

// --- Withdrawal with fees ---

fn run_withdraw_with_fee_comparison(
	label: &str,
	contract: EvmAddress,
	balances: &[u128],
	withdraw_shares: u128,
	i: usize,
	total_supply: u128,
	amp: u128,
	fee: Permill,
) {
	let curve_fee_val = permill_to_curve_fee(fee);
	let (curve_dy, curve_fee_amount) =
		curve_calc_withdraw_one_coin(contract, balances, withdraw_shares, i, total_supply, amp, curve_fee_val);
	let (hydra_dy, hydra_fee_amount) =
		hydra_calc_withdraw_one_asset(balances, withdraw_shares, i, total_supply, amp, fee);

	assert_parity_with_fee(
		&format!("{} withdraw amount", label),
		hydra_dy,
		curve_dy,
		MAX_SWAP_TOLERANCE,
	);
	assert_parity_with_fee(
		&format!("{} withdraw fee", label),
		hydra_fee_amount,
		curve_fee_amount,
		MAX_SWAP_TOLERANCE,
	);
}

#[test]
fn curve_comparison_withdraw_with_fee_2pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![1_000_000_000_000_000_000u128, 1_000_000_000_000_000_000u128];
		let total_supply = 2_000_000_000_000_000_000u128;
		let withdraw_shares = 100_000_000_000_000_000u128;
		let amp = 100u128;

		run_withdraw_with_fee_comparison("2-pool 0.04%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(400));
		run_withdraw_with_fee_comparison("2-pool 0.3%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(3000));
		run_withdraw_with_fee_comparison("2-pool 1%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(10000));
	});
}

#[test]
fn curve_comparison_withdraw_with_fee_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
			1_000_000_000_000_000_000u128,
		];
		let total_supply = 3_000_000_000_000_000_000u128;
		let withdraw_shares = 150_000_000_000_000_000u128; // 5%
		let amp = 2000u128;

		run_withdraw_with_fee_comparison("3-pool 0.04%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(400));
		run_withdraw_with_fee_comparison("3-pool 0.3%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(3000));
		run_withdraw_with_fee_comparison("3-pool 1%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(10000));
	});
}

#[test]
fn curve_comparison_withdraw_with_fee_imbalanced_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_curve_math();
		let balances = vec![
			1_000_000_000_000_000_000u128,
			2_000_000_000_000_000_000u128,
			500_000_000_000_000_000u128,
		];
		let total_supply = 3_400_000_000_000_000_000u128;
		let withdraw_shares = 100_000_000_000_000_000u128;
		let amp = 500u128;

		run_withdraw_with_fee_comparison("imbalanced 3-pool 0.3%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(3000));
		run_withdraw_with_fee_comparison("imbalanced 3-pool 1%", contract, &balances, withdraw_shares, 0, total_supply, amp, Permill::from_parts(10000));
	});
}

// =============================================================================
// BALANCED ADD + PROPORTIONAL REMOVE CYCLE TEST
// =============================================================================
// Tests whether repeated balanced deposits and proportional withdrawals can
// extract value from the pool through rounding.

#[test]
fn curve_comparison_balanced_add_remove_cycle_no_value_extraction() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let amp = 100u128;
		let n_assets = 2usize;
		let fee = Permill::from_parts(500); // 0.05%

		// Initial pool state
		let mut reserves = vec![1_000_000_000_000_000_000u128; n_assets];
		let mut share_issuance = hydra_get_d(&reserves, amp);

		// Attacker starts with these tokens
		let deposit_per_asset = 100_000_000_000_000_000u128; // 0.1 token per asset
		let mut attacker_balances: Vec<u128> = vec![deposit_per_asset; n_assets];

		let iterations = 100u32;

		for _ in 0..iterations {
			// Step 1: Balanced deposit
			let new_reserves: Vec<u128> = reserves
				.iter()
				.zip(attacker_balances.iter())
				.map(|(r, a)| r + a)
				.collect();

			let shares_received = hydra_calc_shares(&reserves, &new_reserves, amp, share_issuance, fee);
			assert!(shares_received > 0, "should receive shares");

			// Update pool state after deposit
			reserves = new_reserves;
			share_issuance += shares_received;

			// Step 2: Proportional withdrawal of all shares received
			let mut withdrawn: Vec<u128> = Vec::new();
			for i in 0..n_assets {
				let amount = calculate_liquidity_out(reserves[i], shares_received, share_issuance)
					.expect("liquidity out failed");
				withdrawn.push(amount);
			}

			// Update pool state after withdrawal
			for i in 0..n_assets {
				reserves[i] -= withdrawn[i];
			}
			share_issuance -= shares_received;

			// Update attacker balances
			attacker_balances = withdrawn;
		}

		// Check: attacker should NOT have more than they started with
		let initial_total = deposit_per_asset * n_assets as u128;
		let final_total: u128 = attacker_balances.iter().sum();

		eprintln!(
			"balanced add+remove cycle ({}x): initial={} final={} diff={} ({})",
			iterations,
			initial_total,
			final_total,
			initial_total as i128 - final_total as i128,
			if final_total <= initial_total {
				"protocol safe"
			} else {
				"VALUE EXTRACTED"
			}
		);

		assert!(
			final_total <= initial_total,
			"attacker extracted value! initial={} final={} profit={}",
			initial_total,
			final_total,
			final_total - initial_total,
		);
	});
}

#[test]
fn curve_comparison_balanced_add_remove_cycle_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let amp = 2000u128;
		let n_assets = 3usize;
		let fee = Permill::from_parts(500); // 0.05%

		let mut reserves = vec![1_000_000_000_000_000_000u128; n_assets];
		let mut share_issuance = hydra_get_d(&reserves, amp);

		let deposit_per_asset = 100_000_000_000_000_000u128;
		let mut attacker_balances: Vec<u128> = vec![deposit_per_asset; n_assets];

		let iterations = 100u32;

		for _ in 0..iterations {
			let new_reserves: Vec<u128> = reserves
				.iter()
				.zip(attacker_balances.iter())
				.map(|(r, a)| r + a)
				.collect();

			let shares_received = hydra_calc_shares(&reserves, &new_reserves, amp, share_issuance, fee);
			assert!(shares_received > 0);

			reserves = new_reserves;
			share_issuance += shares_received;

			let mut withdrawn: Vec<u128> = Vec::new();
			for i in 0..n_assets {
				let amount = calculate_liquidity_out(reserves[i], shares_received, share_issuance)
					.expect("liquidity out failed");
				withdrawn.push(amount);
			}

			for i in 0..n_assets {
				reserves[i] -= withdrawn[i];
			}
			share_issuance -= shares_received;

			attacker_balances = withdrawn;
		}

		let initial_total = deposit_per_asset * n_assets as u128;
		let final_total: u128 = attacker_balances.iter().sum();

		eprintln!(
			"3-pool balanced add+remove cycle ({}x): initial={} final={} diff={} ({})",
			iterations,
			initial_total,
			final_total,
			initial_total as i128 - final_total as i128,
			if final_total <= initial_total {
				"protocol safe"
			} else {
				"VALUE EXTRACTED"
			}
		);

		assert!(
			final_total <= initial_total,
			"attacker extracted value! initial={} final={} profit={}",
			initial_total,
			final_total,
			final_total - initial_total,
		);
	});
}

#[test]
fn curve_comparison_balanced_add_remove_cycle_zero_fee() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let amp = 100u128;
		let n_assets = 2usize;
		let fee = Permill::zero(); // no fee — worst case for protocol

		let mut reserves = vec![1_000_000_000_000_000_000u128; n_assets];
		let mut share_issuance = hydra_get_d(&reserves, amp);

		let deposit_per_asset = 100_000_000_000_000_000u128;
		let mut attacker_balances: Vec<u128> = vec![deposit_per_asset; n_assets];

		let iterations = 1000u32;

		for _ in 0..iterations {
			let new_reserves: Vec<u128> = reserves
				.iter()
				.zip(attacker_balances.iter())
				.map(|(r, a)| r + a)
				.collect();

			let shares_received = hydra_calc_shares(&reserves, &new_reserves, amp, share_issuance, fee);
			assert!(shares_received > 0);

			reserves = new_reserves;
			share_issuance += shares_received;

			let mut withdrawn: Vec<u128> = Vec::new();
			for i in 0..n_assets {
				let amount = calculate_liquidity_out(reserves[i], shares_received, share_issuance)
					.expect("liquidity out failed");
				withdrawn.push(amount);
			}

			for i in 0..n_assets {
				reserves[i] -= withdrawn[i];
			}
			share_issuance -= shares_received;

			attacker_balances = withdrawn;
		}

		let initial_total = deposit_per_asset * n_assets as u128;
		let final_total: u128 = attacker_balances.iter().sum();

		eprintln!(
			"zero-fee balanced add+remove cycle ({}x): initial={} final={} diff={} ({})",
			iterations,
			initial_total,
			final_total,
			initial_total as i128 - final_total as i128,
			if final_total <= initial_total {
				"protocol safe"
			} else {
				"VALUE EXTRACTED"
			}
		);

		assert!(
			final_total <= initial_total,
			"attacker extracted value! initial={} final={} profit={}",
			initial_total,
			final_total,
			final_total - initial_total,
		);
	});
}

// =============================================================================
// IMBALANCED POOL: BALANCED ADD + PROPORTIONAL REMOVE CYCLE TESTS
// =============================================================================

/// Helper: run proportional add + proportional remove cycle on any pool state.
fn run_add_remove_cycle(
	label: &str,
	initial_reserves: Vec<u128>,
	amp: u128,
	fee: Permill,
	deposit_fraction: u128, // deposit = reserve[i] / deposit_fraction
	iterations: u32,
) {
	let n_assets = initial_reserves.len();
	let mut reserves = initial_reserves;
	let mut share_issuance = hydra_get_d(&reserves, amp);

	// Attacker deposits proportional to current reserves
	let mut attacker_balances: Vec<u128> = reserves.iter().map(|r| r / deposit_fraction).collect();
	let initial_total: u128 = attacker_balances.iter().sum();

	for _ in 0..iterations {
		let new_reserves: Vec<u128> = reserves
			.iter()
			.zip(attacker_balances.iter())
			.map(|(r, a)| r + a)
			.collect();

		let shares_received = hydra_calc_shares(&reserves, &new_reserves, amp, share_issuance, fee);
		if shares_received == 0 {
			break;
		}

		reserves = new_reserves;
		share_issuance += shares_received;

		let mut withdrawn: Vec<u128> = Vec::new();
		for i in 0..n_assets {
			let amount = calculate_liquidity_out(reserves[i], shares_received, share_issuance)
				.expect("liquidity out failed");
			withdrawn.push(amount);
		}

		for i in 0..n_assets {
			reserves[i] -= withdrawn[i];
		}
		share_issuance -= shares_received;

		attacker_balances = withdrawn;
	}

	let final_total: u128 = attacker_balances.iter().sum();

	eprintln!(
		"  {}: initial={} final={} diff={} ({})",
		label,
		initial_total,
		final_total,
		initial_total as i128 - final_total as i128,
		if final_total <= initial_total {
			"protocol safe"
		} else {
			"VALUE EXTRACTED"
		}
	);

	assert!(
		final_total <= initial_total,
		"{}: attacker extracted value! initial={} final={} profit={}",
		label,
		initial_total,
		final_total,
		final_total - initial_total,
	);
}

#[test]
fn curve_comparison_add_remove_cycle_imbalanced_2pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let fee = Permill::from_parts(500); // 0.05%
		let no_fee = Permill::zero();

		run_add_remove_cycle(
			"2-pool 2:1 fee=0.05%",
			vec![1_000_000_000_000_000_000, 500_000_000_000_000_000],
			100, fee, 10, 100,
		);
		run_add_remove_cycle(
			"2-pool 2:1 fee=0",
			vec![1_000_000_000_000_000_000, 500_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
		run_add_remove_cycle(
			"2-pool 10:1 fee=0.05%",
			vec![1_000_000_000_000_000_000, 100_000_000_000_000_000],
			100, fee, 10, 100,
		);
		run_add_remove_cycle(
			"2-pool 10:1 fee=0",
			vec![1_000_000_000_000_000_000, 100_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
		run_add_remove_cycle(
			"2-pool 100:1 fee=0.05%",
			vec![1_000_000_000_000_000_000, 10_000_000_000_000_000],
			100, fee, 10, 100,
		);
		run_add_remove_cycle(
			"2-pool 100:1 fee=0",
			vec![1_000_000_000_000_000_000, 10_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
		run_add_remove_cycle(
			"2-pool 1000:1 fee=0",
			vec![1_000_000_000_000_000_000, 1_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
	});
}

#[test]
fn curve_comparison_add_remove_cycle_imbalanced_3pool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let fee = Permill::from_parts(500);
		let no_fee = Permill::zero();

		run_add_remove_cycle(
			"3-pool [1:2:0.5] fee=0.05%",
			vec![1_000_000_000_000_000_000, 2_000_000_000_000_000_000, 500_000_000_000_000_000],
			500, fee, 10, 100,
		);
		run_add_remove_cycle(
			"3-pool [1:2:0.5] fee=0",
			vec![1_000_000_000_000_000_000, 2_000_000_000_000_000_000, 500_000_000_000_000_000],
			500, no_fee, 10, 1000,
		);
		run_add_remove_cycle(
			"3-pool [10:1:1] fee=0",
			vec![10_000_000_000_000_000_000, 1_000_000_000_000_000_000, 1_000_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
		run_add_remove_cycle(
			"3-pool [1:1:0.01] fee=0",
			vec![1_000_000_000_000_000_000, 1_000_000_000_000_000_000, 10_000_000_000_000_000],
			100, no_fee, 10, 1000,
		);
	});
}

#[test]
fn curve_comparison_add_remove_cycle_imbalanced_varying_amp() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let no_fee = Permill::zero();
		let reserves = vec![1_000_000_000_000_000_000u128, 100_000_000_000_000_000u128];

		run_add_remove_cycle("10:1 amp=1 fee=0", reserves.clone(), 1, no_fee, 10, 1000);
		run_add_remove_cycle("10:1 amp=100 fee=0", reserves.clone(), 100, no_fee, 10, 1000);
		run_add_remove_cycle("10:1 amp=10000 fee=0", reserves.clone(), 10000, no_fee, 10, 1000);
	});
}

#[test]
fn curve_comparison_add_remove_cycle_small_deposits_imbalanced() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let no_fee = Permill::zero();

		// Very small deposits — maximizes rounding impact relative to deposit size
		run_add_remove_cycle(
			"2-pool 10:1 tiny deposit fee=0",
			vec![1_000_000_000_000_000_000, 100_000_000_000_000_000],
			100, no_fee, 10000, 1000,
		);
		run_add_remove_cycle(
			"2-pool 10:1 micro deposit fee=0",
			vec![1_000_000_000_000_000_000, 100_000_000_000_000_000],
			100, no_fee, 1_000_000, 1000,
		);
	});
}
