use crate::pallet::{PositionVotes, Positions};
use crate::traits::{DemocracyReferendum, VestingDetails};
use crate::types::{Balance, Conviction, Vote};
use crate::{Config, Error, Pallet};
use frame_support::defensive;
use frame_support::dispatch::DispatchResult;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_democracy::traits::DemocracyHooks;
use pallet_democracy::{AccountVote, ReferendumIndex, ReferendumInfo};
use sp_core::Get;

pub struct StakingDemocracy<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> DemocracyHooks<T::AccountId, Balance> for StakingDemocracy<T>
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

			Pallet::<T>::process_votes(position_id, position)?;

			let amount = vote.balance();
			let conviction = if let AccountVote::Standard { vote, .. } = vote {
				match vote.conviction {
					pallet_democracy::Conviction::None => Conviction::None,
					pallet_democracy::Conviction::Locked1x => Conviction::Locked1x,
					pallet_democracy::Conviction::Locked2x => Conviction::Locked2x,
					pallet_democracy::Conviction::Locked3x => Conviction::Locked3x,
					pallet_democracy::Conviction::Locked4x => Conviction::Locked4x,
					pallet_democracy::Conviction::Locked5x => Conviction::Locked5x,
					pallet_democracy::Conviction::Locked6x => Conviction::Locked6x,
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

			PositionVotes::<T>::try_mutate(position_id, |voting| -> DispatchResult {
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

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex, should_lock: bool) -> DispatchResult {
		let position_id = if let Some(position_id) = Pallet::<T>::get_user_position_id(who)? {
			position_id
		} else {
			return Ok(());
		};

		// This handles a case when user removes vote on finished referendum and the vote was in opposition to the referendum result
		// If user has a staking position, we keep the amount locked
		if should_lock {
			return Err(Error::<T>::RemoveVoteNotAllowed.into());
		}

		let mut position = Positions::<T>::get(position_id).unwrap();
		Pallet::<T>::process_votes(position_id, &mut position).unwrap();
		Positions::<T>::insert(position_id, position);

		PositionVotes::<T>::try_mutate_exists(position_id, |value| -> DispatchResult {
			let voting = match value.as_mut() {
				Some(voting) => voting,
				None => {
					let e = crate::Error::<T>::InconsistentState(crate::InconsistentStateError::PositionNotFound);
					defensive!(e);

					//NOTE: This is intentional, user can't recover from this state and we don't want
					//to block voting.
					return Ok(());
				}
			};

			voting.votes.retain(|(idx, _)| *idx != ref_index);
			Ok(())
		})?;

		Ok(())
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

pub struct ReferendumStatus<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_democracy::Config> DemocracyReferendum for ReferendumStatus<T> {
	fn is_referendum_finished(index: ReferendumIndex) -> bool {
		let maybe_info = pallet_democracy::Pallet::<T>::referendum_info(index);
		matches!(maybe_info, Some(ReferendumInfo::Finished { .. }))
	}
}
