use crate::tests::mock::*;
use crate::ERC20Function;
use crate::EvmAddress;
use crate::{CollateralHoldings, Error, HollarAmountReceived};
use frame_support::traits::Hooks;
use frame_support::{assert_err, assert_noop, assert_ok};
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_stableswap::types::PegSource;
use sp_runtime::{DispatchError, Perbill, Permill};

#[test]
fn arbitrage_should_work() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, DAI, 1_000 * ONE_18),
			(HSM::account_id(), DAI, 100 * ONE_18),
		])
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
					amount: 1_000 * ONE_18,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 900 * ONE_18,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			(100, 100),
			Permill::from_float(0.),
			Perbill::from_percent(10),
		)
		.build()
		.execute_with(|| {
			CollateralHoldings::<Test>::insert(DAI, 100 * ONE_18);
			let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
			let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
			let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
			assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI,));
			let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);
			let arb_amount = pool_balance_dai_after - pool_balance_dai_before;

			let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_balance_dai_before - hsm_balance_dai_after, arb_amount);
			let holding = CollateralHoldings::<Test>::get(DAI);
			assert_eq!(holding, 100 * ONE_18 - arb_amount);
		});
}
