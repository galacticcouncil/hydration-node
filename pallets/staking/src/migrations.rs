use crate::pallet;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_core::Get;
use sp_runtime::traits::BlockNumberProvider;

use crate::{
	types::{Action, Conviction, Voting},
	Config, Pallet, Positions,
};
use frame_support::{storage_alias, traits::ConstU32, Blake2_128Concat};
use sp_runtime::FixedPointNumber;

// Gov v1 allowed 100 votes; OpenGov's current bound must not be used to decode these records.
#[storage_alias]
pub type PositionVotes<T: Config> =
	StorageMap<Pallet<T>, Blake2_128Concat, <T as Config>::PositionItemId, Voting<ConstU32<100>>>;

pub fn settle_legacy_votes<T: Config>(
	position_id: T::PositionItemId,
	voting: Voting<ConstU32<100>>,
	is_finished: impl Fn(u32) -> bool,
) {
	Positions::<T>::mutate_exists(position_id, |position| {
		let Some(position) = position else { return };
		let max_vote = Conviction::max_multiplier().saturating_mul_int(position.stake);
		for (index, vote) in voting.votes {
			if is_finished(index) {
				let points = Pallet::<T>::calculate_points_for_action(Action::DemocracyVote, vote, max_vote);
				position.action_points = position.action_points.saturating_add(points);
			}
		}
	});
}

// This migration sets SixSecBlocksSince which is used to correctly calculate the periods in staking
// after the migration to 6s block time
pub struct SetSixSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetSixSecBlocksSince<T> {
	fn on_runtime_upgrade() -> Weight {
		let current_block_height = T::BlockNumberProvider::current_block_number();

		crate::SixSecBlocksSince::<T>::mutate(|block_height| {
			if *block_height == u32::MAX.into() {
				*block_height = current_block_height
			}

			log::info!("SixSecBlocksSince set to: {current_block_height:?}");
		});

		T::DbWeight::get().reads_writes(1, 1)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::migrations::SetSixSecBlocksSince;
	use crate::tests::mock::{set_block_number, ExtBuilder, Staking, Test};
	use crate::SixSecBlocksSince;
	use frame_system::pallet_prelude::BlockNumberFor;

	#[test]
	fn set_six_blocks_since_executes_when_storage_not_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			SixSecBlocksSince::<Test>::put(u32::MAX as BlockNumberFor<Test>);

			// Act
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::six_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
		});
	}

	#[test]
	fn set_six_blocks_since_does_not_execute_when_storage_is_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			SixSecBlocksSince::<Test>::put(u32::MAX as BlockNumberFor<Test>);
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Act
			set_block_number(1000);
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::six_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
		});
	}
}

#[cfg(test)]
mod legacy_democracy_tests {
	use super::*;
	use crate::{
		tests::mock::{ExtBuilder, Test},
		types::{Position, Vote},
	};
	use sp_runtime::{traits::Zero, FixedU128};

	#[test]
	fn settlement_should_award_only_finished_votes_when_legacy_position_is_retired() {
		ExtBuilder::default().build().execute_with(|| {
			let position_id = 1;
			Positions::<Test>::insert(position_id, Position::new(1_000, FixedU128::zero(), 1));
			let vote = Vote {
				amount: 1_000,
				conviction: Conviction::Locked6x,
			};
			let voting = Voting::<ConstU32<100>> {
				votes: vec![(1, vote), (2, vote)].try_into().unwrap(),
			};
			settle_legacy_votes::<Test>(position_id, voting, |index| index == 1);
			assert_eq!(Positions::<Test>::get(position_id).unwrap().get_action_points(), 100);
		});
	}

	#[test]
	fn legacy_votes_should_decode_original_bound_when_opengov_bound_is_lower() {
		use codec::{Decode, Encode};
		let vote = Vote {
			amount: 1_000,
			conviction: Conviction::Locked6x,
		};
		let voting = Voting::<ConstU32<100>> {
			votes: (0..100).map(|i| (i, vote)).collect::<Vec<_>>().try_into().unwrap(),
		};
		assert_eq!(
			Voting::<ConstU32<100>>::decode(&mut &voting.encode()[..])
				.unwrap()
				.votes
				.len(),
			100
		);
	}
}
