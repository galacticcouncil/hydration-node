#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Tokens};
use orml_traits::currency::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn staking_should_transfer_hdx_fees_to_pot_account_when_omnipool_trade_is_executed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool();

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			CHARLIE.into(),
			DAI,
			20_000_000 * UNITS,
			0,
		));

		assert_ok!(Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			0u128,
		));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();

		assert_eq!(Currencies::free_balance(HDX, &staking_account), 1093580529360);
	});
}
