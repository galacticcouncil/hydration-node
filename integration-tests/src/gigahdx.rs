use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::{TestNet, ALICE, HDX};
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, GigaHdx, RuntimeOrigin};
use orml_traits::MultiCurrency;
use primitives::Balance;
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/gigahdx";

const UNITS: Balance = 1_000_000_000_000;
const STHDX: u32 = 670;
const GIGAHDX: u32 = 67;

/// Requires snapshot with stHDX registered as an AAVE reserve and
/// GIGAHDX configured as the corresponding aToken.
#[test]
fn giga_stake_should_work_on_mainnet_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let alice = sp_runtime::AccountId32::from(ALICE);
		let stake_amount = 1_000 * UNITS;

		// Give ALICE some HDX
		assert_ok!(<hydradx_runtime::Currencies as MultiCurrency<_>>::deposit(
			HDX,
			&alice,
			10_000 * UNITS
		));

		let gigapot = GigaHdx::gigapot_account_id();

		let hdx_before = Currencies::free_balance(HDX, &alice);
		let gigapot_hdx_before = Currencies::free_balance(HDX, &gigapot);

		// Stake 1000 HDX
		assert_ok!(GigaHdx::giga_stake(
			RuntimeOrigin::signed(alice.clone()),
			stake_amount
		));

		// HDX transferred from ALICE to gigapot
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_before - stake_amount);
		assert_eq!(
			Currencies::free_balance(HDX, &gigapot),
			gigapot_hdx_before + stake_amount
		);

		// stHDX minted and supplied to AAVE — alice should not hold any
		assert_eq!(Currencies::free_balance(STHDX, &alice), 0);

		// GIGAHDX (aToken) received by alice via real AAVE supply
		assert_eq!(Currencies::free_balance(GIGAHDX, &alice), stake_amount);

		// Exchange rate and totals updated correctly
		assert_eq!(GigaHdx::total_hdx(), stake_amount);
		assert_eq!(GigaHdx::total_st_hdx_supply(), stake_amount);
		assert_eq!(GigaHdx::exchange_rate(), sp_runtime::FixedU128::from_u32(1));
	});
}
