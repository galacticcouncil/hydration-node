// Copyright (C) 2020-2026 Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Retires Gov v1 without dropping deposits, voter locks, or earned staking points.
//! Pending proposals and ongoing referenda are retired. Scheduler owns approved enactments and
//! their preimage requests; they remain scheduled.

use crate::{AccountId, Balance, Balances, Runtime};
use codec::{Decode, Encode};
use frame_support::{
	migrations::{SteppedMigration, SteppedMigrationError},
	storage::storage_prefix,
	traits::{ConstU32, LockableCurrency, ReservableCurrency},
	weights::{Weight, WeightMeter},
	BoundedVec,
};
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

	fn item_weight(phase: u8) -> Weight {
		let db = <Runtime as frame_system::Config>::DbWeight::get();
		match phase {
			// A legacy position or proposal contains at most 100 votes or depositors.
			0 | 2 => db
				.reads_writes(600, 600)
				.saturating_add(Weight::from_parts(1_000_000_000, 1_048_576)),
			3 => db
				.reads_writes(6, 6)
				.saturating_add(Weight::from_parts(100_000_000, 32_768)),
			_ => db
				.reads_writes(2, 1)
				.saturating_add(Weight::from_parts(10_000_000, 65_536)),
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

	fn migrate_key(phase: u8, key: &[u8]) -> Result<(), SteppedMigrationError> {
		match phase {
			0 => {
				let id = <Runtime as pallet_staking::Config>::PositionItemId::decode(
					&mut key.get(48..).ok_or(SteppedMigrationError::Failed)?,
				)
				.map_err(|_| SteppedMigrationError::Failed)?;
				let raw = sp_io::storage::get(key).ok_or(SteppedMigrationError::Failed)?;
				let votes = pallet_staking::types::Voting::<ConstU32<100>>::decode(&mut &raw[..])
					.map_err(|_| SteppedMigrationError::Failed)?;
				pallet_staking::migrations::settle_legacy_votes::<Runtime>(id, votes, Self::is_finished);
			}
			2 => {
				let raw = sp_io::storage::get(key).ok_or(SteppedMigrationError::Failed)?;
				let (depositors, amount) = <(BoundedVec<AccountId, ConstU32<100>>, Balance)>::decode(&mut &raw[..])
					.map_err(|_| SteppedMigrationError::Failed)?;
				for who in depositors {
					Balances::unreserve(&who, amount);
				}
			}
			3 => {
				let who = AccountId::decode(&mut key.get(40..).ok_or(SteppedMigrationError::Failed)?)
					.map_err(|_| SteppedMigrationError::Failed)?;
				Balances::remove_lock(*b"democrac", &who);
			}
			_ => {}
		}
		sp_io::storage::clear(key);
		Ok(())
	}
}

impl SteppedMigration for RemoveDemocracy {
	type Cursor = u8;
	type Identifier = [u8; 18];

	fn id() -> Self::Identifier {
		*b"hydra-gov1-cleanup"
	}

	fn step(cursor: Option<u8>, meter: &mut WeightMeter) -> Result<Option<u8>, SteppedMigrationError> {
		let mut phase = cursor.unwrap_or_default();
		if phase > 4 {
			return Err(SteppedMigrationError::InvalidCursor);
		}
		let required = Self::item_weight(phase);
		if !meter.can_consume(required) {
			return Err(SteppedMigrationError::InsufficientWeight { required });
		}
		loop {
			if meter.try_consume(Self::item_weight(phase)).is_err() {
				return Ok(Some(phase));
			}
			let prefix = Self::prefix(phase);
			match sp_io::storage::next_key(&prefix).filter(|key| key.starts_with(&prefix)) {
				Some(key) => Self::migrate_key(phase, &key)?,
				None if phase < 4 => phase += 1,
				None => return Ok(None),
			}
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
		Ok(expected.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let expected =
			Vec::<(AccountId, Balance)>::decode(&mut &state[..]).map_err(|_| "invalid democracy migration snapshot")?;
		for (who, reserved) in expected {
			frame_support::ensure!(
				Balances::reserved_balance(&who) == reserved,
				"incorrect democracy refund"
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

	#[cfg(not(feature = "runtime-benchmarks"))]
	#[test]
	fn cleanup_should_record_completion_when_run_by_runtime_migrator() {
		use frame_support::{migrations::MultiStepMigrator, traits::Hooks};
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let key = storage_prefix(b"Democracy", b"PublicPropCount");
			sp_io::storage::set(&key, &5u32.encode());
			<crate::MultiBlockMigrations as Hooks<crate::BlockNumber>>::on_runtime_upgrade();
			for block in 1..20 {
				crate::System::set_block_number(block);
				crate::MultiBlockMigrations::step();
				if !crate::MultiBlockMigrations::ongoing() {
					break;
				}
			}
			assert!(!crate::MultiBlockMigrations::ongoing());
			assert!(!sp_io::storage::exists(&key));
			assert_eq!(pallet_migrations::Historic::<Runtime>::iter().count(), 1);
			<crate::MultiBlockMigrations as Hooks<crate::BlockNumber>>::on_runtime_upgrade();
			crate::MultiBlockMigrations::step();
			assert_eq!(pallet_migrations::Historic::<Runtime>::iter().count(), 1);
		});
	}

	fn map_key(storage: &[u8], encoded_key: &[u8]) -> Vec<u8> {
		let mut key = storage_prefix(b"Democracy", storage).to_vec();
		key.extend_from_slice(&sp_io::hashing::twox_64(encoded_key));
		key.extend_from_slice(encoded_key);
		key
	}

	fn run_cleanup() -> u32 {
		#[cfg(feature = "try-runtime")]
		let snapshot = RemoveDemocracy::pre_upgrade().unwrap();
		let mut cursor = None;
		let mut steps = 0;
		loop {
			let mut meter = WeightMeter::with_limit(RemoveDemocracy::item_weight(0));
			cursor = RemoveDemocracy::transactional_step(cursor, &mut meter).unwrap();
			steps += 1;
			assert!(steps < 100);
			if cursor.is_none() {
				#[cfg(feature = "try-runtime")]
				RemoveDemocracy::post_upgrade(snapshot).unwrap();
				return steps;
			}
		}
	}

	#[test]
	fn cleanup_should_refund_deposits_and_release_only_legacy_locks_when_resumed() {
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

			assert!(run_cleanup() > 1);
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
	fn cleanup_should_roll_back_when_deposit_cannot_be_decoded() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let key = map_key(b"DepositOf", &0u32.encode());
			sp_io::storage::set(&key, &[255]);
			let mut meter = WeightMeter::with_limit(RemoveDemocracy::item_weight(2));
			assert!(matches!(
				RemoveDemocracy::transactional_step(Some(2), &mut meter),
				Err(SteppedMigrationError::Failed)
			));
			assert!(sp_io::storage::exists(&key));
		});
	}

	#[test]
	fn cleanup_should_leave_state_unchanged_when_weight_is_insufficient() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let mut meter = WeightMeter::with_limit(Weight::zero());
			assert!(matches!(
				RemoveDemocracy::transactional_step(None, &mut meter),
				Err(SteppedMigrationError::InsufficientWeight { .. })
			));
		});
	}
}
