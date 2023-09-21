use crate::{
	integrations::democracy::StakingDemocracy,
	types::{Conviction, Vote},
};

use super::*;

use frame_system::RawOrigin;
use mock::Staking;
use pallet_democracy::{traits::DemocracyHooks, AccountVote};
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
					amount: 100_000 * ONE,
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
					action_points: 1_u128,
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
					action_points: 5_u128,
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
					action_points: 64_u128,
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

#[test]
fn process_votes_should_work_when_on_vote_is_called() {
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

			//Act
			assert_ok!(StakingDemocracy::<Test>::on_vote(
				&BOB,
				7,
				AccountVote::Standard {
					balance: 1_000 * ONE,
					vote: pallet_democracy::Vote {
						aye: true,
						conviction: pallet_democracy::Conviction::None
					}
				}
			));

			//Assert
			assert_eq!(
				Position {
					action_points: 64_u128,
					..position_before
				},
				Staking::positions(position_id).unwrap()
			);
			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 3);
		});
}

#[test]
fn initialize_staking_should_work_when_pot_has_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), pot, HDX, 1_000 * ONE, 0));

			//Act
			assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));

			//Assert
			assert!(has_event(
				Event::<Test>::StakingInitialized {
					non_dustable_balance: 1_000 * ONE
				}
				.into()
			));
		});
}

#[test]
fn initialize_staking_should_not_work_when_pot_has_no_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Act & assert
			assert_noop!(
				Staking::initialize_staking(RawOrigin::Root.into()),
				Error::<Test>::MissingPotBalance
			);
		});
}

#[test]
fn initialize_staking_should_not_work_when_staking_is_already_initialized() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), pot, HDX, 1_000 * ONE, 0));
			assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));

			//Act & assert
			assert_noop!(
				Staking::initialize_staking(RawOrigin::Root.into()),
				Error::<Test>::AlreadyInitialized
			);
		});
}

#[test]
fn update_rewards_should_emit_event_when_rps_change() {
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
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);

			let mut staking = Staking::staking();

			//Act
			assert_ok!(Staking::update_rewards(&mut staking));

			//Assert
			assert_last_event!(Event::<Test>::AccumulatedRpsUpdated {
				accumulated_rps: staking.accumulated_reward_per_stake,
				total_stake: staking.total_stake
			}
			.into());
		});
}

#[test]
fn update_rewards_should_not_emit_event_when_pending_rewards_are_zero() {
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
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(0);

			let mut staking = Staking::staking();

			//Act
			assert_ok!(Staking::update_rewards(&mut staking));

			//Assert
			assert!(!has_event(
				Event::<Test>::AccumulatedRpsUpdated {
					accumulated_rps: staking.accumulated_reward_per_stake,
					total_stake: staking.total_stake
				}
				.into()
			));
		});
}

#[test]
fn process_votes_should_calculate_action_points_corectly_when_referendum_is_finished() {
	//NOTE: this test checks if action points are calculated correctly based on how much of stake
	//was used in votings.
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
					amount: 100_000 * ONE,
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
					action_points: 1_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Vote with max stake + max conviction
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: 120_000 * ONE,
					conviction: Conviction::Locked6x,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 100_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Too small vote to get any action points
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: ONE,
					conviction: Conviction::None,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 0_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Vote max stake + half convition
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: 120_000 * ONE,
					conviction: Conviction::Locked3x,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 50_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Vote with half stake + max convition
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: 60_000 * ONE,
					conviction: Conviction::Locked6x,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 50_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Vote with random stake + random conviction
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: 10_000 * ONE,
					conviction: Conviction::Locked2x,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 2_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);

			//Vote with max stake + conviction.none
			//NOTE: reset previous test
			position.action_points = 0;

			let votes = vec![(
				2_u32,
				Vote {
					amount: 120_000 * ONE,
					conviction: Conviction::None,
				},
			)];
			let v = Voting::<MaxVotes> {
				votes: votes.try_into().unwrap(),
			};

			PositionVotes::<Test>::insert(position_id, v);

			//Act
			assert_ok!(Staking::process_votes(position_id, &mut position));

			//Assert
			assert_eq!(
				Position {
					action_points: 1_u128,
					..position_before
				},
				position
			);

			assert_eq!(PositionVotes::<Test>::get(position_id).votes.len(), 0);
		});
}

#[test]
fn democracy_hook_vote_cap_should_work() {
	//Locks OVERLAY so 1000 tokens lock and 100 tokens lock results in 1000 tokens locked in total.
	use pallet_democracy::{Conviction as Dconviction, Vote as Dvote};
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
			(VESTED_100K, HDX, 180_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
			(VESTED_100K, 80_000 * ONE, 1_465_000, 10_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			let vested_position_id = 4;
			let ref_idx: u32 = 1;
			let v = AccountVote::Standard {
				vote: Dvote {
					aye: true,
					conviction: Dconviction::Locked6x,
				},
				balance: 100_000 * ONE,
			};

			assert_eq!(Staking::position_votes(vested_position_id).votes.len(), 0);

			//Act - happy path, user have enough token for staking and vesting.
			assert_ok!(StakingDemocracy::<Test>::on_vote(&VESTED_100K, ref_idx, v));

			//Assert
			let staking_votes = Staking::position_votes(vested_position_id).votes;

			assert_eq!(staking_votes.len(), 1);
			assert_eq!(staking_votes[0].1, Vote::new(80_000 * ONE, Conviction::Locked6x));

			//Assert 2 - 1 token is missing to fully satisfy both locks
			PositionVotes::<Test>::remove(vested_position_id);
			Tokens::transfer(RuntimeOrigin::signed(VESTED_100K), ALICE, HDX, 1).unwrap();

			//Act
			assert_ok!(StakingDemocracy::<Test>::on_vote(&VESTED_100K, ref_idx, v));

			//Assert
			let staking_votes = Staking::position_votes(vested_position_id).votes;

			assert_eq!(staking_votes.len(), 1);
			assert_eq!(staking_votes[0].1, Vote::new(80_000 * ONE - 1, Conviction::Locked6x));

			//Assert 3 - only vesting lock is satisfied
			PositionVotes::<Test>::remove(vested_position_id);
			Tokens::transfer(RuntimeOrigin::signed(VESTED_100K), ALICE, HDX, 80_000 * ONE - 1).unwrap();

			assert_eq!(Tokens::free_balance(HDX, &VESTED_100K), 100_000 * ONE);

			//Act 3
			assert_ok!(StakingDemocracy::<Test>::on_vote(&VESTED_100K, ref_idx, v));

			//Assert
			let staking_votes = Staking::position_votes(vested_position_id).votes;

			assert_eq!(staking_votes.len(), 1);
			assert_eq!(staking_votes[0].1, Vote::new(0, Conviction::Locked6x));

			//Assert 4 - portion of the lock are locked rewards
			PositionVotes::<Test>::remove(vested_position_id);
			Tokens::transfer(RuntimeOrigin::signed(ALICE), VESTED_100K, HDX, 20_000 * ONE).unwrap();

			let p = Staking::positions(vested_position_id).unwrap();
			Positions::<Test>::insert(
				vested_position_id,
				Position {
					accumulated_locked_rewards: 10_000 * ONE,
					..p
				},
			);

			assert_eq!(Tokens::free_balance(HDX, &VESTED_100K), 120_000 * ONE);
			let v = AccountVote::Standard {
				vote: Dvote {
					aye: true,
					conviction: Dconviction::Locked6x,
				},
				balance: 120_000 * ONE,
			};

			//Act 4
			assert_ok!(StakingDemocracy::<Test>::on_vote(&VESTED_100K, ref_idx, v));

			//Assert
			let staking_votes = Staking::position_votes(vested_position_id).votes;

			assert_eq!(staking_votes.len(), 1);
			assert_eq!(staking_votes[0].1, Vote::new(10_000 * ONE, Conviction::Locked6x));

			//Assert 5 -  sum of vested and locked rewards is bigger than account's balance
			PositionVotes::<Test>::remove(vested_position_id);
			Tokens::transfer(RuntimeOrigin::signed(VESTED_100K), ALICE, HDX, 20_000 * ONE).unwrap();

			let p = Staking::positions(vested_position_id).unwrap();
			Positions::<Test>::insert(
				vested_position_id,
				Position {
					accumulated_locked_rewards: 10_000 * ONE,
					..p
				},
			);

			assert_eq!(Tokens::free_balance(HDX, &VESTED_100K), 100_000 * ONE);
			let v = AccountVote::Standard {
				vote: Dvote {
					aye: true,
					conviction: Dconviction::Locked6x,
				},
				balance: 100_000 * ONE,
			};

			//Act 5
			assert_ok!(StakingDemocracy::<Test>::on_vote(&VESTED_100K, ref_idx, v));

			//Assert
			let staking_votes = Staking::position_votes(vested_position_id).votes;

			assert_eq!(staking_votes.len(), 1);
			assert_eq!(staking_votes[0].1, Vote::new(0, Conviction::Locked6x));
		});
}
