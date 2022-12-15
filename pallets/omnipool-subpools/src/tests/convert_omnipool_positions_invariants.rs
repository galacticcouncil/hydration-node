use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
use primitive_types::U256;
use proptest::prelude::*;
use test_utils::assert_balance;

proptest! {
	//Spec: https://www.notion.so/Convert-Omnipool-position-to-Stableswap-Subpool-position-b18dabaa55bf433fa96f4ebf67cecec4
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_lrna_for_stableswap_asset(
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		amount in asset_reserve(),
		balance in asset_reserve(),

	) {
			ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((LP1, asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((LP1, asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(
					Origin::root(),
					asset_3.asset_id,
					asset_3.price,
					Permill::from_percent(100),
					LP1
				));
				assert_ok!(Omnipool::add_token(
					Origin::root(),
					asset_4.asset_id,
					asset_4.price,
					Permill::from_percent(100),
					LP1
				));
				let asset_3_state = Omnipool::load_asset_state(ASSET_3).unwrap();
				create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

				let share_asset_state = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let position = Position {
					asset_id: asset_3.asset_id,
					amount: asset_3_state.reserve,
					price: (asset_3_state.hub_reserve, asset_3_state.reserve),
					shares: asset_3_state.shares,
				};

				let migration_details_for_asset_3 = OmnipoolSubpools::migrated_assets(asset_3.asset_id).unwrap().1;

				//Act
				let converted_position = OmnipoolSubpools::convert_position(
					SHARE_ASSET_AS_POOL_ID,
					migration_details_for_asset_3.clone(),
					position.clone(),
				)
				.unwrap();

				assert_eq!(
					converted_position.shares * migration_details_for_asset_3.shares,
					position.shares * migration_details_for_asset_3.hub_reserve
				);

				let new_price = FixedU128::from_rational(converted_position.price.0, converted_position.price.1);
				let price_from_migration_details = FixedU128::from_rational(
					migration_details_for_asset_3.price.0,
					migration_details_for_asset_3.price.1,
				);
				let old_price = FixedU128::from_rational(position.price.0, position.price.1);

				assert_eq!(new_price * price_from_migration_details, old_price);
			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
