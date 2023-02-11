#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::assert_ok;

use orml_traits::NamedMultiReservableCurrency;
use pallet_otc::types::OrderId;
use sp_runtime::traits::{BlakeTwo256, Hash};
use xcm_emulator::TestExt;
#[test]
fn place_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			false,
		));

		// Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_some());
	});
}

#[test]
fn fill_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::fill_order(
			hydradx_runtime::Origin::signed(BOB.into()),
			0,
			DAI,
			15 * UNITS,
		));

		//Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_some());

		let reserve_id = named_reserve_identifier(0);
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&reserve_id, HDX.into(), &ALICE.into()),
			25 * UNITS
		);
	});
}

#[test]
fn cancel_order_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::OTC::place_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			DAI,
			HDX,
			20 * UNITS,
			100 * UNITS,
			true,
		));

		// Act
		assert_ok!(hydradx_runtime::OTC::cancel_order(
			hydradx_runtime::Origin::signed(ALICE.into()),
			0
		));

		//Assert
		let order = hydradx_runtime::OTC::orders(0);
		assert!(order.is_none());

		let reserve_id = named_reserve_identifier(0);
		assert_eq!(
			hydradx_runtime::Currencies::reserved_balance_named(&reserve_id, HDX.into(), &ALICE.into()),
			0
		);
	});
}

fn named_reserve_identifier(order_id: OrderId) -> [u8; 8] {
	let prefix = b"otc";
	let mut result = [0; 8];
	result[0..3].copy_from_slice(prefix);
	result[3..7].copy_from_slice(&order_id.to_be_bytes());

	let hashed = BlakeTwo256::hash(&result);
	let mut hashed_array = [0; 8];
	hashed_array.copy_from_slice(&hashed.as_ref()[..8]);
	hashed_array
}
