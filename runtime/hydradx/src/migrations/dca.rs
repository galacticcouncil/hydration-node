// Copyright (C) 2020-2026 Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Retires legacy DCA (`DCA`, index 66): each schedule becomes a DCA intent or is cancelled and
//! refunded (at most `MAX_SCHEDULES` per run; beyond that the run stops and leaves the rest), whatever is still reserved under `dcaorder` on any account is released, the paused
//! `DCA.schedule` entry is dropped and the `DCA` prefix is cleared.
//! Reads the old storage through aliases because the pallet crate is gone.

use crate::evm::erc20_currency::GAS_LIMIT as ERC20_GAS_LIMIT;
use crate::{AccountId, AssetRegistry, Balance, Currencies, Dispatcher, Intent, NativeAssetId, Runtime};
use codec::{Decode, Encode};
use frame_support::{
	storage::{unhashed, with_transaction},
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
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub const DCA_RESERVE_ID: [u8; 8] = *b"dcaorder";
/// Block-weight safety stop per run, about twice what mainnet holds.
pub const MAX_SCHEDULES: usize = 30;
/// Legacy `MaxPriceDifferenceBetweenBlocks`, applied when a schedule set no slippage.
const DEFAULT_SLIPPAGE: Permill = Permill::from_parts(15_000);
/// On top of `ERC20_GAS_LIMIT` per reserve-moving step. Re-reserving an aToken runs Aave's health-factor
/// walk over the owner's reserves: measured ~598k gas in total (400k base + ~198k extra) for mainnet's
/// indebted owner with 12 reserves.
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

// Routes and per-block id lists are decoded unbounded: same encoding as the legacy `BoundedVec`s,
// but a value over the old bound must not make a schedule unreadable here.
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

/// What legacy `on_initialize` executed from; `ScheduleExecutionBlock` is only the per-id index of it.
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
		let mut planned = BTreeMap::<u32, u32>::new();
		for (block, ids) in ScheduleIdsPerBlock::iter() {
			meter.reads += 1;
			for id in ids {
				planned.entry(id).and_modify(|b| *b = (*b).min(block)).or_insert(block);
			}
		}

		let mut schedules: Vec<_> = Schedules::iter().take(MAX_SCHEDULES + 1).collect();
		meter.reads += schedules.len() as u64;
		schedules.sort_by_key(|(id, _)| *id);
		let limit_hit = schedules.len() > MAX_SCHEDULES;

		for (id, schedule) in schedules.into_iter().take(MAX_SCHEDULES) {
			Self::migrate_schedule(id, &schedule, planned.get(&id).copied(), meter);
			Schedules::remove(id);
			RemainingAmounts::remove(id);
			meter.writes += 2;
		}

		if limit_hit {
			log::error!(target: LOG_TARGET, "DCA retirement stopped after {MAX_SCHEDULES} schedules: legacy DCA state remains, run the migration again");
			return;
		}

		Self::release_legacy_reserves(meter);

		PausedTransactions::<Runtime>::remove(dca_pause_key());
		meter.writes += 1;

		let removed = unhashed::clear_prefix(&sp_io::hashing::twox_128(b"DCA"), None, None).unique;
		meter.writes += removed as u64;
		log::info!(target: LOG_TARGET, "DCA retired: {removed} keys removed");
	}

	fn migrate_schedule(id: u32, schedule: &LegacySchedule, planned: Option<u32>, meter: &mut Meter) {
		meter.reads += 1;
		let Some(remaining) = RemainingAmounts::get(id) else {
			log::info!(target: LOG_TARGET, "DCA schedule {id}: orphan without remaining amount, deleted");
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
		// A schedule planned at P traded inside block P. An intent is offered from block X's state and
		// settles in X + 1, so X = P - 1 keeps the trade in the same block.
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

	/// Walks every account: `unlock_reserves` is gone with the pallet, so a `dcaorder` reserve no
	/// schedule accounted for (or an erc20 unreserve that failed above) would otherwise stay locked.
	fn release_legacy_reserves(meter: &mut Meter) {
		let native = pallet_balances::Reserves::<Runtime>::iter().map(|(owner, reserves)| {
			let amount = reserves.iter().find(|r| r.id == DCA_RESERVE_ID).map(|r| r.amount);
			(owner, NativeAssetId::get(), amount)
		});
		let tokens = orml_tokens::Reserves::<Runtime>::iter().map(|(owner, asset, reserves)| {
			let amount = reserves.iter().find(|r| r.id == DCA_RESERVE_ID).map(|r| r.amount);
			(owner, asset, amount)
		});
		let legacy: Vec<_> = native
			.chain(tokens)
			.inspect(|_| meter.reads += 1)
			.filter_map(|(owner, asset, amount)| Some((owner, asset, amount?)))
			.collect();
		let skipped = legacy.len().saturating_sub(MAX_SCHEDULES);
		if skipped > 0 {
			log::error!(target: LOG_TARGET, "DCA reserve release stopped after {MAX_SCHEDULES}: {skipped} dcaorder reserves stay reserved");
		}
		for (owner, asset, reserved) in legacy.into_iter().take(MAX_SCHEDULES) {
			let (left, _) = Self::with_extra_gas(asset, 1, meter, || {
				Ok(Currencies::unreserve_named(&DCA_RESERVE_ID, asset, &owner, reserved))
			});
			let left = left.unwrap_or(reserved);
			if left != 0 {
				log::error!(target: LOG_TARGET, "DCA reserve could not be released: {left} of asset {asset} stays reserved under dcaorder for {owner:?}");
			}
		}
	}

	/// Runs `f` in its own storage layer with `EXTRA_GAS` on top of each EVM call's limit. Charges
	/// `evm_calls` bound-erc20 calls at their base limit plus the extra gas consumed, which is read
	/// before a rollback would erase it.
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
			));
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
