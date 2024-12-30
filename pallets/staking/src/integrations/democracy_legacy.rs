use crate::pallet::{PositionVotes, Positions, ProcessedVotes};
use crate::types::{Action, Balance, Conviction, Vote};
use crate::{Config, Pallet};
use frame_support::dispatch::DispatchResult;
use orml_traits::MultiCurrencyExtended;
use pallet_democracy::traits::DemocracyHooks;
use pallet_democracy::{AccountVote, ReferendumIndex};
use sp_runtime::FixedPointNumber;
use sp_core::Get;

pub struct LegacyStakingDemocracy<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> DemocracyHooks<T::AccountId, Balance> for LegacyStakingDemocracy<T>
where
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
{
	fn on_vote(_who: &T::AccountId, _ref_index: ReferendumIndex, _vote: AccountVote<Balance>) -> DispatchResult {
		// Do nothing.
		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex, is_finished: Option<bool>) {
		let Some(maybe_position_id) = Pallet::<T>::get_user_position_id(who).ok() else {
			return;
		};

		let Some(position_id) = maybe_position_id else {
			return;
		};

		let entry = ProcessedVotes::<T>::take(who, ref_index);
		if entry.is_some() {
			// this vote was already processed, just remove it
			return;
		}

		let _ = Positions::<T>::try_mutate(position_id, |maybe_position| -> DispatchResult {
			if let Some(position) = maybe_position.as_mut() {
				let max_position_vote = Conviction::max_multiplier().saturating_mul_int(position.stake);

				if let Some(vote_idx) = PositionVotes::<T>::get(position_id)
					.votes
					.iter()
					.position(|(idx, _)| *idx == ref_index)
				{
					let (ref_idx, vote) = PositionVotes::<T>::get(position_id).votes[vote_idx];
					debug_assert_eq!(ref_idx, ref_index, "Referendum index mismatch");
					let points =
						Pallet::<T>::calculate_points_for_action(Action::DemocracyVote, vote, max_position_vote);
					// Add points only if referendum is finished
					if let Some(is_finished) = is_finished {
						if is_finished {
							position.action_points = position.action_points.saturating_add(points);
						}
					}
					PositionVotes::<T>::mutate(position_id, |voting| {
						voting.votes.remove(vote_idx);
					});
				}
			}
			Ok(())
		});
	}

	fn remove_vote_locks_if_needed(who: &T::AccountId, ref_index: ReferendumIndex) -> Option<Balance> {
		let position_id = Pallet::<T>::get_user_position_id(who).ok()??;

		if let Some(vote) = ProcessedVotes::<T>::get(who, ref_index) {
			return Some(vote.amount);
		}

		let vote_idx = PositionVotes::<T>::get(position_id)
			.votes
			.iter()
			.position(|(idx, _)| *idx == ref_index)?;

		let (ref_idx, vote) = PositionVotes::<T>::get(position_id).votes[vote_idx];
		debug_assert_eq!(ref_idx, ref_index, "Referendum index mismatch");
		Some(vote.amount)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(who: &T::AccountId) {
		use crate::LockIdentifier;
		#[cfg(not(feature = "std"))]
		use codec::alloc::string::ToString;
		use frame_system::Origin;
		use orml_traits::MultiLockableCurrency;

		T::Currency::update_balance(
			T::NativeAssetId::get(),
			&Pallet::<T>::pot_account_id(),
			10_000_000_000_000i128,
		)
		.unwrap();
		Pallet::<T>::initialize_staking(Origin::<T>::Root.into()).unwrap();
		T::Currency::update_balance(T::NativeAssetId::get(), who, 1_000_000_000_000_000i128).unwrap();
		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1_000_000_000_000_000u128).unwrap();

		let position_id = Pallet::<T>::get_user_position_id(&who.clone()).unwrap().unwrap();

		let mut votes = sp_std::vec::Vec::<(u32, Vote)>::new();
		for i in 0..<T as crate::pallet::Config>::MaxVotes::get() {
			votes.push((
				i,
				Vote {
					amount: 20_000_000_000_000_000,
					conviction: Conviction::Locked1x,
				},
			));
		}

		for i in 0..<T as crate::pallet::Config>::MaxLocks::get() - 5 {
			let id: LockIdentifier = scale_info::prelude::format!("{:a>8}", i.to_string())
				.as_bytes()
				.try_into()
				.unwrap();

			T::Currency::set_lock(id, T::NativeAssetId::get(), who, 10_000_000_000_000_u128).unwrap();
		}

		let voting = crate::types::Voting::<T::MaxVotes> {
			votes: votes.try_into().unwrap(),
		};

		crate::PositionVotes::<T>::insert(position_id, voting);
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(who: &T::AccountId) {
		use frame_system::Origin;

		T::Currency::update_balance(
			T::NativeAssetId::get(),
			&Pallet::<T>::pot_account_id(),
			10_000_000_000_000i128,
		)
		.unwrap();
		Pallet::<T>::initialize_staking(Origin::<T>::Root.into()).unwrap();
		T::Currency::update_balance(T::NativeAssetId::get(), who, 1_000_000_000_000_000i128).unwrap();
		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1_000_000_000_000_000u128).unwrap();
	}
}
