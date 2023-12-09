// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
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

mod invariants;
mod mock;

use super::*;
pub use mock::{
	BlockNumber, EmaOracle, ExtBuilder, RuntimeEvent as TestEvent, RuntimeOrigin, System, Test, ACA, DOT, HDX,
	ORACLE_ENTRY_1, ORACLE_ENTRY_2,
};

use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;
use rug::Rational;

/// Default oracle source for tests.
pub(crate) const SOURCE: Source = *b"dummysrc";

fn supported_periods() -> BoundedVec<OraclePeriod, ConstU32<MAX_PERIODS>> {
	<Test as crate::Config>::SupportedPeriods::get()
}

macro_rules! assert_price_approx_eq {
	($x:expr, $y:expr, $z:expr) => {{
		assert_price_approx_eq!($x, $y, $z, "not approximately equal");
	}};
	($x:expr, $y:expr, $z:expr, $r:expr) => {{
		let x = Rational::from(Into::<(u128, u128)>::into($x));
		let y = Rational::from(Into::<(u128, u128)>::into($y));
		let z = Rational::from(Into::<(u128, u128)>::into($z));
		let diff = if x >= y {
			x.clone() - y.clone()
		} else {
			y.clone() - x.clone()
		};
		assert!(
			diff <= z,
			"\n{}\n    left: {:?}\n   right: {:?}\n    diff: {:?}\nmax_diff: {:?}\n",
			$r,
			x,
			y,
			diff,
			z
		);
	}};
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

/// Return the entry of an asset pair in the accumulator.
fn get_accumulator_entry(src: Source, (a, b): (AssetId, AssetId)) -> Option<OracleEntry<BlockNumber>> {
	let acc = Accumulator::<Test>::get();
	acc.get(&(src, ordered_pair(a, b))).cloned()
}

fn get_oracle_entry(a: AssetId, b: AssetId, period: OraclePeriod) -> Option<OracleEntry<BlockNumber>> {
	Oracles::<Test>::get((SOURCE, ordered_pair(a, b), period)).map(|(e, _)| e)
}

#[test]
fn genesis_config_works() {
	ExtBuilder::default()
		.with_initial_data(vec![
			(
				SOURCE,
				(HDX, DOT),
				(1_000_000, 1).into(),
				Liquidity::new(2_000_000, 2_000_000_000),
			),
			(
				SOURCE,
				(HDX, ACA),
				(3_000_000, 1).into(),
				Liquidity::new(4_000_000, 4_000_000_000),
			),
		])
		.build()
		.execute_with(|| {
			for period in supported_periods() {
				assert_eq!(
					get_oracle_entry(HDX, DOT, period),
					Some(OracleEntry {
						price: Price::new(1_000_000, 1),
						volume: Volume::default(),
						liquidity: Liquidity::new(2_000_000, 2_000_000_000),
						updated_at: 0,
					})
				);

				assert_eq!(
					get_oracle_entry(HDX, ACA, period),
					Some(OracleEntry {
						price: Price::new(3_000_000, 1),
						volume: Volume::default(),
						liquidity: Liquidity::new(4_000_000, 4_000_000_000),
						updated_at: 0,
					})
				);
			}
		});
}

#[test]
fn on_trade_should_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), None);
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_2));
		let price_entry = ORACLE_ENTRY_2.with_added_volume_from(&ORACLE_ENTRY_1);
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)).unwrap(), price_entry);
	});
}

#[test]
fn on_trade_handler_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), None);
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
		));
		let expected = OracleEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::from_a_in_b_out(1_000, 500),
			liquidity: Liquidity::new(2_000, 1_000),
			updated_at: 5,
		};
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), Some(expected));
	});
}

#[test]
fn on_liquidity_changed_handler_should_work() {
	new_test_ext().execute_with(|| {
		let updated_at = 5;
		System::set_block_number(updated_at);
		let no_volume_entry = OracleEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::default(),
			liquidity: Liquidity::new(2_000, 1_000),
			updated_at,
		};
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), None);
		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			DOT,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
		));
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), Some(no_volume_entry));
	});
}

#[test]
fn price_should_be_determined_from_liquidity() {
	new_test_ext().execute_with(|| {
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), None);
		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			DOT,
			5,
			1,
			2_000_000,
			1_000_000,
			Price::new(2_000_000, 1_000_000),
		));
		let expected = Price::new(2_000_000, 1_000_000);
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)).unwrap().price, expected);

		assert_eq!(get_accumulator_entry(SOURCE, (DOT, ACA)), None);
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			DOT,
			ACA,
			1234,
			789,
			5_000_000,
			500,
			Price::new(5_000_000, 500),
		));
		let expected = Price::new(5_000_000, 500);
		assert_eq!(get_accumulator_entry(SOURCE, (DOT, ACA)).unwrap().price, expected);
	});
}

#[test]
fn on_liquidity_changed_should_allow_zero_values() {
	let updated_at = 5;
	let (liquidity_a, liquidity_b) = (2_000, 1_000);
	let amount = 1_000;

	new_test_ext().execute_with(|| {
		System::set_block_number(updated_at);
		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			DOT,
			Balance::zero(),
			amount,
			liquidity_a,
			liquidity_b,
			Price::new(liquidity_a, liquidity_b),
		));
		let only_liquidity_entry = OracleEntry {
			price: Price::new(liquidity_a, liquidity_b),
			volume: Volume::default(),
			liquidity: (liquidity_a, liquidity_b).into(),
			updated_at,
		};
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), Some(only_liquidity_entry));
	});

	new_test_ext().execute_with(|| {
		System::set_block_number(updated_at);
		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			DOT,
			amount,
			Balance::zero(),
			liquidity_a,
			liquidity_b,
			Price::new(liquidity_a, liquidity_b),
		));
		let only_liquidity_entry = OracleEntry {
			price: Price::new(liquidity_a, liquidity_b),
			volume: Volume::default(),
			liquidity: (liquidity_a, liquidity_b).into(),
			updated_at,
		};
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), Some(only_liquidity_entry));
	});

	new_test_ext().execute_with(|| {
		System::set_block_number(updated_at);
		assert_ok!(OnActivityHandler::<Test>::on_liquidity_changed(
			SOURCE,
			HDX,
			DOT,
			amount,
			amount,
			Balance::zero(),
			Balance::zero(),
			Price::zero(),
		));
		let only_price_entry = OracleEntry {
			price: Price::zero(),
			volume: Volume::default(),
			liquidity: (Balance::zero(), Balance::zero()).into(),
			updated_at,
		};
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), Some(only_price_entry));
	});
}

#[test]
fn on_trade_should_exclude_zero_values() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(SOURCE, HDX, DOT, 1_000, 1_000, Balance::zero(), 1_000, Price::zero())
				.map_err(|(_w, e)| e),
			Error::<Test>::OnTradeValueZero
		);

		assert_noop!(
			OnActivityHandler::<Test>::on_trade(SOURCE, HDX, DOT, 1_000, 1_000, 2_000, Balance::zero(), Price::zero())
				.map_err(|(_w, e)| e),
			Error::<Test>::OnTradeValueZero
		);
	});
}

#[test]
fn on_entry_should_error_on_accumulator_overflow() {
	new_test_ext().execute_with(|| {
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();
		// let's fill the accumulator
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
			));
		}
		// on_trade should fail once the accumulator is full
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(
				SOURCE,
				2 * max_entries,
				2 * max_entries + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
			)
			.map_err(|(_w, e)| e),
			Error::<Test>::TooManyUniqueEntries
		);
	});
}

#[test]
fn volume_normalization_should_factor_in_asset_order() {
	assert_ne!(
		determine_normalized_volume(HDX, DOT, 1_000, 500),
		determine_normalized_volume(DOT, HDX, 500, 1_000)
	);
}

#[test]
fn liquidity_normalization_should_factor_in_asset_order() {
	assert_ne!(
		determine_normalized_liquidity(HDX, DOT, 1_000, 500),
		determine_normalized_liquidity(DOT, HDX, 1_000, 500)
	);
}

#[test]
fn oracle_volume_should_factor_in_asset_order() {
	new_test_ext().execute_with(|| {
		assert_eq!(get_accumulator_entry(SOURCE, (HDX, DOT)), None);

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			2_000_000,
			1_000,
			2_000,
			1,
			Price::new(2_000, 1),
		));
		// we reverse the order of the arguments
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			DOT,
			HDX,
			1_000,
			2_000_000,
			1,
			2_000,
			Price::new(1, 2_000),
		));

		let price_entry = get_accumulator_entry(SOURCE, (HDX, DOT)).unwrap();
		let first_entry = OracleEntry {
			price: Price::new(2_000, 1),
			volume: Volume::from_a_in_b_out(2_000_000, 1_000),
			liquidity: (2_000, 1).into(),
			updated_at: 0,
		};
		let second_entry = OracleEntry {
			price: Price::new(2_000, 1),
			volume: Volume::from_a_out_b_in(2_000_000, 1_000),
			liquidity: (2_000, 1).into(),
			updated_at: 0,
		};

		let result = second_entry.with_added_volume_from(&first_entry);
		assert_eq!(price_entry, result);
	});
}

#[test]
fn update_data_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);

		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_2));
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, ACA), ORACLE_ENTRY_1));

		EmaOracle::on_finalize(5);
		System::set_block_number(6);
		EmaOracle::on_initialize(6);

		for period in supported_periods() {
			assert_eq!(
				get_oracle_entry(HDX, DOT, period),
				Some(ORACLE_ENTRY_2.with_added_volume_from(&ORACLE_ENTRY_1)),
			);
			assert_eq!(get_oracle_entry(HDX, ACA, period), Some(ORACLE_ENTRY_1),);
		}
	});
}

#[test]
fn update_data_should_use_old_last_block_oracle_to_update_to_parent() {
	new_test_ext().execute_with(|| {
		System::set_block_number(5);
		EmaOracle::on_initialize(5);
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), ORACLE_ENTRY_1));
		EmaOracle::on_finalize(5);

		System::set_block_number(6);
		EmaOracle::on_initialize(6);
		let second_entry = OracleEntry {
			liquidity: Liquidity::new(3_000, 1_500),
			updated_at: 6,
			..ORACLE_ENTRY_1
		};
		assert_ok!(EmaOracle::on_trade(
			SOURCE,
			ordered_pair(HDX, DOT),
			second_entry.clone()
		));
		EmaOracle::on_finalize(6);

		System::set_block_number(50);
		EmaOracle::on_initialize(50);
		let third_entry = OracleEntry {
			liquidity: Liquidity::new(10, 5),
			updated_at: 50,
			..ORACLE_ENTRY_1
		};
		assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), third_entry.clone()));
		EmaOracle::on_finalize(50);

		for period in supported_periods() {
			let second_at_50 = OracleEntry {
				updated_at: 49,
				volume: Volume::default(),
				..second_entry.clone()
			};
			let mut expected = ORACLE_ENTRY_1.clone();
			expected
				.update_to_new_by_integrating_incoming(period, &second_entry)
				.unwrap()
				.update_outdated_to_current(period, &second_at_50)
				.unwrap()
				.update_to_new_by_integrating_incoming(period, &third_entry)
				.unwrap();
			assert_eq!(
				get_oracle_entry(HDX, DOT, period).unwrap(),
				expected,
				"Oracle entry should be updated correctly for {:?}",
				period
			);
		}
	});
}

#[test]
fn calculate_new_by_integrating_incoming_only_updates_updated_at_on_stable_values() {
	let period = TenMinutes;
	let start_oracle = OracleEntry {
		price: Price::new(4, 1),
		volume: Volume::from_a_in_b_out(1, 4),
		liquidity: Liquidity::new(4, 1),
		updated_at: 5_u32,
	};
	let next_value = OracleEntry {
		updated_at: 6,
		..start_oracle.clone()
	};
	let next_oracle = start_oracle.calculate_new_by_integrating_incoming(period, &next_value);
	assert_eq!(next_oracle, Some(next_value));
}

#[test]
fn calculate_new_by_integrating_incoming_with_works() {
	let start_oracle = OracleEntry {
		price: Price::new(50, 1),
		volume: Volume::from_a_in_b_out(1, 50),
		liquidity: Liquidity::new(50, 1),
		updated_at: 5_u32,
	};

	let next_value = OracleEntry {
		price: Price::new(151, 1),
		volume: Volume::from_a_in_b_out(1, 151),
		liquidity: Liquidity::new(151, 1),
		updated_at: 6,
	};
	let next_oracle = start_oracle
		.calculate_new_by_integrating_incoming(TenMinutes, &next_value)
		.unwrap();
	// ten minutes corresponds to 100 blocks which corresponds to a smoothing factor of
	// `2 / 101 ≈ 1 / 50` which means that for an update from 50 to 151 we expect an update of
	// about 2
	let expected_oracle = OracleEntry {
		price: Price::new(52, 1),
		volume: Volume::from_a_in_b_out(1, 52),
		liquidity: Liquidity::new(52, 1),
		updated_at: 6,
	};
	let tolerance = Price::new(1, 1e10 as u128);
	assert_price_approx_eq!(next_oracle.price, expected_oracle.price, tolerance);
	assert_eq!(next_oracle.volume, expected_oracle.volume);
	assert_eq!(next_oracle.liquidity, expected_oracle.liquidity);
	assert_eq!(next_oracle.updated_at, expected_oracle.updated_at);
}

#[test]
fn calculate_new_by_integrating_incoming_last_block_period_returns_new_value() {
	let start_oracle = OracleEntry {
		price: Price::new(4, 1),
		volume: Volume::from_a_in_b_out(1_u128, 4_u128),
		liquidity: Liquidity::new(4_u128, 1_u128),
		updated_at: 5_u32,
	};

	let next_value = OracleEntry {
		price: Price::new(8, 1),
		volume: Volume::from_a_in_b_out(1_u128, 8_u128),
		liquidity: Liquidity::new(8_u128, 1_u128),
		updated_at: 6,
	};
	let next_oracle = start_oracle.calculate_new_by_integrating_incoming(LastBlock, &next_value);
	let expected_oracle = next_value;
	assert_eq!(next_oracle, Some(expected_oracle));
}

#[test]
fn calculate_current_from_outdated_should_incorporate_longer_time_deltas() {
	let period = TenMinutes;
	let start_oracle = OracleEntry {
		price: Price::new(4_000, 1),
		volume: Volume::from_a_in_b_out(1, 4_000),
		liquidity: Liquidity::new(4_000, 1),
		updated_at: 5_u32,
	};
	let next_value = OracleEntry {
		price: Price::new(8_000, 1),
		volume: Volume::from_a_in_b_out(1, 8_000),
		liquidity: Liquidity::new(8_000, 1),
		updated_at: 1_000,
	};
	let next_oracle = start_oracle
		.calculate_current_from_outdated(period, &next_value)
		.unwrap();
	assert_price_approx_eq!(
		next_oracle.price,
		next_value.price,
		(1, 10_000),
		"Oracle price deviates too much."
	);
}

#[test]
fn get_price_works() {
	ExtBuilder::default()
		.with_initial_data(vec![(
			SOURCE,
			(HDX, DOT),
			(1_000_000, 1).into(),
			Liquidity::new(2_000_000, 2),
		)])
		.build()
		.execute_with(|| {
			System::set_block_number(2);
			let expected = ((1_000_000, 1).into(), 1);
			assert_eq!(EmaOracle::get_price(HDX, DOT, LastBlock, SOURCE), Ok(expected));
			assert_eq!(EmaOracle::get_price(HDX, DOT, TenMinutes, SOURCE), Ok(expected));
			assert_eq!(EmaOracle::get_price(HDX, DOT, Day, SOURCE), Ok(expected));
			assert_eq!(EmaOracle::get_price(HDX, DOT, Week, SOURCE), Ok(expected));
		});
}

#[test]
fn trying_to_get_price_for_same_asset_should_error() {
	ExtBuilder::default()
		.with_initial_data(vec![(
			SOURCE,
			(HDX, DOT),
			(1_000_000, 1).into(),
			Liquidity::new(2_000_000, 2),
		)])
		.build()
		.execute_with(|| {
			System::set_block_number(2);
			assert_eq!(
				EmaOracle::get_price(HDX, HDX, LastBlock, SOURCE),
				Err(OracleError::SameAsset),
			);
		});
}

#[test]
fn get_entry_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
		));
		EmaOracle::on_finalize(1);
		System::set_block_number(100);
		let expected = AggregatedEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::default(), // volume for new blocks is zero by default
			liquidity: Liquidity::new(2_000, 1_000),
			oracle_age: 98,
		};
		assert_eq!(EmaOracle::get_entry(HDX, DOT, LastBlock, SOURCE), Ok(expected));

		let expected_ten_min = AggregatedEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::from_a_in_b_out(141, 70), // volume oracle gets updated towards zero
			liquidity: Liquidity::new(2_000, 1_000),
			oracle_age: 98,
		};
		assert_eq!(EmaOracle::get_entry(HDX, DOT, TenMinutes, SOURCE), Ok(expected_ten_min));

		let expected_day = AggregatedEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::from_a_in_b_out(986, 493),
			liquidity: Liquidity::new(2_000, 1_000),
			oracle_age: 98,
		};
		assert_eq!(EmaOracle::get_entry(HDX, DOT, Day, SOURCE), Ok(expected_day));

		let expected_week = AggregatedEntry {
			price: Price::new(2_000, 1_000),
			volume: Volume::from_a_in_b_out(998, 499),
			liquidity: Liquidity::new(2_000, 1_000),
			oracle_age: 98,
		};
		assert_eq!(EmaOracle::get_entry(HDX, DOT, Week, SOURCE), Ok(expected_week));
	});
}

#[test]
fn get_price_returns_updated_price() {
	ExtBuilder::default()
		.with_initial_data(vec![(
			SOURCE,
			(HDX, DOT),
			(1_000_000, 1).into(),
			Liquidity::new(2_000_000, 2),
		)])
		.build()
		.execute_with(|| {
			let on_trade_entry = OracleEntry {
				price: Price::new(500_000, 1),
				volume: Volume::default(),
				liquidity: Liquidity::new(2_000_000, 2),
				updated_at: 1,
			};
			System::set_block_number(1);
			assert_ok!(EmaOracle::on_trade(SOURCE, ordered_pair(HDX, DOT), on_trade_entry));
			EmaOracle::on_finalize(1);

			System::set_block_number(10_001);

			assert_eq!(
				EmaOracle::get_price(HDX, DOT, LastBlock, SOURCE).unwrap().1,
				10_000,
				"Oracle should be 10_000 blocks old."
			);
			assert_eq!(
				EmaOracle::get_price(HDX, DOT, Day, SOURCE).unwrap().1,
				10_000,
				"Oracle should be 10_000 blocks old."
			);

			let tolerance = (1, 1_000);
			assert_price_approx_eq!(
				EmaOracle::get_price(HDX, DOT, LastBlock, SOURCE).unwrap().0,
				Price::new(500_000, 1),
				tolerance,
				"LastBlock Oracle should have most recent value."
			);
			assert_price_approx_eq!(
				EmaOracle::get_price(HDX, DOT, TenMinutes, SOURCE).unwrap().0,
				Price::new(500_000, 1),
				tolerance,
				"TenMinutes Oracle should converge within 1000 blocks."
			);
			assert_price_approx_eq!(
				EmaOracle::get_price(HDX, DOT, Day, SOURCE).unwrap().0,
				Price::new(6_246_761_041_102_896_u128, 10_000_000_000),
				tolerance,
				"Day Oracle should converge somewhat."
			);
			assert_price_approx_eq!(
				EmaOracle::get_price(HDX, DOT, Week, SOURCE).unwrap().0,
				Price::new(9_100_156_788_246_781_u128, 10_000_000_000),
				tolerance,
				"Week Oracle should converge a little."
			);
		});
}

#[test]
fn ema_update_should_return_none_if_new_entry_is_older() {
	let mut entry = OracleEntry {
		updated_at: 10,
		..ORACLE_ENTRY_1
	};
	let original = entry.clone();
	// older than current
	let outdated_entry = OracleEntry {
		updated_at: 9,
		..ORACLE_ENTRY_2
	};
	assert_eq!(entry.calculate_current_from_outdated(TenMinutes, &outdated_entry), None);
	assert_eq!(entry.calculate_current_from_outdated(LastBlock, &outdated_entry), None);
	assert_eq!(
		entry.calculate_new_by_integrating_incoming(TenMinutes, &outdated_entry),
		None
	);
	assert_eq!(
		entry.calculate_new_by_integrating_incoming(LastBlock, &outdated_entry),
		None
	);
	// same updated_at as current
	let outdated_entry = OracleEntry {
		updated_at: 10,
		..ORACLE_ENTRY_2
	};
	assert_eq!(entry.calculate_current_from_outdated(TenMinutes, &outdated_entry), None);
	assert_eq!(entry.calculate_current_from_outdated(LastBlock, &outdated_entry), None);
	assert_eq!(
		entry.calculate_new_by_integrating_incoming(TenMinutes, &outdated_entry),
		None
	);
	assert_eq!(
		entry.calculate_new_by_integrating_incoming(LastBlock, &outdated_entry),
		None
	);

	assert_eq!(
		entry.update_to_new_by_integrating_incoming(TenMinutes, &outdated_entry),
		None
	);
	assert_eq!(entry, original);
	assert_eq!(entry.update_outdated_to_current(TenMinutes, &outdated_entry), None);
	assert_eq!(entry, original);
}

#[test]
fn check_period_smoothing_factors() {
	use hydra_dx_math::ema::smoothing_from_period;

	// We assume a 6 second block time.
	let secs_per_block = 6;
	let minutes = 60 / secs_per_block;
	let hours = 60 * minutes;
	let days = 24 * hours;

	let last_block = smoothing_from_period(1);
	println!("Last Block: {} (bits: {})", last_block, last_block.to_bits());
	assert_eq!(into_smoothing(LastBlock), last_block);

	let short = smoothing_from_period(9);
	println!("Short: {} (bits: {})", short, short.to_bits());
	assert_eq!(into_smoothing(Short), short);

	let ten_minutes = smoothing_from_period(10 * minutes);
	println!("Ten Minutes: {} (bits: {})", ten_minutes, ten_minutes.to_bits());
	assert_eq!(into_smoothing(TenMinutes), ten_minutes);

	let hour = smoothing_from_period(hours);
	println!("Hour: {} (bits: {})", hour, hour.to_bits());
	assert_eq!(into_smoothing(Hour), hour);

	let day = smoothing_from_period(days);
	println!("Day: {} (bits: {})", day, day.to_bits());
	assert_eq!(into_smoothing(Day), day);

	let week = smoothing_from_period(7 * days);
	println!("Week: {} (bits: {})", week, week.to_bits());
	assert_eq!(into_smoothing(Week), week);
}
