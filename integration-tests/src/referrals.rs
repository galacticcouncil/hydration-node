#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, Referrals, Runtime, RuntimeOrigin};
use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn registering_a_code_should_charge_registration_fee() {
	Hydra::execute_with(|| {
		let code = b"BALLS69".to_vec();
		let (reg_asset, reg_fee, reg_account) = <Runtime as pallet_referrals::Config>::RegistrationFee::get();
		let balance = Currencies::free_balance(reg_asset, &reg_account);
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE.into()), code));
		let balance_after = Currencies::free_balance(reg_asset, &reg_account);
		let diff = balance_after - balance;
		assert_eq!(diff, reg_fee);
	});
}

#[test]
fn trading_in_omnipool_should_transfer_portion_of_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
	});
}

#[test]
fn trading_in_omnipool_should_increase_referrer_shares() {
	Hydra::execute_with(|| {
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
	});
}
#[test]
fn trading_in_omnipool_should_increase_trader_shares() {
	Hydra::execute_with(|| {
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
	});
}
