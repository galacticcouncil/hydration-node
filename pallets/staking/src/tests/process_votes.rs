use crate::types::{Conviction, Vote};

use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;

//NOTE: Referendums with even indexes are finished.

#[test]
fn process_votes_should_work_when_referendum_is_finished() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.with_votings(vec![(
			1,
			vec![(
				2_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::None,
				},
			)],
		)])
		.build()
		.execute_with(|| {
			let position_id = 1;
			let position_before = Staking::positions(position_id).unwrap();
			let mut position = Staking::positions(position_id).unwrap();

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 10_000_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);
		});
}

#[test]
fn process_votes_should_do_nothing_when_referendum_is_not_finished() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.with_votings(vec![(
			1,
			vec![(
				1_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::None,
				},
			)],
		)])
		.build()
		.execute_with(|| {
			let position_id = 1;
			let position_before = Staking::positions(position_id).unwrap();
			let mut position = Staking::positions(position_id).unwrap();

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(position_before, position);
			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 1);
		});
}

#[test]
fn process_votes_should_work_when_referendum_is_finished_with_conviction() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.with_votings(vec![(
			1,
			vec![(
				2_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::Locked4x,
				},
			)],
		)])
		.build()
		.execute_with(|| {
			let position_id = 1;
			let position_before = Staking::positions(position_id).unwrap();
			let mut position = Staking::positions(position_id).unwrap();

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 50_000_u128,
					..position_before
				},
				position
			);
			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);
		});
}

#[test]
fn process_votes_should_work_when_multiple_votes_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.with_votings(vec![(
			1,
			vec![
				(
					1_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked4x,
					},
				),
				(
					2_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked2x,
					},
				),
				(
					3_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::None,
					},
				),
				(
					4_u32,
					Vote {
						amount: 230_000 * ONE,
						conviction: Conviction::Locked1x,
					},
				),
				(
					8_u32,
					Vote {
						amount: 230_000 * ONE,
						conviction: Conviction::Locked1x,
					},
				),
				(
					6_u32,
					Vote {
						amount: 2 * ONE,
						conviction: Conviction::Locked3x,
					},
				),
			],
		)])
		.build()
		.execute_with(|| {
			let position_id = 1;
			let position_before = Staking::positions(position_id).unwrap();
			let mut position = Staking::positions(position_id).unwrap();

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 950_008_u128,
					..position_before
				},
				position
			);
			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 2);
		});
}

#[test]
fn process_votes_should_do_nothing_when_referendum_doesnt_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			let position_id = 1;
			let position_before = Staking::positions(position_id).unwrap();
			let mut position = Staking::positions(position_id).unwrap();

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(position_before, position);
		});
}
