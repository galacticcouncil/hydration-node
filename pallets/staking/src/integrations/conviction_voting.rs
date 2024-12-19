use crate::pallet::{Positions, Votes, VotesRewarded};
use crate::traits::{GetReferendumState, VestingDetails};
use crate::types::{Action, Balance, Conviction, ReferendumIndex, Vote};
use crate::{Config, Error, Pallet};
use frame_support::defensive;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::{PollStatus, Polling};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_conviction_voting::traits::VotingHooks;
use pallet_conviction_voting::AccountVote;
use sp_core::Get;
use sp_runtime::FixedPointNumber;

pub struct StakingConvictionVoting<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> VotingHooks<T::AccountId, ReferendumIndex, Balance> for StakingConvictionVoting<T>
where
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
{
	fn on_vote(who: &T::AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult {
		let position_id = if let Some(position_id) = Pallet::<T>::get_user_position_id(who)? {
			position_id
		} else {
			return Ok(());
		};

		Positions::<T>::try_mutate(position_id, |maybe_position| {
			let position = match maybe_position.as_mut() {
				Some(position) => position,
				None => {
					let e = crate::Error::<T>::InconsistentState(crate::InconsistentStateError::PositionNotFound);
					defensive!(e);

					//NOTE: This is intentional, user can't recover from this state and we don't want
					//to block voting.
					return Ok(());
				}
			};

			Pallet::<T>::process_votes(who, position_id, position)?;

			let amount = vote.balance();
			let conviction = if let AccountVote::Standard { vote, .. } = vote {
				match vote.conviction {
					pallet_conviction_voting::Conviction::None => Conviction::None,
					pallet_conviction_voting::Conviction::Locked1x => Conviction::Locked1x,
					pallet_conviction_voting::Conviction::Locked2x => Conviction::Locked2x,
					pallet_conviction_voting::Conviction::Locked3x => Conviction::Locked3x,
					pallet_conviction_voting::Conviction::Locked4x => Conviction::Locked4x,
					pallet_conviction_voting::Conviction::Locked5x => Conviction::Locked5x,
					pallet_conviction_voting::Conviction::Locked6x => Conviction::Locked6x,
				}
			} else {
				Conviction::default()
			};

			// We are capping vote by min(position stake, user's balance - vested amount - locked
			// rewards).
			// Sub of vested and lockek rewards is necessary because locks overlay so users may end
			// up in the situation where portion of the staking lock is also vested or locked
			// rewads and we don't want to assign points for it.
			let max_vote = T::Currency::free_balance(T::NativeAssetId::get(), who)
				.saturating_sub(T::Vesting::locked(who.clone()))
				.saturating_sub(position.accumulated_locked_rewards)
				.min(position.stake);
			let staking_vote = Vote {
				amount: amount.min(position.stake).min(max_vote),
				conviction,
			};

			Votes::<T>::try_mutate(position_id, |voting| -> DispatchResult {
				match voting.votes.binary_search_by_key(&ref_index, |value| value.0) {
					Ok(idx) => {
						let _ = sp_std::mem::replace(&mut voting.votes[idx], (ref_index, staking_vote));
					}
					Err(idx) => {
						voting
							.votes
							.try_insert(idx, (ref_index, staking_vote))
							.map_err(|_| Error::<T>::MaxVotesReached)?;
					}
				}
				Ok(())
			})
		})
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex, ongoing: Option<bool>) {
		let Some(maybe_position_id) = Pallet::<T>::get_user_position_id(who).ok() else {
			return;
		};

		let Some(position_id) = maybe_position_id else {
			return;
		};

		let entry = VotesRewarded::<T>::take(who, ref_index);
		if entry.is_some() {
			// this vote was already processed, just remove it
			return;
		}

		let _ = Positions::<T>::try_mutate(position_id, |maybe_position| -> DispatchResult {
			if let Some(position) = maybe_position.as_mut() {
				let max_position_vote = Conviction::max_multiplier().saturating_mul_int(position.stake);

				if let Some(vote_idx) = Votes::<T>::get(position_id)
					.votes
					.iter()
					.position(|(idx, _)| *idx == ref_index)
				{
					let (ref_idx, vote) = Votes::<T>::get(position_id).votes[vote_idx];
					debug_assert_eq!(ref_idx, ref_index, "Referendum index mismatch");
					let points =
						Pallet::<T>::calculate_points_for_action(Action::DemocracyVote, vote, max_position_vote);
					// Add points only if referendum is finished
					if let Some(is_ongoing) = ongoing {
						if !is_ongoing {
							position.action_points = position.action_points.saturating_add(points);
						}
					}
					Votes::<T>::mutate(position_id, |voting| {
						voting.votes.remove(vote_idx);
					});
				}
			}
			Ok(())
		});
	}

	fn balance_locked_on_unsuccessful_vote(who: &T::AccountId, ref_index: ReferendumIndex) -> Option<Balance> {
		let position_id = Pallet::<T>::get_user_position_id(who).ok()??;

		if let Some(vote) = VotesRewarded::<T>::get(who, ref_index) {
			return Some(vote.amount);
		}

		let vote_idx = Votes::<T>::get(position_id)
			.votes
			.iter()
			.position(|(idx, _)| *idx == ref_index)?;

		let (ref_idx, vote) = Votes::<T>::get(position_id).votes[vote_idx];
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

		if T::Currency::free_balance(T::NativeAssetId::get(), who) <= 1_000_000_000_000_000 {
			T::Currency::update_balance(T::NativeAssetId::get(), who, 1_000_000_000_000_000i128).unwrap();
		}

		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1_000_000_000_000_000u128).unwrap();

		for i in 0..<T as crate::pallet::Config>::MaxLocks::get() - 5 {
			let id: LockIdentifier = scale_info::prelude::format!("{:a>8}", i.to_string())
				.as_bytes()
				.try_into()
				.unwrap();

			T::Currency::set_lock(id, T::NativeAssetId::get(), who, 10_000_000_000_000_u128).unwrap();
		}
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
		if T::Currency::free_balance(T::NativeAssetId::get(), who) <= 1_000_000_000_000_000 {
			T::Currency::update_balance(T::NativeAssetId::get(), who, 1_000_000_000_000_000i128).unwrap();
		}
		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1_000_000_000_000_000u128).unwrap();
	}
}

pub struct ReferendumStatus<P, T>(sp_std::marker::PhantomData<(P, T)>);

impl<T, P: Polling<T>> GetReferendumState<ReferendumIndex> for ReferendumStatus<P, T>
where
	<P as Polling<T>>::Index: From<ReferendumIndex>,
{
	fn is_referendum_finished(_index: ReferendumIndex) -> bool {
		let r = <P as Polling<T>>::try_access_poll::<bool>(_index.into(), |status| {
			let r = match status {
				PollStatus::Completed(_, _) => true,
				PollStatus::Ongoing(_, _) => false,
				PollStatus::None => false,
			};
			Ok(r)
		});
		debug_assert!(r.is_ok(), "Failed to access poll");
		// If we failed to access poll, we assume that referendum is not finished - this should never be the case
		// Note: we cant return true, because it would reward points.
		r.unwrap_or(false)
	}
}
