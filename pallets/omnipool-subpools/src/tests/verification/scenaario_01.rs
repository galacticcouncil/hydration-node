use super::super::*;
use crate::{add_omnipool_token, create_subpool};
use pallet_omnipool::types::{AssetReserveState, Tradability};

//const ONE: u128 = 1;
const ALICE_INITIAL_LRNA_BALANCE: Balance = 500 * ONE;
const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;

const WDAI: u32 = 3;
const TETHER: u32 = 4;
const AUSD: u32 = 5;
const R1: u32 = 6;
const R2: u32 = 7;

const OMNIPOOL_INITIAL_HDX_BALANCE: Balance = 1_000_000 * ONE;
const OMNIPOOL_INITIAL_DAI_BALANCE: Balance = 1_000_000 * ONE;

const OMNIPOOL_INITIAL_WDAI_BALANCE: Balance = 1_000_000 * ONE;
const OMNIPOOL_INITIAL_TETHER_BALANCE: Balance = 1_000_000 * ONE;

#[test]
fn subpool_trades_should_work_correct_when_trading_between_subpool_and_omnipool() {
	ExtBuilder::default()
		.with_registered_asset(WDAI)
		.with_registered_asset(AUSD)
		.with_registered_asset(TETHER)
		.with_registered_asset(R1)
		.with_registered_asset(R2)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), HDX, OMNIPOOL_INITIAL_HDX_BALANCE),
			(Omnipool::protocol_account(), DAI, OMNIPOOL_INITIAL_DAI_BALANCE),
		])
		.add_endowed_accounts((Omnipool::protocol_account(), WDAI, OMNIPOOL_INITIAL_WDAI_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), R1, OMNIPOOL_INITIAL_WDAI_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), R2, OMNIPOOL_INITIAL_WDAI_BALANCE * 3))
		.add_endowed_accounts((Omnipool::protocol_account(), AUSD, OMNIPOOL_INITIAL_WDAI_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), TETHER, OMNIPOOL_INITIAL_TETHER_BALANCE))
		.add_endowed_accounts((ALICE, R1, 10_000 * ONE))
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(2.0), FixedU128::from_float(0.1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(WDAI, FixedU128::from_float(2.0));
			add_omnipool_token!(TETHER, FixedU128::from_float(2.0));
			add_omnipool_token!(AUSD, FixedU128::from_float(2.000002));
			add_omnipool_token!(R1, FixedU128::from_float(4.0));
			add_omnipool_token!(R2, FixedU128::from_float(1.33333333333));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, WDAI, TETHER);
			/*

			let hdx_state = Omnipool::load_asset_state(HDX);
			let ausd_state = Omnipool::load_asset_state(AUSD);
			let r1_state = Omnipool::load_asset_state(R1);
			let r2_state = Omnipool::load_asset_state(R2);
			let subpool_state = Stableswap::get_pool(SHARE_ASSET_AS_POOL_ID).unwrap();
			let subpool_share_state= Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID);

			dbg!(hdx_state);
			dbg!(&subpool_state);
			dbg!(subpool_state.balances::<Test>());
			dbg!(subpool_share_state);

			dbg!(ausd_state);
			dbg!(r1_state);
			dbg!(r2_state);

			let lrna_amount = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			dbg!(lrna_amount);

			 */

			let amount_to_buy = 1000 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				WDAI,
				R1,
				amount_to_buy,
				u128::MAX,
			));

			let r1_state = Omnipool::load_asset_state(R1).unwrap();
			let subpool_state = Stableswap::get_pool(SHARE_ASSET_AS_POOL_ID).unwrap();
			let subpool_share_state = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

			assert_eq!(
				r1_state,
				AssetReserveState {
					reserve: 1000502008657456330u128,
					hub_reserve: 3997992972915147028u128,
					shares: 1000000000000000000u128,
					protocol_shares: 0u128,
					cap: 1000000000000000000u128,
					tradable: Tradability::default()
				},
			);

			assert_eq!(
				subpool_state.balances::<Test>(),
				vec![999000000000000000, 1000000000000000000]
			);

			assert_eq!(
				subpool_share_state,
				AssetReserveState {
					reserve: 3997993979449541414u128,
					hub_reserve: 4002007027084852972u128,
					shares: 4000000000000000000u128,
					protocol_shares: 0u128,
					cap: 500000000000000000u128,
					tradable: Tradability::default(),
				},
			);
		});
}
