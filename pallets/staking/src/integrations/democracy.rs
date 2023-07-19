use crate::pallet::{PositionVotes, Positions};
use crate::traits::DemocracyReferendum;
use crate::types::{Balance, Conviction, Vote};
use crate::{Config, Error, Pallet};
use frame_support::dispatch::DispatchResult;
use frame_system::Origin;
use orml_traits::MultiCurrencyExtended;
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
		let position = if let Some(position) = Positions::<T>::get(position_id) {
			position
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

		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex) -> DispatchResult {
		let position_id = if let Some(position_id) = Pallet::<T>::get_user_position_id(who)? {
			position_id
		} else {
			return Ok(());
		};

		PositionVotes::<T>::try_mutate_exists(position_id, |value| -> DispatchResult {
			let voting = value.as_mut().ok_or(Error::<T>::MaxVotesReached)?;
			voting.votes.retain(|(idx, _)| *idx != ref_index);
			Ok(())
		})?;

		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(who: &T::AccountId) {
		T::Currency::update_balance(
			T::HdxAssetId::get(),
			&Pallet::<T>::pot_account_id(),
			10_000_000_000_000i128,
		)
		.unwrap();
		Pallet::<T>::initialize_staking(Origin::<T>::Root.into()).unwrap();
		T::Currency::update_balance(T::HdxAssetId::get(), who, 1000_000_000_000_000i128).unwrap();
		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1000_000_000_000_000u128).unwrap();
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(who: &T::AccountId) {
		T::Currency::update_balance(
			T::HdxAssetId::get(),
			&Pallet::<T>::pot_account_id(),
			10_000_000_000_000i128,
		)
		.unwrap();
		Pallet::<T>::initialize_staking(Origin::<T>::Root.into()).unwrap();
		T::Currency::update_balance(T::HdxAssetId::get(), who, 1000_000_000_000_000i128).unwrap();
		Pallet::<T>::stake(Origin::<T>::Signed(who.clone()).into(), 1000_000_000_000_000u128).unwrap();
	}
}

pub struct ReferendumStatus<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_democracy::Config> DemocracyReferendum for ReferendumStatus<T> {
	fn is_referendum_finished(index: ReferendumIndex) -> bool {
		let maybe_info = pallet_democracy::Pallet::<T>::referendum_info(index);
		matches!(maybe_info, Some(ReferendumInfo::Finished { .. }))
	}
}
