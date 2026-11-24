// Copyright (C) 2020-2026 Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Retires legacy DCA (`DCA`, index 66): schedules become DCA intents or are refunded, `dcaorder`
//! reserves are released and the `DCA` prefix is cleared. The pallet crate is gone, so the old
//! storage is read through aliases.

use crate::evm::erc20_currency::GAS_LIMIT as ERC20_GAS_LIMIT;
use crate::{AccountId, AssetRegistry, Balance, Currencies, Dispatcher, Intent, Runtime};
use codec::{Decode, Encode};
use frame_support::{
	storage::{unhashed, with_transaction, KeyPrefixIterator},
	storage_alias,
	traits::OnRuntimeUpgrade,
	weights::Weight,
	Blake2_128Concat,
};
use hydradx_traits::{evm::ExtraGasSupport, router::Trade, BoundErc20};
use ice_support::DcaParams;
use orml_traits::NamedMultiReservableCurrency;
use pallet_evm::GasWeightMapping;
use pallet_transaction_pause::{BoundedName, PausedTransactions};
use primitives::AssetId;
use sp_runtime::{DispatchError, Permill, TransactionOutcome};
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use sp_std::vec::Vec;

pub const DCA_RESERVE_ID: [u8; 8] = *b"dcaorder";
/// Above this the migration aborts; mainnet holds about half.
pub const MAX_SCHEDULES: usize = 30;
pub const MAX_DCA_KEYS: u32 = 1_000;
/// Upper estimate for one entry's key, value and trie nodes.
const PROOF_SIZE_PER_READ: u64 = 512;
/// Legacy `MaxPriceDifferenceBetweenBlocks`, applied when a schedule set no slippage.
const DEFAULT_SLIPPAGE: Permill = Permill::from_parts(15_000);
/// Moving an indebted owner's aToken runs Aave's health-factor check: ~600k gas against the 400k limit.
const EXTRA_GAS: u64 = 1_000_000;
const LOG_TARGET: &str = "runtime::migration";

#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub struct LegacySchedule {
	pub owner: AccountId,
	pub period: u32,
	pub total_amount: Balance,
	pub max_retries: Option<u8>,
	pub stability_threshold: Option<Permill>,
	pub slippage: Option<Permill>,
	pub order: LegacyOrder,
}

// `Vec` where legacy had `BoundedVec`: same encoding, and an over-bound value still decodes.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq)]
pub enum LegacyOrder {
	Sell {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		route: Vec<Trade<AssetId>>,
	},
	Buy {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		route: Vec<Trade<AssetId>>,
	},
}

impl LegacyOrder {
	fn asset_in(&self) -> AssetId {
		match self {
			LegacyOrder::Sell { asset_in, .. } | LegacyOrder::Buy { asset_in, .. } => *asset_in,
		}
	}
}

#[storage_alias(verbatim)]
pub type Schedules = StorageMap<DCA, Blake2_128Concat, u32, LegacySchedule>;

#[storage_alias(verbatim)]
pub type RemainingAmounts = StorageMap<DCA, Blake2_128Concat, u32, Balance>;

/// What legacy `on_initialize` executed from.
#[storage_alias(verbatim)]
pub type ScheduleIdsPerBlock = StorageMap<DCA, Blake2_128Concat, u32, Vec<u32>>;

pub struct MigrateDcaToIntents;

#[derive(Default)]
struct Meter {
	reads: u64,
	writes: u64,
	gas: u64,
}

impl MigrateDcaToIntents {
	fn migrate(meter: &mut Meter) {
		let mut schedules: Vec<_> = Schedules::iter().take(MAX_SCHEDULES + 1).collect();
		meter.reads += schedules.len() as u64;
		if schedules.len() > MAX_SCHEDULES {
			log::error!(target: LOG_TARGET, "DCA retirement aborted: more than {MAX_SCHEDULES} schedules, nothing changed");
			return;
		}
		let planned_blocks: Vec<_> = ScheduleIdsPerBlock::iter().take(MAX_SCHEDULES + 1).collect();
		meter.reads += planned_blocks.len() as u64;
		if planned_blocks.len() > MAX_SCHEDULES {
			log::error!(target: LOG_TARGET, "DCA retirement aborted: more than {MAX_SCHEDULES} planned blocks, nothing changed");
			return;
		}
		let prefix = sp_io::hashing::twox_128(b"DCA").to_vec();
		let dca_keys = KeyPrefixIterator::new(prefix.clone(), prefix, |_| Ok(()))
			.take(MAX_DCA_KEYS as usize + 1)
			.count();
		meter.reads += dca_keys as u64;
		if dca_keys > MAX_DCA_KEYS as usize {
			log::error!(target: LOG_TARGET, "DCA retirement aborted: more than {MAX_DCA_KEYS} DCA keys, nothing changed");
			return;
		}

		let mut planned = BTreeMap::<u32, u32>::new();
		for (block, ids) in planned_blocks {
			for id in ids {
				planned.entry(id).and_modify(|b| *b = (*b).min(block)).or_insert(block);
			}
		}

		let legacy: BTreeSet<_> = schedules
			.iter()
			.map(|(_, schedule)| (schedule.owner.clone(), schedule.order.asset_in()))
			.collect();
		schedules.sort_by_key(|(id, _)| *id);
		for (id, schedule) in schedules {
			Self::migrate_schedule(id, &schedule, planned.get(&id).copied(), meter);
			// The `Schedules` entry stays, so a re-run sees an orphan and still retries its reserve.
			RemainingAmounts::remove(id);
			meter.writes += 1;
		}

		if !Self::release_legacy_reserves(legacy, meter) {
			log::error!(target: LOG_TARGET, "DCA retirement not finalised: dcaorder reserves may remain, run the migration again");
			return;
		}

		PausedTransactions::<Runtime>::remove(dca_pause_key());
		meter.writes += 1;

		let cleared = unhashed::clear_prefix(&sp_io::hashing::twox_128(b"DCA"), Some(MAX_DCA_KEYS), None);
		meter.writes += cleared.unique as u64;
		if cleared.maybe_cursor.is_some() {
			log::error!(target: LOG_TARGET, "DCA keys remain after removing {MAX_DCA_KEYS}, run the migration again");
		}
		log::info!(target: LOG_TARGET, "DCA retired: {} keys removed", cleared.unique);
	}

	fn migrate_schedule(id: u32, schedule: &LegacySchedule, planned: Option<u32>, meter: &mut Meter) {
		meter.reads += 1;
		let Some(remaining) = RemainingAmounts::get(id) else {
			log::info!(target: LOG_TARGET, "DCA schedule {id}: orphan without remaining amount, skipped");
			return;
		};

		let LegacyOrder::Sell {
			asset_in,
			asset_out,
			amount_in,
			min_amount_out,
			..
		} = schedule.order
		else {
			return Self::cancel(id, schedule, remaining, "buy order", meter);
		};
		let budget = (schedule.total_amount != 0).then_some(remaining);
		if budget.is_some() && remaining < amount_in {
			return Self::cancel(id, schedule, remaining, "budget below one trade", meter);
		}
		let params = DcaParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_amount_out,
			slippage: schedule.slippage.unwrap_or(DEFAULT_SLIPPAGE),
			budget,
			period: schedule.period,
		};
		// A schedule planned at P traded in P; an intent offered at P - 1 settles in P.
		let first_eligible = planned.map_or(0, |block| block.saturating_sub(1));

		let (result, extra_gas) = Self::with_extra_gas(asset_in, 2, meter, || {
			let left = Currencies::unreserve_named(&DCA_RESERVE_ID, asset_in, &schedule.owner, remaining);
			if left != 0 {
				return Err(DispatchError::Other("legacy reserve below remaining amount"));
			}
			Intent::do_add_migrated_intent(schedule.owner.clone(), params, first_eligible)
		});
		match result {
			Ok(intent_id) => {
				meter.reads += 8;
				meter.writes += 8;
				log::info!(target: LOG_TARGET, "DCA schedule {id} -> intent {intent_id} (extra gas used: {extra_gas})");
			}
			Err(error) => Self::cancel(id, schedule, remaining, error, meter),
		}
	}

	fn cancel(
		id: u32,
		schedule: &LegacySchedule,
		remaining: Balance,
		reason: impl core::fmt::Debug,
		meter: &mut Meter,
	) {
		let asset = schedule.order.asset_in();
		let (left, extra_gas) = Self::with_extra_gas(asset, 1, meter, || {
			Ok(Currencies::unreserve_named(
				&DCA_RESERVE_ID,
				asset,
				&schedule.owner,
				remaining,
			))
		});
		let refunded = remaining.saturating_sub(left.unwrap_or(remaining));
		log::info!(target: LOG_TARGET, "DCA schedule {id} cancelled ({reason:?}): refunded {refunded} of asset {asset} (extra gas used: {extra_gas})");
	}

	/// Returns whether every `dcaorder` reserve was released.
	fn release_legacy_reserves(legacy: BTreeSet<(AccountId, AssetId)>, meter: &mut Meter) -> bool {
		let mut clean = true;
		for (owner, asset) in legacy {
			meter.reads += 1;
			let reserved = Currencies::reserved_balance_named(&DCA_RESERVE_ID, asset, &owner);
			if reserved == 0 {
				continue;
			}
			let (left, _) = Self::with_extra_gas(asset, 1, meter, || {
				Ok(Currencies::unreserve_named(&DCA_RESERVE_ID, asset, &owner, reserved))
			});
			let left = left.unwrap_or(reserved);
			if left != 0 {
				clean = false;
				log::error!(target: LOG_TARGET, "DCA reserve could not be released: {left} of asset {asset} stays reserved under dcaorder for {owner:?}");
			}
		}
		clean
	}

	/// Runs `f` in a storage layer with `EXTRA_GAS`; the gas used is read before a rollback erases it.
	fn with_extra_gas<R>(
		asset: AssetId,
		evm_calls: u64,
		meter: &mut Meter,
		f: impl FnOnce() -> Result<R, DispatchError>,
	) -> (Result<R, DispatchError>, u64) {
		if AssetRegistry::contract_address(asset).is_some() {
			meter.gas += evm_calls * ERC20_GAS_LIMIT;
		}
		Dispatcher::set_extra_gas(EXTRA_GAS);
		let mut extra_used = 0;
		let result = with_transaction(|| {
			let result = f();
			extra_used = EXTRA_GAS.saturating_sub(Dispatcher::extra_gas());
			if result.is_ok() {
				TransactionOutcome::Commit(result)
			} else {
				TransactionOutcome::Rollback(result)
			}
		});
		Dispatcher::clear_extra_gas();
		meter.gas += extra_used;
		meter.reads += 6;
		meter.writes += 6;
		(result, extra_used)
	}
}

impl OnRuntimeUpgrade for MigrateDcaToIntents {
	fn on_runtime_upgrade() -> Weight {
		let mut meter = Meter::default();
		Self::migrate(&mut meter);
		let weight = <Runtime as frame_system::Config>::DbWeight::get()
			.reads_writes(meter.reads, meter.writes)
			.saturating_add(<Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
				meter.gas, false,
			))
			.saturating_add(Weight::from_parts(0, meter.reads.saturating_mul(PROOF_SIZE_PER_READ)));
		log::info!(target: LOG_TARGET, "DCA retirement weight: {weight:?} (gas charged: {})", meter.gas);
		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let current = frame_system::Pallet::<Runtime>::block_number();
		let mut planned = BTreeMap::<u32, u32>::new();
		for (block, ids) in ScheduleIdsPerBlock::iter() {
			for id in ids {
				planned.entry(id).and_modify(|b| *b = (*b).min(block)).or_insert(block);
			}
		}
		let convertible: Vec<Expected> = Schedules::iter()
			.filter_map(|(id, schedule)| {
				let remaining = RemainingAmounts::get(id)?;
				let LegacyOrder::Sell {
					asset_in,
					asset_out,
					amount_in,
					..
				} = schedule.order
				else {
					return None;
				};
				let rolling = schedule.total_amount == 0;
				if !rolling && remaining < amount_in {
					return None;
				}
				let first_eligible = planned.get(&id).map_or(0, |block| block.saturating_sub(1));
				Some((
					schedule.owner,
					asset_in,
					asset_out,
					amount_in,
					(!rolling).then_some(remaining),
					current.max(first_eligible).saturating_sub(schedule.period),
				))
			})
			.collect();
		let intents: Vec<ice_support::IntentId> = pallet_intent::Intents::<Runtime>::iter_keys().collect();
		Ok((intents, convertible).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::ensure;
		let (before, mut convertible) = <(Vec<ice_support::IntentId>, Vec<Expected>)>::decode(&mut &state[..])
			.map_err(|_| "invalid DCA migration snapshot")?;

		ensure!(
			!unhashed::contains_prefixed_key(&sp_io::hashing::twox_128(b"DCA")),
			"DCA storage remains"
		);
		ensure!(
			!pallet_balances::Reserves::<Runtime>::iter_values()
				.any(|reserves| reserves.iter().any(|r| r.id == DCA_RESERVE_ID && r.amount > 0)),
			"native balance still reserved under dcaorder"
		);
		ensure!(
			!orml_tokens::Reserves::<Runtime>::iter_values()
				.any(|reserves| reserves.iter().any(|r| r.id == DCA_RESERVE_ID && r.amount > 0)),
			"token balance still reserved under dcaorder"
		);
		ensure!(
			!PausedTransactions::<Runtime>::contains_key(dca_pause_key()),
			"DCA schedule is still paused"
		);

		let mut converted = 0usize;
		for (id, intent) in pallet_intent::Intents::<Runtime>::iter() {
			if before.contains(&id) {
				continue;
			}
			let ice_support::IntentData::Dca(dca) = intent.data else {
				return Err("migration created a non-DCA intent".into());
			};
			let owner = Intent::intent_owner(id).ok_or("intent without owner")?;
			let key = (
				owner,
				dca.asset_in,
				dca.asset_out,
				dca.amount_in,
				dca.budget,
				dca.last_execution_block,
			);
			let index = convertible
				.iter()
				.position(|expected| *expected == key)
				.ok_or("intent does not match a convertible schedule")?;
			convertible.swap_remove(index);
			converted += 1;
		}
		for (owner, asset_in, ..) in &convertible {
			log::warn!(target: LOG_TARGET, "convertible DCA schedule of {owner:?} selling {asset_in} was cancelled instead");
		}
		log::info!(target: LOG_TARGET, "DCA retirement created {converted} intents, one per converted schedule");
		Ok(())
	}
}

fn dca_pause_key() -> (BoundedName, BoundedName) {
	(
		BoundedName::truncate_from(b"DCA".to_vec()),
		BoundedName::truncate_from(b"schedule".to_vec()),
	)
}

#[cfg(feature = "try-runtime")]
type Expected = (AccountId, AssetId, AssetId, Balance, Option<Balance>, u32);
