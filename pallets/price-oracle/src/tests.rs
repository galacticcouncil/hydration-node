// This file is part of pallet-price-oracle.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
pub use crate::mock::{
	ExtBuilder, PriceOracle, RuntimeEvent as TestEvent, RuntimeOrigin, System, Test, ACA, DOT, ETH, HDX,
	ORACLE_ENTRY_1, ORACLE_ENTRY_2,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{OnFinalize, OnInitialize},
};

/// Default oracle source for tests.
const SOURCE: Source = *b"dummysrc";

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

fn expect_events(e: Vec<TestEvent>) {
	test_utils::expect_events::<TestEvent, Test>(e);
}

#[test]
fn genesis_config_works() {
	ExtBuilder::default()
		.with_price_data(vec![
			((HDX, DOT), Price::from(1_000_000), 2_000_000),
			((HDX, ACA), Price::from(3_000_000), 4_000_000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(PriceOracle::num_of_assets(), 2);
			assert_eq!(PriceOracle::price_data_ten().len(), 2);

			assert_eq!(PriceOracle::price_data_ten()[0].0, PriceOracle::get_name(HDX, DOT));
			assert_eq!(
				PriceOracle::price_data_ten()[0].1.get_last(),
				PriceInfo {
					avg_price: Price::from(1_000_000),
					volume: 2_000_000
				}
			);

			assert_eq!(PriceOracle::price_data_ten()[1].0, PriceOracle::get_name(HDX, ACA));
			assert_eq!(
				PriceOracle::price_data_ten()[1].1.get_last(),
				PriceInfo {
					avg_price: Price::from(3_000_000),
					volume: 4_000_000
				}
			);
		});
}

#[test]
fn add_new_asset_pair_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		PriceOracle::on_initialize(3);

		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);
		let hdx_aca_pair_name = PriceOracle::get_name(HDX, ACA);
		let hdx_eth_pair_name = PriceOracle::get_name(HDX, ETH);

		assert_eq!(PriceOracle::num_of_assets(), 0);
		assert_eq!(PriceOracle::new_assets(), vec![AssetPairId::new(); 0]);
		assert!(!<PriceDataTen<Test>>::get().contains(&(hdx_dot_pair_name.clone(), BucketQueue::default())));

		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		assert_eq!(PriceOracle::num_of_assets(), 0);
		assert_eq!(PriceOracle::new_assets(), vec![hdx_dot_pair_name.clone()]);
		assert!(!<PriceDataTen<Test>>::get().contains(&(hdx_dot_pair_name.clone(), BucketQueue::default())));

		PriceOracle::on_finalize(3);
		System::set_block_number(4);
		PriceOracle::on_initialize(4);

		assert_eq!(PriceOracle::num_of_assets(), 1);
		assert!(<PriceDataTen<Test>>::get().contains(&(hdx_dot_pair_name, BucketQueue::default())));

		assert_eq!(PriceOracle::new_assets(), vec![AssetPairId::new(); 0]);

		assert_ok!(PriceOracle::on_create_pool(HDX, ACA));
		assert_ok!(PriceOracle::on_create_pool(HDX, ETH));

		assert_eq!(PriceOracle::num_of_assets(), 1);

		let mut vec_assets = vec![hdx_aca_pair_name.clone(), hdx_eth_pair_name.clone()];
		vec_assets.sort_unstable();

		assert_eq!(PriceOracle::new_assets(), vec_assets);
		assert!(!<PriceDataTen<Test>>::get().contains(&(hdx_aca_pair_name.clone(), BucketQueue::default())));
		assert!(!<PriceDataTen<Test>>::get().contains(&(hdx_eth_pair_name.clone(), BucketQueue::default())));

		PriceOracle::on_finalize(4);
		System::set_block_number(5);
		PriceOracle::on_initialize(5);

		assert_eq!(PriceOracle::num_of_assets(), 3);
		assert!(<PriceDataTen<Test>>::get().contains(&(hdx_aca_pair_name, BucketQueue::default())));
		assert!(<PriceDataTen<Test>>::get().contains(&(hdx_eth_pair_name, BucketQueue::default())));

		assert_eq!(PriceOracle::new_assets(), vec![AssetPairId::new(); 0]);

		expect_events(vec![
			Event::PoolRegistered {
				asset_a: HDX,
				asset_b: DOT,
			}
			.into(),
			Event::PoolRegistered {
				asset_a: HDX,
				asset_b: ACA,
			}
			.into(),
			Event::PoolRegistered {
				asset_a: HDX,
				asset_b: ETH,
			}
			.into(),
		]);
	});
}

#[test]
fn on_create_pool_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		PriceOracle::on_initialize(3);

		// duplicity in the asset queue
		assert!(!<PriceDataTen<Test>>::get().contains(&(PriceOracle::get_name(HDX, DOT), BucketQueue::default())));
		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));
		assert_noop!(PriceOracle::on_create_pool(HDX, DOT), Error::<Test>::AssetAlreadyAdded);

		PriceOracle::on_finalize(3);
		System::set_block_number(4);
		PriceOracle::on_initialize(4);

		// asset already tracked
		assert_noop!(PriceOracle::on_create_pool(HDX, DOT), Error::<Test>::AssetAlreadyAdded);

		PriceOracle::on_finalize(4);
		System::set_block_number(5);
		PriceOracle::on_initialize(5);

		// tracked assets overflow
		TrackedAssetsCount::<Test>::set(u32::MAX - 1);

		assert_ok!(PriceOracle::on_create_pool(HDX, ACA));
		assert_noop!(
			PriceOracle::on_create_pool(HDX, ETH),
			Error::<Test>::TrackedAssetsOverflow
		);

		PriceOracle::on_finalize(5);

		expect_events(vec![
			Event::PoolRegistered {
				asset_a: HDX,
				asset_b: DOT,
			}
			.into(),
			Event::PoolRegistered {
				asset_a: HDX,
				asset_b: ACA,
			}
			.into(),
		]);
	});
}

#[test]
fn on_trade_should_work() {
	new_test_ext().execute_with(|| {
		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);

		assert_eq!(
			<PriceDataAccumulator<Test>>::try_get(hdx_dot_pair_name.clone()),
			Err(())
		);
		PriceOracle::on_trade(HDX, DOT, ORACLE_ENTRY_1);
		PriceOracle::on_trade(HDX, DOT, ORACLE_ENTRY_2);
		let price_entry = ORACLE_ENTRY_1.calculate_new_price_entry(&ORACLE_ENTRY_2);
		assert_eq!(
			<PriceDataAccumulator<Test>>::try_get(hdx_dot_pair_name).ok(),
			price_entry
		);
	});
}

#[test]
fn on_trade_handler_should_work() {
	new_test_ext().execute_with(|| {
		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);

		assert_eq!(
			<PriceDataAccumulator<Test>>::try_get(hdx_dot_pair_name.clone()),
			Err(())
		);

		assert_ok!(PriceOracleHandler::<Test>::on_trade(
			SOURCE, HDX, DOT, 1_000, 500, 2_000, 1_000
		));
		assert_eq!(
			<PriceDataAccumulator<Test>>::try_get(hdx_dot_pair_name),
			Ok(ORACLE_ENTRY_1)
		);
	});
}

#[test]
fn price_normalization_should_work() {
	new_test_ext().execute_with(|| {
		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);

		assert_eq!(
			<PriceDataAccumulator<Test>>::try_get(hdx_dot_pair_name.clone()),
			Err(())
		);

		assert_noop!(
			PriceOracleHandler::<Test>::on_trade(SOURCE, HDX, DOT, Balance::MAX, 1, 2_000, 1_000).map_err(|(_w, e)| e),
			DispatchError::Other("Invalid price")
		);

		assert_noop!(
			PriceOracleHandler::<Test>::on_trade(SOURCE, HDX, DOT, 1, Balance::MAX, 2_000, 1_000).map_err(|(_w, e)| e),
			DispatchError::Other("Invalid price")
		);

		assert_noop!(
			PriceOracleHandler::<Test>::on_trade(SOURCE, HDX, DOT, Balance::zero(), 1_000, 2_000, 1_000)
				.map_err(|(_w, e)| e),
			DispatchError::Other("Invalid values")
		);

		assert_noop!(
			PriceOracleHandler::<Test>::on_trade(SOURCE, HDX, DOT, 1_000, Balance::zero(), 2_000, 1_000)
				.map_err(|(_w, e)| e),
			DispatchError::Other("Invalid price")
		);

		assert_ok!(PriceOracleHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			340282366920938463463,
			1,
			2_000,
			1_000
		));

		assert_noop!(
			PriceOracleHandler::<Test>::on_trade(SOURCE, HDX, DOT, 1, 340282366920938463463, 2_000, 1_000)
				.map_err(|(_w, e)| e),
			DispatchError::Other("Invalid values")
		);

		assert_ok!(PriceOracleHandler::<Test>::on_trade(
			SOURCE, HDX, DOT, 2_000_000, 1_000, 2_000, 1_000
		));

		assert_ok!(PriceOracleHandler::<Test>::on_trade(
			SOURCE, HDX, DOT, 1_000, 2_000_000, 2_000, 1_000
		));

		let price_entry = PriceDataAccumulator::<Test>::get(hdx_dot_pair_name);
		let first_entry = PriceEntry {
			price: Price::from(340282366920938463463),
			trade_amount: 340282366920938463463,
			liquidity_amount: 2_000,
		};

		let second_entry = PriceEntry {
			price: Price::from(2_000),
			trade_amount: 2_000_000,
			liquidity_amount: 2_000,
		};

		let third_entry = PriceEntry {
			price: Price::from_float(0.0005),
			trade_amount: 1_000,
			liquidity_amount: 2_000,
		};

		let result = PriceEntry::default()
			.calculate_new_price_entry(&first_entry)
			.unwrap()
			.calculate_new_price_entry(&second_entry)
			.unwrap()
			.calculate_new_price_entry(&third_entry)
			.unwrap();
		assert_eq!(price_entry, result);
	});
}

#[test]
fn update_data_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		PriceOracle::on_initialize(3);

		assert_ok!(PriceOracle::on_create_pool(HDX, ACA));
		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		PriceOracle::on_finalize(3);
		System::set_block_number(4);
		PriceOracle::on_initialize(4);

		PriceOracle::on_trade(HDX, DOT, ORACLE_ENTRY_1);
		PriceOracle::on_trade(HDX, DOT, ORACLE_ENTRY_2);
		PriceOracle::on_trade(HDX, ACA, ORACLE_ENTRY_1);

		PriceOracle::on_finalize(4);
		System::set_block_number(5);
		PriceOracle::on_initialize(5);

		let data_ten_a = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == PriceOracle::get_name(HDX, DOT))
			.unwrap()
			.1;
		let data_ten_b = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == PriceOracle::get_name(HDX, ACA))
			.unwrap()
			.1;

		assert_eq!(
			data_ten_a.get_last(),
			PriceInfo {
				avg_price: 4.into(),
				volume: 4_000
			}
		);
		assert_eq!(
			data_ten_b.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
	});
}

#[test]
fn update_data_with_incorrect_input_should_not_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		PriceOracle::on_initialize(3);

		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		PriceOracle::on_finalize(3);
		System::set_block_number(4);
		PriceOracle::on_initialize(4);

		PriceOracle::on_trade(
			HDX,
			DOT,
			PriceEntry {
				price: Price::from(1),
				trade_amount: Zero::zero(),
				liquidity_amount: Zero::zero(),
			},
		);

		PriceOracle::on_finalize(4);
		System::set_block_number(5);
		PriceOracle::on_initialize(5);

		let data_ten = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == PriceOracle::get_name(HDX, DOT))
			.unwrap()
			.1;
		assert_eq!(
			data_ten.get_last(),
			PriceInfo {
				avg_price: Zero::zero(),
				volume: Zero::zero()
			}
		);
	});
}

#[test]
fn update_empty_data_should_work() {
	new_test_ext().execute_with(|| {
		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);

		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		for i in 0..1002 {
			PriceOracle::on_initialize(i);
			System::set_block_number(i);
			PriceOracle::on_finalize(i);
		}

		let data_ten = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == hdx_dot_pair_name)
			.unwrap()
			.1;
		assert_eq!(
			data_ten.get_last(),
			PriceInfo {
				avg_price: Zero::zero(),
				volume: Zero::zero()
			}
		);

		let data_hundred = PriceOracle::price_data_hundred(hdx_dot_pair_name.clone());
		assert_eq!(
			data_hundred.get_last(),
			PriceInfo {
				avg_price: Zero::zero(),
				volume: Zero::zero()
			}
		);

		let data_thousand = PriceOracle::price_data_thousand(hdx_dot_pair_name);
		assert_eq!(
			data_thousand.get_last(),
			PriceInfo {
				avg_price: Zero::zero(),
				volume: Zero::zero()
			}
		);
	});
}

#[test]
fn bucket_queue_should_work() {
	let mut queue = BucketQueue::default();
	for i in 0..BucketQueue::BUCKET_SIZE {
		assert_eq!(queue[i as usize], PriceInfo::default());
	}
	assert_eq!(queue.get_last(), PriceInfo::default());

	for i in 0..BucketQueue::BUCKET_SIZE {
		let new_price = Price::from(i as u128);
		queue.update_last(PriceInfo {
			avg_price: new_price,
			volume: 0,
		});
		assert_eq!(
			queue.get_last(),
			PriceInfo {
				avg_price: new_price,
				volume: 0
			}
		);
		// for k in 0..BucketQueue::BUCKET_SIZE {
		//     print!(" {}", queue.bucket[k as usize].avg_price.to_float());
		// }
		// println!();

		for j in 0..BucketQueue::BUCKET_SIZE {
			if i < j {
				assert_eq!(queue[j as usize], PriceInfo::default());
			} else {
				assert_eq!(
					queue[j as usize],
					PriceInfo {
						avg_price: Price::from(j as u128),
						volume: 0
					}
				);
			}
		}
	}

	for i in BucketQueue::BUCKET_SIZE..BucketQueue::BUCKET_SIZE * 3 {
		let new_price = Price::from(i as u128);
		queue.update_last(PriceInfo {
			avg_price: new_price,
			volume: 0,
		});
		// for k in 0..BucketQueue::BUCKET_SIZE {
		// 	print!(" {}", queue.bucket[k as usize].avg_price.to_float());
		// }
		// println!();

		for j in 0..BucketQueue::BUCKET_SIZE {
			if (i % BucketQueue::BUCKET_SIZE) < j {
				assert_eq!(
					queue[j as usize],
					PriceInfo {
						avg_price: Price::from((10 * (i / BucketQueue::BUCKET_SIZE).saturating_sub(1) + j) as u128),
						volume: 0
					}
				);
			} else {
				assert_eq!(
					queue[j as usize],
					PriceInfo {
						avg_price: Price::from((j as u128) + 10u128 * (i / BucketQueue::BUCKET_SIZE) as u128),
						volume: 0
					}
				);
			}
		}
	}
}

#[test]
fn continuous_trades_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		for i in 0..210 {
			System::set_block_number(i);
			PriceOracle::on_initialize(System::block_number());

			PriceOracle::on_trade(
				HDX,
				DOT,
				PriceEntry {
					price: Price::from((i + 1) as u128),
					trade_amount: (i * 1_000).into(),
					liquidity_amount: 1u128,
				},
			);

			// let ten = PriceOracle::price_data_ten().iter().find(|&x| x.0 == ASSET_PAIR_A).unwrap().1;
			// let hundred = PriceOracle::price_data_hundred(ASSET_PAIR_A);
			// let thousand = PriceOracle::price_data_thousand(ASSET_PAIR_A);
			//
			// for i in 0..BUCKET_SIZE {
			// 	print!(" {}", ten[i as usize].avg_price.to_float());
			// }
			// println!();
			//
			// for i in 0..BUCKET_SIZE {
			// 	print!(" {}", hundred[i as usize].avg_price.to_float());
			// }
			// println!();
			//
			// for i in 0..BUCKET_SIZE {
			// 	print!(" {}", thousand[i as usize].avg_price.to_float());
			// }
			// println!("\n");
		}
	})
}

#[test]
fn stable_price_should_work() {
	new_test_ext().execute_with(|| {
		let hdx_dot_pair_name = PriceOracle::get_name(HDX, DOT);

		let num_of_iters = BucketQueue::BUCKET_SIZE.pow(3);
		assert_ok!(PriceOracle::on_create_pool(HDX, DOT));

		for i in num_of_iters - 2..2 * num_of_iters + 2 {
			PriceOracle::on_initialize(i.into());
			System::set_block_number(i.into());
			PriceOracle::on_trade(HDX, DOT, ORACLE_ENTRY_1);
			PriceOracle::on_finalize(i.into());
		}

		let data_ten = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == hdx_dot_pair_name)
			.unwrap()
			.1;
		let data_hundred = PriceOracle::price_data_hundred(hdx_dot_pair_name.clone());
		let data_thousand = PriceOracle::price_data_thousand(hdx_dot_pair_name.clone());

		assert_eq!(
			data_ten.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
		assert_eq!(
			data_hundred.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
		assert_eq!(
			data_thousand.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);

		for i in num_of_iters..2 * num_of_iters {
			PriceOracle::on_initialize(i.into());
			System::set_block_number(i.into());
			PriceOracle::on_finalize(i.into());
		}

		let data_ten = PriceOracle::price_data_ten()
			.iter()
			.find(|&x| x.0 == hdx_dot_pair_name)
			.unwrap()
			.1;
		let data_hundred = PriceOracle::price_data_hundred(hdx_dot_pair_name.clone());
		let data_thousand = PriceOracle::price_data_thousand(hdx_dot_pair_name);

		assert_eq!(
			data_ten.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
		assert_eq!(
			data_hundred.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
		assert_eq!(
			data_thousand.get_last(),
			PriceInfo {
				avg_price: 2.into(),
				volume: 1_000
			}
		);
	});
}
