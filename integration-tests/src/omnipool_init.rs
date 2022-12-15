#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::assert_ok;

use orml_traits::currency::MultiCurrency;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

#[test]
fn omnipool_launch_init_params_should_be_correct() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();
		let stable_amount = 50_000 * UNITS * 1_000_000;
		let native_amount = 936_329_588_000_000_000;
		let dot_amount = 87_719_298_250_000_u128;

		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::Origin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));
		let hdx_balance = hydradx_runtime::Balances::free_balance(&omnipool_account);
		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &omnipool_account);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_account);

		assert_eq!(lrna_balance, 3374999999982000);
		assert_eq!(dai_balance, stable_amount);
		assert_eq!(hdx_balance, native_amount);

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::Origin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		let hdx_balance = hydradx_runtime::Balances::free_balance(&omnipool_account);
		let dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &omnipool_account);
		let lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_account);
		let dot_balance = hydradx_runtime::Tokens::free_balance(DOT, &omnipool_account);

		assert_eq!(lrna_balance, 5625000000094500);
		assert_eq!(dai_balance, stable_amount);
		assert_eq!(hdx_balance, native_amount);
		assert_eq!(dot_balance, dot_amount);
	});
}
