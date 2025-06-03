use crate::tests::mock::*;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use pallet_stableswap::types::PegSource;
use sp_runtime::{FixedU128, Perbill, Permill};

#[test]
fn arbitrage_should_work() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		// Create a stablepool for HOLLAR and DAI
		.with_pool(
			pool_id,
			vec![HOLLAR, DAI],
			22,
			Permill::from_percent(0),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1_000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 900 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_float(0.),
			Perbill::from_percent(10),
		)
		.build()
		.execute_with(|| {
			move_block();
			let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
			assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));
			// Set HSM collateral holdings
			assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

			let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
			let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
			let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
			assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI));

			let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);
			let arb_amount = pool_balance_dai_after - pool_balance_dai_before;

			let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_balance_dai_before - hsm_balance_dai_after, arb_amount);
			// Check final HSM balance
			assert_eq!(hsm_balance_dai_after, 100 * ONE - arb_amount);
		});
}
