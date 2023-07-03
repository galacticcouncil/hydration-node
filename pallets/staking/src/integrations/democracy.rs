use crate::pallet::{PositionVotes, Positions};
use crate::traits::DemocracyReferendum;
use crate::types::{Balance, Conviction, Vote};
use crate::{Config, Error, Pallet};
use frame_support::dispatch::DispatchResult;
use pallet_democracy::traits::DemocracyHooks;
use pallet_democracy::{AccountVote, ReferendumIndex, ReferendumInfo};

pub struct StakingDemocracy<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> DemocracyHooks<T::AccountId, Balance> for StakingDemocracy<T> {
	fn on_vote(who: &T::AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult {
		let maybe_position_id = Pallet::<T>::get_user_position_id(who)?;
		let position_id = if maybe_position_id.is_some() {
			maybe_position_id.unwrap()
		} else {
			return Ok(());
		};
		let position = Positions::<T>::get(&position_id);
		let position = if position.is_some() {
			position.unwrap()
		} else {
			return Ok(());
		};

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

		let staking_vote = Vote {
			amount: amount.min(position.stake), // use only max staked amount
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
		})?;
		//TODO: event?
		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex) -> DispatchResult {
		let maybe_position_id = Pallet::<T>::get_user_position_id(who)?;
		let position_id = if maybe_position_id.is_some() {
			maybe_position_id.unwrap()
		} else {
			return Ok(());
		};

		PositionVotes::<T>::try_mutate_exists(position_id, |value| -> DispatchResult {
			let voting = value.as_mut().ok_or(Error::<T>::MaxVotesReached)?;
			voting.votes.retain(|(idx, _)| *idx != ref_index);
			Ok(())
		})?;
		//TODO: event?
		Ok(())
	}
}

pub struct ReferendumStatus<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_democracy::Config> DemocracyReferendum for ReferendumStatus<T> {
	fn is_referendum_finished(index: ReferendumIndex) -> bool {
		let maybe_info = pallet_democracy::Pallet::<T>::referendum_info(index);
		match maybe_info {
			Some(info) => match info {
				ReferendumInfo::Finished { .. } => true,
				_ => false,
			},
			_ => false,
		}
	}
}
