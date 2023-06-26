use crate::pallet::{PositionVotes, Positions};
use crate::types::{Balance, Conviction, Vote};
use crate::{Config, Error, Pallet};
use frame_support::dispatch::DispatchResult;
use pallet_democracy::traits::DemocracyHooks;
use pallet_democracy::{AccountVote, ReferendumIndex};

pub struct StakingDemocracy<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> DemocracyHooks<T::AccountId, Balance> for StakingDemocracy<T> {
	fn on_vote(who: &T::AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult {
		//TODO: handle unwraps
		let position_id = Pallet::<T>::get_user_position_id(who)?.unwrap();
		let position = Positions::<T>::get(&position_id).unwrap();

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
			amount: amount.max(position.stake), // use only max staked amount
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
}
