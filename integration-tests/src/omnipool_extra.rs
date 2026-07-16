#![cfg(test)]

// Integration coverage for omnipool extrinsics the suite otherwise exercises only
// at the pallet level: the slippage-protected `remove_liquidity_with_limit` and the
// authority-gated `refund_refused_asset`.

use crate::omnipool_init::hydra_run_to_block;
use crate::polkadot_test_net::LRNA;
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::Tradability;
use pretty_assertions::assert_eq;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const DOT_PRICE: FixedU128 = FixedU128::from_inner(25_650_000_000_000_000_000);

// Add DOT to the omnipool with `owner` holding the initial LP position, then advance
// a few blocks and enable removals. Returns the position id created by `add_token`.
fn add_dot_position(owner: AccountId) -> u128 {
	let position_id = Omnipool::next_position_id();

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DOT,
		DOT_PRICE,
		Permill::from_percent(100),
		owner,
	));

	hydra_run_to_block(10);

	assert_ok!(Omnipool::set_asset_tradable_state(
		RuntimeOrigin::root(),
		DOT,
		Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY,
	));

	position_id
}

#[test]
fn remove_liquidity_with_limit_should_return_asset_and_reduce_shares_when_limit_satisfied() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let lp = AccountId::from(BOB);
		let position_id = add_dot_position(lp.clone());

		let position =
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, lp.clone()).unwrap();
		let remove_shares = position.shares / 2;
		assert!(remove_shares > 0, "position must hold enough shares to halve");

		let dot_before = Tokens::free_balance(DOT, &lp);

		assert_ok!(Omnipool::remove_liquidity_with_limit(
			RuntimeOrigin::signed(lp.clone()),
			position_id,
			remove_shares,
			1, // a real, satisfiable min-out limit
		));

		// The withdrawn asset was credited to the LP.
		assert!(Tokens::free_balance(DOT, &lp) > dot_before);

		// Partial removal leaves the position in place with exactly `remove_shares` fewer shares.
		let updated =
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, lp.clone()).unwrap();
		assert_eq!(updated.shares, position.shares - remove_shares);
	});
}

#[test]
fn remove_liquidity_with_limit_should_fail_when_min_limit_exceeds_amount_out() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let lp = AccountId::from(BOB);
		let position_id = add_dot_position(lp.clone());

		let position =
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, lp.clone()).unwrap();

		assert_noop!(
			Omnipool::remove_liquidity_with_limit(
				RuntimeOrigin::signed(lp),
				position_id,
				position.shares,
				u128::MAX, // impossible min-out limit
			),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::SlippageLimit
		);
	});
}

#[test]
fn refund_refused_asset_should_transfer_to_recipient_when_called_by_authority() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		// DOT is registered but not added to the omnipool. Simulate a refused add: the initial
		// liquidity was already transferred to the pool account and now has to be refunded.
		let protocol = Omnipool::protocol_account();
		let recipient = AccountId::from(CHARLIE);
		let refund_amount = 100 * UNITS;

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			protocol.clone(),
			DOT,
			refund_amount,
		));

		let recipient_before = Tokens::free_balance(DOT, &recipient);
		let protocol_before = Tokens::free_balance(DOT, &protocol);

		assert_ok!(Omnipool::refund_refused_asset(
			RuntimeOrigin::root(),
			DOT,
			refund_amount,
			recipient.clone(),
		));

		assert_eq!(Tokens::free_balance(DOT, &recipient), recipient_before + refund_amount);
		assert_eq!(Tokens::free_balance(DOT, &protocol), protocol_before - refund_amount);
	});
}

#[test]
fn refund_refused_asset_should_fail_when_origin_not_authority() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let protocol = Omnipool::protocol_account();
		let refund_amount = 100 * UNITS;
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			protocol,
			DOT,
			refund_amount,
		));

		assert_noop!(
			Omnipool::refund_refused_asset(
				RuntimeOrigin::signed(AccountId::from(CHARLIE)),
				DOT,
				refund_amount,
				AccountId::from(CHARLIE),
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn refund_refused_asset_should_fail_when_asset_already_in_pool() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		// Once DOT is a live omnipool asset it can no longer be treated as a refused refund.
		assert_ok!(Omnipool::add_token(
			RuntimeOrigin::root(),
			DOT,
			DOT_PRICE,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_noop!(
			Omnipool::refund_refused_asset(RuntimeOrigin::root(), DOT, UNITS, AccountId::from(CHARLIE),),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::AssetAlreadyAdded
		);
	});
}

#[test]
fn refund_refused_asset_should_fail_when_asset_is_hub_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		assert_noop!(
			Omnipool::refund_refused_asset(RuntimeOrigin::root(), LRNA, UNITS, AccountId::from(CHARLIE),),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::AssetRefundNotAllowed
		);
	});
}

#[test]
fn refund_refused_asset_should_fail_when_protocol_balance_insufficient() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		// DOT is registered and not in the pool, but the protocol account holds none of it.
		assert_noop!(
			Omnipool::refund_refused_asset(RuntimeOrigin::root(), DOT, 100 * UNITS, AccountId::from(CHARLIE),),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::InsufficientBalance
		);
	});
}
