// Copyright (C) 2020-2026 Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Retires Gov v1 without dropping deposits, voter locks, or earned staking points.
//! Pending proposals and ongoing referenda are retired. Scheduler owns approved enactments and
//! their preimage requests; they remain scheduled.

use crate::{AccountId, Balance, Balances, Runtime};
use codec::{Decode, Encode};
use frame_support::{
	storage::{storage_prefix, with_transaction},
	traits::{ConstU32, LockableCurrency, OnRuntimeUpgrade, ReservableCurrency},
	weights::Weight,
	BoundedVec,
};
use sp_runtime::{DispatchError, TransactionOutcome};
use sp_std::vec::Vec;

pub struct RemoveDemocracy;

impl RemoveDemocracy {
	fn prefix(phase: u8) -> Vec<u8> {
		match phase {
			0 => storage_prefix(b"Staking", b"PositionVotes").to_vec(),
			1 => storage_prefix(b"Staking", b"ProcessedVotes").to_vec(),
			2 => storage_prefix(b"Democracy", b"DepositOf").to_vec(),
			3 => storage_prefix(b"Democracy", b"VotingOf").to_vec(),
			_ => sp_io::hashing::twox_128(b"Democracy").to_vec(),
		}
	}

	fn is_finished(index: u32) -> bool {
		let mut key = storage_prefix(b"Democracy", b"ReferendumInfoOf").to_vec();
		let encoded = index.encode();
		key.extend_from_slice(&sp_io::hashing::twox_64(&encoded));
		key.extend_from_slice(&encoded);
		// ReferendumInfo::Finished has SCALE variant index 1. Reading only the discriminant
		// avoids decoding historical proposals containing retired RuntimeCall variants.
		sp_io::storage::get(&key).is_some_and(|value| value.first() == Some(&1))
	}

	fn migrate_key(phase: u8, key: &[u8]) -> Result<Weight, &'static str> {
		let mut reads = 0;
		let mut writes = 1;
		match phase {
			0 => {
				let id = <Runtime as pallet_staking::Config>::PositionItemId::decode(
					&mut key.get(48..).ok_or("invalid Gov v1 storage")?,
				)
				.map_err(|_| "invalid Gov v1 storage")?;
				let raw = sp_io::storage::get(key).ok_or("invalid Gov v1 storage")?;
				reads += 1;
				let votes = pallet_staking::types::Voting::<ConstU32<100>>::decode(&mut &raw[..])
					.map_err(|_| "invalid Gov v1 storage")?;
				if !votes.votes.is_empty() {
					reads += 1 + votes.votes.len() as u64;
					writes += 1;
					pallet_staking::migrations::settle_legacy_votes::<Runtime>(id, votes, Self::is_finished);
				}
			}
			2 => {
				let raw = sp_io::storage::get(key).ok_or("invalid Gov v1 storage")?;
				reads += 1;
				let (depositors, amount) = <(BoundedVec<AccountId, ConstU32<100>>, Balance)>::decode(&mut &raw[..])
					.map_err(|_| "invalid Gov v1 storage")?;
				for who in depositors {
					reads += 6;
					writes += 6;
					Balances::unreserve(&who, amount);
				}
			}
			3 => {
				let who = AccountId::decode(&mut key.get(40..).ok_or("invalid Gov v1 storage")?)
					.map_err(|_| "invalid Gov v1 storage")?;
				reads += 1;
				if pallet_balances::Locks::<Runtime>::get(&who)
					.iter()
					.any(|lock| lock.id == *b"democrac")
				{
					reads += 5;
					writes += 5;
					Balances::remove_lock(*b"democrac", &who);
				}
			}
			_ => {}
		}
		sp_io::storage::clear(key);
		Ok(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(reads, writes))
	}

	fn migrate() -> Result<Weight, &'static str> {
		let db = <Runtime as frame_system::Config>::DbWeight::get();
		let mut weight = Weight::zero();
		for phase in 0..=4 {
			let prefix = Self::prefix(phase);
			let mut removed = 0u64;
			loop {
				weight = weight.saturating_add(db.reads(1));
				let Some(key) = sp_io::storage::next_key(&prefix).filter(|key| key.starts_with(&prefix)) else {
					break;
				};
				weight = weight.saturating_add(Self::migrate_key(phase, &key)?);
				removed += 1;
			}
			log::info!(target: "runtime::migration", "RemoveDemocracy phase {phase}: removed {removed} keys");
		}
		log::info!(target: "runtime::migration", "RemoveDemocracy estimated DB weight: {weight:?}");
		Ok(weight)
	}
}

impl OnRuntimeUpgrade for RemoveDemocracy {
	fn on_runtime_upgrade() -> Weight {
		// The pallet is already absent from the runtime: a partial cleanup must never be committed.
		let result = with_transaction(|| match Self::migrate().map_err(DispatchError::Other) {
			Ok(weight) => TransactionOutcome::Commit(Ok(weight)),
			Err(error) => TransactionOutcome::Rollback(Err(error)),
		});
		match result {
			Ok(weight) => weight,
			Err(error) => panic!("Gov v1 cleanup failed: {error:?}"),
		}
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		use sp_std::collections::btree_map::BTreeMap;
		let mut refunds = BTreeMap::<AccountId, Balance>::new();
		let prefix = Self::prefix(2);
		let mut key = prefix.clone();
		while let Some(next) = sp_io::storage::next_key(&key).filter(|k| k.starts_with(&prefix)) {
			let raw = sp_io::storage::get(&next).ok_or("missing democracy deposit")?;
			let (accounts, amount) = <(BoundedVec<AccountId, ConstU32<100>>, Balance)>::decode(&mut &raw[..])
				.map_err(|_| "invalid democracy deposit")?;
			for who in accounts {
				let refund = refunds.entry(who).or_default();
				*refund = refund.saturating_add(amount);
			}
			key = next;
		}
		let expected: Vec<_> = refunds
			.into_iter()
			.map(|(who, amount)| {
				let reserved = Balances::reserved_balance(&who);
				(who, reserved.saturating_sub(amount))
			})
			.collect();
		let other_locks: Vec<_> = pallet_balances::Locks::<Runtime>::iter()
			.filter(|(_, locks)| locks.iter().any(|lock| lock.id == *b"democrac"))
			.map(|(who, locks)| {
				let remaining: Vec<_> = locks.into_iter().filter(|lock| lock.id != *b"democrac").collect();
				(who, remaining)
			})
			.collect();
		Ok((expected, other_locks).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		type Snapshot = (
			Vec<(AccountId, Balance)>,
			Vec<(AccountId, Vec<pallet_balances::BalanceLock<Balance>>)>,
		);
		let (expected, other_locks) =
			Snapshot::decode(&mut &state[..]).map_err(|_| "invalid democracy migration snapshot")?;
		for (who, reserved) in expected {
			frame_support::ensure!(
				Balances::reserved_balance(&who) == reserved,
				"incorrect democracy refund"
			);
		}
		for (who, locks) in other_locks {
			frame_support::ensure!(
				pallet_balances::Locks::<Runtime>::get(who).to_vec() == locks,
				"non-democracy balance locks changed"
			);
		}
		for phase in [0, 1, 4] {
			frame_support::ensure!(
				!frame_support::storage::unhashed::contains_prefixed_key(&Self::prefix(phase)),
				"Gov v1 storage remains after cleanup"
			);
		}
		frame_support::ensure!(
			!pallet_balances::Locks::<Runtime>::iter_values().any(|locks| locks.iter().any(|l| l.id == *b"democrac")),
			"Gov v1 balance lock remains after cleanup"
		);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::traits::{Currency, WithdrawReasons};

	fn map_key(storage: &[u8], encoded_key: &[u8]) -> Vec<u8> {
		let mut key = storage_prefix(b"Democracy", storage).to_vec();
		key.extend_from_slice(&sp_io::hashing::twox_64(encoded_key));
		key.extend_from_slice(encoded_key);
		key
	}

	fn run_cleanup() -> Weight {
		#[cfg(feature = "try-runtime")]
		let snapshot = RemoveDemocracy::pre_upgrade().unwrap();
		let weight = RemoveDemocracy::on_runtime_upgrade();
		#[cfg(feature = "try-runtime")]
		RemoveDemocracy::post_upgrade(snapshot).unwrap();
		weight
	}

	#[test]
	fn cleanup_should_refund_deposits_and_release_only_legacy_locks_when_upgraded() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let who = AccountId::new([1; 32]);
			Balances::make_free_balance_be(&who, 1_000_000_000_000_000_000);
			assert!(Balances::reserve(&who, 3_000).is_ok());
			Balances::set_lock(*b"democrac", &who, 10_000, WithdrawReasons::all());
			Balances::set_lock(*b"pyconvot", &who, 5_000, WithdrawReasons::all());
			let depositors = BoundedVec::<_, ConstU32<100>>::try_from(vec![who.clone(), who.clone()]).unwrap();
			sp_io::storage::set(&map_key(b"DepositOf", &0u32.encode()), &(depositors, 500u128).encode());
			// Values need not be decoded to release direct, delegated, or prior democracy locks.
			sp_io::storage::set(&map_key(b"VotingOf", &who.encode()), &[0]);
			sp_io::storage::set(&storage_prefix(b"Democracy", b"PublicPropCount"), &5u32.encode());
			let opengov_key = storage_prefix(b"ConvictionVoting", b"VotingFor");
			sp_io::storage::set(&opengov_key, &[42]);
			let legacy_votes = pallet_staking::types::Voting::<ConstU32<100>>::default();
			pallet_staking::migrations::PositionVotes::<Runtime>::insert(1, legacy_votes);
			let mut processed_key = storage_prefix(b"Staking", b"ProcessedVotes").to_vec();
			processed_key.extend_from_slice(&[1; 64]);
			sp_io::storage::set(&processed_key, &[0]);
			let call = Box::new(crate::RuntimeCall::System(frame_system::Call::remark {
				remark: vec![1],
			}));
			assert!(crate::Scheduler::schedule(crate::RuntimeOrigin::root(), 100, None, 63, call).is_ok());
			let agenda = pallet_scheduler::Agenda::<Runtime>::get(100).encode();

			assert!(run_cleanup().ref_time() > 0);
			assert_eq!(Balances::reserved_balance(&who), 2_000);
			let locks = pallet_balances::Locks::<Runtime>::get(&who);
			assert_eq!(locks.len(), 1);
			assert_eq!(locks[0].id, *b"pyconvot");
			assert_eq!(locks[0].amount, 5_000);
			assert_eq!(sp_io::storage::get(&opengov_key).unwrap().to_vec(), vec![42]);
			assert!(!frame_support::storage::unhashed::contains_prefixed_key(
				&RemoveDemocracy::prefix(4)
			));
			assert!(!pallet_staking::migrations::PositionVotes::<Runtime>::contains_key(1));
			assert!(!sp_io::storage::exists(&processed_key));
			assert_eq!(pallet_scheduler::Agenda::<Runtime>::get(100).encode(), agenda);

			run_cleanup();
			assert_eq!(Balances::reserved_balance(&who), 2_000);
		});
	}

	#[test]
	fn cleanup_should_settle_orphan_staking_votes_when_democracy_votes_were_removed() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let id = 1u128;
			let mut position_key = storage_prefix(b"Staking", b"Positions").to_vec();
			position_key.extend_from_slice(&sp_io::hashing::blake2_128(&id.encode()));
			position_key.extend_from_slice(&id.encode());
			let position = pallet_staking::types::Position::new(1_000, sp_runtime::FixedU128::default(), 1u32);
			sp_io::storage::set(&position_key, &position.encode());
			let vote = pallet_staking::types::Vote::new(1_000, pallet_staking::types::Conviction::Locked6x);
			let voting = pallet_staking::types::Voting::<ConstU32<100>> {
				votes: vec![(1, vote), (2, vote), (3, vote)].try_into().unwrap(),
			};
			pallet_staking::migrations::PositionVotes::<Runtime>::insert(id, voting);
			sp_io::storage::set(
				&map_key(b"ReferendumInfoOf", &1u32.encode()),
				&(1u8, true, 1u32).encode(),
			);
			sp_io::storage::set(&map_key(b"ReferendumInfoOf", &2u32.encode()), &[0]);

			run_cleanup();
			assert_eq!(crate::Staking::get_position(id).unwrap().get_action_points(), 100);
			assert!(!pallet_staking::migrations::PositionVotes::<Runtime>::contains_key(id));
			run_cleanup();
			assert_eq!(crate::Staking::get_position(id).unwrap().get_action_points(), 100);
		});
	}

	#[test]
	fn cleanup_should_release_delegation_locks_when_account_has_no_direct_votes() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let who = AccountId::new([2; 32]);
			Balances::make_free_balance_be(&who, 1_000_000_000_000_000_000);
			Balances::set_lock(*b"democrac", &who, 10_000, WithdrawReasons::all());
			Balances::set_lock(*b"stk_stks", &who, 5_000, WithdrawReasons::all());
			sp_io::storage::set(&map_key(b"VotingOf", &who.encode()), &[1]);

			run_cleanup();
			let locks = pallet_balances::Locks::<Runtime>::get(&who);
			assert_eq!(locks.len(), 1);
			assert_eq!(locks[0].id, *b"stk_stks");
			assert_eq!(locks[0].amount, 5_000);
		});
	}

	#[test]
	fn cleanup_should_roll_back_when_deposit_cannot_be_decoded() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let key = map_key(b"DepositOf", &0u32.encode());
			sp_io::storage::set(&key, &[255]);
			pallet_staking::migrations::PositionVotes::<Runtime>::insert(
				1,
				pallet_staking::types::Voting::<ConstU32<100>>::default(),
			);
			let result = std::panic::catch_unwind(RemoveDemocracy::on_runtime_upgrade);
			assert!(result.is_err());
			assert!(sp_io::storage::exists(&key));
			assert!(pallet_staking::migrations::PositionVotes::<Runtime>::contains_key(1));
		});
	}
}
