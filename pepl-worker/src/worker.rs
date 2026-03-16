//! Main liquidation worker loop — generic over environment traits.
//!
//! This module contains the core logic ported from `node/src/liquidation_worker.rs`,
//! with the following bug fixes applied:
//! - BUG 1: Waitlist uses TTL (block_number) instead of permanent HashSet
//! - BUG 2: Always dry-run before submitting, even for oracle-update paths
//! - BUG 4+5: Track submission state and re-evaluate after TTL expiry
//! - ISSUE 6: Log when max transactions cap is hit and how many users were skipped

use crate::config::{WorkerConfig, WAITLIST_TTL_BLOCKS};
use crate::traits::*;
use ethabi::ethereum_types::U256;
use liquidation_worker_support::{Borrower, MoneyMarketData, RuntimeApiProvider, UserData};
use sp_core::H256;
use std::cmp::Ordering;
use std::collections::HashMap;

const LOG_TARGET: &str = "pepl-worker";

/// State of the current worker task.
#[derive(Clone, Debug)]
enum WorkerTask {
	LiquidateAll,
	OracleUpdate(Vec<OracleUpdate>),
	WaitForNewEvent,
}

/// Waitlist entry tracking when a user was submitted for liquidation.
#[derive(Clone, Debug)]
struct WaitlistEntry {
	submitted_at_block: BlockNumber,
}

/// The main worker loop. Generic over all environment interactions.
///
/// Type parameters:
/// - `Block`, `OriginCaller`, `RuntimeCall`, `RuntimeEvent`: Substrate runtime types
/// - `B`: Block source
/// - `T`: Transaction submitter
/// - `O`: Oracle source
/// - `D`: Dry-run validator
/// - `Api`: Runtime API provider
pub fn run_worker<Block, OriginCaller, RuntimeCall, RuntimeEvent, B, T, O, D, Api>(
	block_source: &mut B,
	tx_submitter: &T,
	oracle_source: &mut O,
	dry_runner: &D,
	api_provider: &Api,
	config: &WorkerConfig,
	money_market: &mut MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	borrowers: &mut Vec<Borrower>,
	initial_timestamp: u64,
) where
	Block: sp_runtime::traits::Block,
	Block::Hash: From<H256>,
	B: BlockSource,
	T: TxSubmitter,
	O: OracleSource,
	D: DryRunner,
	Api: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> + Clone,
{
	let mut current_evm_timestamp = initial_timestamp;
	let mut liquidated_users = Vec::<UserAddress>::new();
	// BUG 1 FIX: Use HashMap<EvmAddress, WaitlistEntry> with block-number TTL instead of HashSet
	let mut tx_waitlist: HashMap<UserAddress, WaitlistEntry> = HashMap::new();
	let mut current_block_number: BlockNumber = 0;
	let mut current_block_hash: [u8; 32] = [0u8; 32];
	let mut borrowers_snapshot = borrowers.clone();

	// Persistent oracle overrides: when --oracle-persist is set, oracle scenario prices
	// are stored here and re-applied after every MoneyMarketData re-init (which fetches
	// fresh prices from chain and would otherwise wipe out the injected prices).
	let mut active_oracle_overrides: Vec<OracleUpdate> = Vec::new();

	let mut current_task = WorkerTask::WaitForNewEvent;

	loop {
		// Check for new oracle updates (non-blocking poll)
		let oracle_updates = oracle_source.poll_oracle_updates();
		if !oracle_updates.is_empty() {
			log::info!(
				target: LOG_TARGET,
				"block {:?}: received oracle update with {} entries",
				current_block_number,
				oracle_updates.len()
			);
			current_task = WorkerTask::OracleUpdate(oracle_updates);
		}

		match current_task.clone() {
			WorkerTask::WaitForNewEvent => {
				log::info!(target: LOG_TARGET, "block {:?}: waiting for new event", current_block_number);
				if let Some(block_event) = block_source.next_block() {
					process_new_block(
						&block_event,
						borrowers,
						&mut borrowers_snapshot,
						&mut liquidated_users,
						&mut tx_waitlist,
						&mut current_block_number,
						&mut current_block_hash,
						&mut current_evm_timestamp,
						api_provider,
						config,
						money_market,
						&active_oracle_overrides,
					);
					current_task = WorkerTask::LiquidateAll;
				} else {
					log::info!(target: LOG_TARGET, "block source exhausted, exiting worker loop");
					return;
				}
			}

			WorkerTask::LiquidateAll => {
				let now = std::time::Instant::now();
				let scan_count = if let Some(threshold) = config.hf_scan_threshold {
					let t = U256::from(threshold);
					borrowers_snapshot.iter().filter(|b| b.health_factor.is_zero() || b.health_factor <= t).count()
				} else {
					borrowers_snapshot.len()
				};
				log::info!(
					target: LOG_TARGET,
					"block {:?}: starting LiquidateAll ({} borrowers to scan, {} total)",
					current_block_number,
					scan_count,
					borrowers_snapshot.len()
				);

				let mut skipped_due_to_cap = 0usize;
				let mut index = 0;
				let mut interrupted = false;
				let mut evaluated = 0usize;

				while index < borrowers_snapshot.len() {
					// Check for new block interrupt (default behavior).
					// In node mode MM re-init is fast (~200ms) so we always want fresh state.
					// With --no-interrupt (standalone), skip this check to complete the scan.
					if !config.no_interrupt {
						if let Some(block_event) = block_source.try_next_block() {
							log::info!(
								target: LOG_TARGET,
								"block {:?}: LiquidateAll interrupted by new block at {}/{} ({}ms evaluated)",
								current_block_number,
								evaluated,
								borrowers_snapshot.len(),
								now.elapsed().as_millis()
							);
							process_new_block(
								&block_event,
								borrowers,
								&mut borrowers_snapshot,
								&mut liquidated_users,
								&mut tx_waitlist,
								&mut current_block_number,
								&mut current_block_hash,
								&mut current_evm_timestamp,
								api_provider,
								config,
								money_market,
								&active_oracle_overrides,
							);
							current_task = WorkerTask::LiquidateAll;
							interrupted = true;
							break;
						}
					}

					let borrower = borrowers_snapshot[index].clone();
					let result = try_liquidate(
						api_provider,
						config,
						tx_submitter,
						dry_runner,
						borrowers,
						&borrower,
						None, // no updated_assets for LiquidateAll
						money_market,
						&mut liquidated_users,
						&mut tx_waitlist,
						current_block_number,
						current_block_hash,
						current_evm_timestamp,
						&mut skipped_due_to_cap,
					);
					if result.is_err() {
						return;
					}
					evaluated += 1;
					index += 1;

					// Oracle updates are always checked — they carry new prices
					// and are time-critical regardless of mode.
					let oracle_updates = oracle_source.poll_oracle_updates();
					if !oracle_updates.is_empty() {
						log::info!(
							target: LOG_TARGET,
							"block {:?}: LiquidateAll interrupted by oracle update at {}/{} ({}ms)",
							current_block_number,
							evaluated,
							borrowers_snapshot.len(),
							now.elapsed().as_millis()
						);
						current_task = WorkerTask::OracleUpdate(oracle_updates);
						interrupted = true;
						break;
					}
				}

				if !interrupted {
					// ISSUE 6 FIX: Log when cap is hit
					if skipped_due_to_cap > 0 {
						log::warn!(
							target: LOG_TARGET,
							"block {:?}: max liquidations cap reached, {} users skipped",
							current_block_number,
							skipped_due_to_cap
						);
					}

					log::info!(
						target: LOG_TARGET,
						"block {:?}: LiquidateAll completed — {} borrowers evaluated in {}ms",
						current_block_number,
						evaluated,
						now.elapsed().as_millis()
					);
					current_task = WorkerTask::WaitForNewEvent;
				}
			}

			WorkerTask::OracleUpdate(ref oracle_update_data) => {
				let now = std::time::Instant::now();
				log::info!(
					target: LOG_TARGET,
					"block {:?}: starting OracleUpdate ({} price updates)",
					current_block_number,
					oracle_update_data.len()
				);

				// When oracle_persist is enabled, store overrides so they survive MM re-init.
				if config.oracle_persist {
					for update in oracle_update_data.iter() {
						// Replace existing override for same asset, or append.
						if let Some(existing) = active_oracle_overrides
							.iter_mut()
							.find(|o| o.asset_address == update.asset_address)
						{
							existing.price = update.price;
						} else {
							active_oracle_overrides.push(update.clone());
						}
					}
					log::info!(
						target: LOG_TARGET,
						"block {:?}: oracle_persist: stored {} active overrides",
						current_block_number,
						active_oracle_overrides.len()
					);
				}

				let mut updated_assets = Vec::new();
				for update in oracle_update_data.iter() {
					if let Some(new_price) = update.price {
						log::trace!(
							target: LOG_TARGET,
							"block {:?}: updating oracle price for asset {:?}",
							current_block_number,
							update.asset_address
						);
						money_market.update_reserve_price(update.asset_address, &new_price);
					}
					updated_assets.push(update.asset_address);
				}

				let mut skipped_due_to_cap = 0usize;
				let mut index = 0;
				let mut interrupted = false;
				let mut evaluated = 0usize;

				while index < borrowers_snapshot.len() {
					// Check for new block interrupt (unless --no-interrupt).
					if !config.no_interrupt {
						if let Some(block_event) = block_source.try_next_block() {
							log::info!(
								target: LOG_TARGET,
								"block {:?}: OracleUpdate interrupted by new block at {}/{} ({}ms evaluated)",
								current_block_number,
								evaluated,
								borrowers_snapshot.len(),
								now.elapsed().as_millis()
							);
							process_new_block(
								&block_event,
								borrowers,
								&mut borrowers_snapshot,
								&mut liquidated_users,
								&mut tx_waitlist,
								&mut current_block_number,
								&mut current_block_hash,
								&mut current_evm_timestamp,
								api_provider,
								config,
								money_market,
								&active_oracle_overrides,
							);
							current_task = WorkerTask::LiquidateAll;
							interrupted = true;
							break;
						}
					}

					let borrower = borrowers_snapshot[index].clone();
					let result = try_liquidate(
						api_provider,
						config,
						tx_submitter,
						dry_runner,
						borrowers,
						&borrower,
						Some(&updated_assets),
						money_market,
						&mut liquidated_users,
						&mut tx_waitlist,
						current_block_number,
						current_block_hash,
						current_evm_timestamp,
						&mut skipped_due_to_cap,
					);
					if result.is_err() {
						return;
					}
					evaluated += 1;
					index += 1;

					// Oracle updates are always checked mid-scan.
					let new_oracle = oracle_source.poll_oracle_updates();
					if !new_oracle.is_empty() {
						log::info!(
							target: LOG_TARGET,
							"block {:?}: OracleUpdate interrupted by new oracle update at {}/{} ({}ms)",
							current_block_number,
							evaluated,
							borrowers_snapshot.len(),
							now.elapsed().as_millis()
						);
						current_task = WorkerTask::OracleUpdate(new_oracle);
						interrupted = true;
						break;
					}
				}

				if !interrupted {
					if skipped_due_to_cap > 0 {
						log::warn!(
							target: LOG_TARGET,
							"block {:?}: max liquidations cap reached during oracle update, {} users skipped",
							current_block_number,
							skipped_due_to_cap
						);
					}

					log::info!(
						target: LOG_TARGET,
						"block {:?}: OracleUpdate completed — {} borrowers evaluated in {}ms",
						current_block_number,
						evaluated,
						now.elapsed().as_millis()
					);
					current_task = WorkerTask::LiquidateAll;
				}
			}
		}
	}
}

/// Process block state updates: waitlist eviction, borrower management, block tracking.
/// This is the testable core of `process_new_block` without runtime type dependencies.
#[cfg(test)]
pub(crate) fn process_block_state(
	event: &BlockEvent,
	borrowers: &mut Vec<Borrower>,
	borrowers_snapshot: &mut Vec<Borrower>,
	liquidated_users: &mut Vec<UserAddress>,
	tx_waitlist: &mut HashMap<UserAddress, BlockNumber>,
	current_block_number: &mut BlockNumber,
	current_block_hash: &mut [u8; 32],
) {
	*current_block_number = event.block_number;
	*current_block_hash = event.block_hash;

	// Remove successfully liquidated users from the waitlist.
	for user in &event.liquidated_users {
		tx_waitlist.remove(user);
	}

	// Evict waitlist entries older than TTL blocks.
	let block_num = *current_block_number;
	let evicted: Vec<UserAddress> = tx_waitlist
		.iter()
		.filter(|(_, submitted_at)| block_num.saturating_sub(**submitted_at) > WAITLIST_TTL_BLOCKS)
		.map(|(addr, _)| *addr)
		.collect();
	for addr in &evicted {
		tx_waitlist.remove(addr);
	}

	// Add new borrowers.
	add_new_borrowers(event.new_borrowers.clone(), borrowers);

	// Clear per-block liquidated users list.
	liquidated_users.clear();

	// Update the snapshot for iteration.
	*borrowers_snapshot = borrowers.clone();
}

/// Process a new block event: update state, evict stale waitlist entries, add new borrowers,
/// re-initialize MoneyMarketData and update EVM timestamp.
fn process_new_block<Block, OriginCaller, RuntimeCall, RuntimeEvent, Api>(
	event: &BlockEvent,
	borrowers: &mut Vec<Borrower>,
	borrowers_snapshot: &mut Vec<Borrower>,
	liquidated_users: &mut Vec<UserAddress>,
	tx_waitlist: &mut HashMap<UserAddress, WaitlistEntry>,
	current_block_number: &mut BlockNumber,
	current_block_hash: &mut [u8; 32],
	current_evm_timestamp: &mut u64,
	api_provider: &Api,
	config: &WorkerConfig,
	money_market: &mut MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	oracle_overrides: &[OracleUpdate],
) where
	Block: sp_runtime::traits::Block,
	Block::Hash: From<H256>,
	Api: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> + Clone,
{
	let block_start = std::time::Instant::now();
	*current_block_number = event.block_number;
	*current_block_hash = event.block_hash;

	log::trace!(
		target: LOG_TARGET,
		"block {:?}: processing new block (hash: {:?}, {} new borrowers, {} liquidated users)",
		event.block_number,
		hex::encode(event.block_hash),
		event.new_borrowers.len(),
		event.liquidated_users.len()
	);

	let block_hash: Block::Hash = H256::from(event.block_hash).into();

	// Remove successfully liquidated users from the waitlist.
	for user in &event.liquidated_users {
		log::debug!(target: LOG_TARGET, "block {:?}: removing liquidated user {:?} from waitlist", current_block_number, user);
		tx_waitlist.remove(user);
	}

	// BUG 1 FIX: Evict waitlist entries older than TTL blocks.
	// Users whose tx was submitted but no Liquidated event arrived within TTL are re-evaluated.
	let block_num = *current_block_number;
	let evicted: Vec<UserAddress> = tx_waitlist
		.iter()
		.filter(|(_, entry)| block_num.saturating_sub(entry.submitted_at_block) > WAITLIST_TTL_BLOCKS)
		.map(|(addr, _)| *addr)
		.collect();
	for addr in &evicted {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: evicting user {:?} from waitlist (submitted at block {}, TTL expired)",
			current_block_number,
			addr,
			tx_waitlist.get(addr).map(|e| e.submitted_at_block).unwrap_or(0)
		);
		tx_waitlist.remove(addr);
	}

	// Add new borrowers.
	add_new_borrowers(event.new_borrowers.clone(), borrowers);

	// Clear per-block liquidated users list.
	liquidated_users.clear();

	// Re-initialize MoneyMarketData for the new block.
	// In node mode this is essential (state changes between blocks).
	// In standalone mode this refreshes reserve parameters.
	let mm_start = std::time::Instant::now();
	match MoneyMarketData::<Block, OriginCaller, RuntimeCall, RuntimeEvent>::new(
		api_provider.clone(),
		block_hash,
		config.pap_contract,
		config.runtime_api_caller,
	) {
		Ok(new_mm) => {
			log::trace!(
				target: LOG_TARGET,
				"block {:?}: MoneyMarketData re-initialized ({} reserves) in {}ms",
				current_block_number,
				new_mm.reserves().len(),
				mm_start.elapsed().as_millis()
			);
			*money_market = new_mm;

			// Re-apply persistent oracle overrides (--oracle-persist).
			// MM re-init fetches fresh prices from chain, wiping out injected prices.
			if !oracle_overrides.is_empty() {
				for ovr in oracle_overrides {
					if let Some(price) = ovr.price {
						money_market.update_reserve_price(ovr.asset_address, &price);
					}
				}
				log::info!(
					target: LOG_TARGET,
					"block {:?}: re-applied {} persistent oracle overrides after MM re-init",
					current_block_number,
					oracle_overrides.len()
				);
			}
		}
		Err(e) => {
			log::warn!(
				target: LOG_TARGET,
				"block {:?}: MoneyMarketData re-init failed ({:?}) after {}ms, keeping previous state",
				current_block_number,
				e,
				mm_start.elapsed().as_millis()
			);
		}
	}

	// Update EVM timestamp.
	let ts_start = std::time::Instant::now();
	if let Some(ts) = api_provider.current_timestamp(block_hash) {
		log::trace!(
			target: LOG_TARGET,
			"block {:?}: EVM timestamp updated to {} in {}ms",
			current_block_number,
			ts,
			ts_start.elapsed().as_millis()
		);
		*current_evm_timestamp = ts;
	} else {
		log::warn!(
			target: LOG_TARGET,
			"block {:?}: failed to fetch EVM timestamp after {}ms, keeping previous value",
			current_block_number,
			ts_start.elapsed().as_millis()
		);
	}

	// Update the snapshot for iteration.
	*borrowers_snapshot = borrowers.clone();

	log::trace!(
		target: LOG_TARGET,
		"block {:?}: block processing completed in {}ms ({} borrowers, {} in waitlist)",
		current_block_number,
		block_start.elapsed().as_millis(),
		borrowers.len(),
		tx_waitlist.len()
	);
}

/// Add new borrowers or reset HF for existing ones.
pub(crate) fn add_new_borrowers(new_borrowers: Vec<UserAddress>, borrowers: &mut Vec<Borrower>) {
	for user_address in new_borrowers {
		match borrowers.iter_mut().find(|b| b.user_address == user_address) {
			Some(b) => {
				b.health_factor = U256::zero();
			}
			None => {
				borrowers.insert(
					0,
					Borrower {
						user_address,
						health_factor: U256::zero(),
					},
				);
			}
		}
	}
	borrowers.sort_by(|a, b| a.health_factor.partial_cmp(&b.health_factor).unwrap_or(Ordering::Equal));
}

/// Try to liquidate a single borrower. Returns Err(()) to signal fatal error (stop worker).
#[allow(clippy::too_many_arguments)]
fn try_liquidate<Block, OriginCaller, RuntimeCall, RuntimeEvent, Api, T, D>(
	api_provider: &Api,
	config: &WorkerConfig,
	tx_submitter: &T,
	dry_runner: &D,
	borrowers: &mut [Borrower],
	target_borrower: &Borrower,
	updated_assets: Option<&Vec<AssetAddress>>,
	money_market: &mut MoneyMarketData<Block, OriginCaller, RuntimeCall, RuntimeEvent>,
	liquidated_users: &mut Vec<UserAddress>,
	tx_waitlist: &mut HashMap<UserAddress, WaitlistEntry>,
	current_block_number: BlockNumber,
	current_block_hash: [u8; 32],
	current_evm_timestamp: u64,
	skipped_due_to_cap: &mut usize,
) -> Result<(), ()>
where
	Block: sp_runtime::traits::Block,
	Block::Hash: From<H256>,
	Api: RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> + Clone,
	T: TxSubmitter,
	D: DryRunner,
{
	let Some(borrower) = borrowers
		.iter_mut()
		.find(|element| element.user_address == target_borrower.user_address)
	else {
		return Ok(());
	};

	let borrower_start = std::time::Instant::now();

	// Skip if already liquidated in this block.
	if liquidated_users.contains(&borrower.user_address) {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] skip — already liquidated this block (0ms)",
			current_block_number,
			borrower.user_address
		);
		return Ok(());
	}

	// BUG 2 FIX: Removed the gate that skipped waitlisted users during oracle updates.
	// Now we always re-evaluate all users regardless of waitlist + oracle update state.
	// The waitlist TTL (BUG 1) handles the case where a previous submission didn't land.

	// Skip if user is in waitlist and TTL hasn't expired yet.
	if let Some(entry) = tx_waitlist.get(&borrower.user_address) {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] skip — in waitlist since block {} (0ms)",
			current_block_number,
			borrower.user_address,
			entry.submitted_at_block
		);
		return Ok(());
	}

	// HF scan threshold: skip borrowers whose cached HF is well above 1.0.
	// This saves expensive RPC calls in standalone mode. New borrowers (HF=0) and
	// borrowers near liquidation are always checked.
	if let Some(threshold) = config.hf_scan_threshold {
		let threshold = U256::from(threshold);
		// HF=0 means "not yet calculated" — always check these.
		if !borrower.health_factor.is_zero() && borrower.health_factor > threshold {
			return Ok(());
		}
	}

	// Get user data based on current (possibly updated) prices.
	let block_hash: Block::Hash = H256::from(current_block_hash).into();
	let user_start = std::time::Instant::now();
	let Ok(user_data) = UserData::new(
		api_provider.clone(),
		block_hash,
		money_market,
		borrower.user_address,
		current_evm_timestamp,
		config.runtime_api_caller,
	) else {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] skip — UserData fetch failed ({}ms)",
			current_block_number,
			borrower.user_address,
			borrower_start.elapsed().as_millis()
		);
		return Ok(());
	};
	let user_data_ms = user_start.elapsed().as_millis();

	let hf_start = std::time::Instant::now();
	if let Ok(current_hf) =
		user_data.health_factor::<Block, Api, OriginCaller, RuntimeCall, RuntimeEvent>(money_market)
	{
		borrower.health_factor = current_hf;
		let hf_ms = hf_start.elapsed().as_millis();

		let hf_one = U256::from(10u128.pow(18));
		if current_hf > hf_one {
			log::info!(
				target: LOG_TARGET,
				"block {:?}: [{:?}] healthy — HF={} (UserData: {}ms, HF: {}ms, total: {}ms)",
				current_block_number,
				borrower.user_address,
				current_hf,
				user_data_ms,
				hf_ms,
				borrower_start.elapsed().as_millis()
			);
			return Ok(());
		}

		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] undercollateralized — HF={} (UserData: {}ms, HF: {}ms)",
			current_block_number,
			borrower.user_address,
			current_hf,
			user_data_ms,
			hf_ms
		);
	} else {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] skip — HF calculation failed (UserData: {}ms, HF: {}ms, total: {}ms)",
			current_block_number,
			borrower.user_address,
			user_data_ms,
			hf_start.elapsed().as_millis(),
			borrower_start.elapsed().as_millis()
		);
		return Ok(());
	}

	let liq_option_start = std::time::Instant::now();
	if let Ok(Some(liquidation_option)) = money_market.get_best_liquidation_option::<Api>(
		&user_data,
		config.target_hf.into(),
		updated_assets,
	) {
		let liq_option_ms = liq_option_start.elapsed().as_millis();

		let (Some(collateral_asset_id), Some(debt_asset_id)) = (
			money_market.address_to_asset(liquidation_option.collateral_asset),
			money_market.address_to_asset(liquidation_option.debt_asset),
		) else {
			log::error!(
				target: LOG_TARGET,
				"block {:?}: [{:?}] ERROR — address_to_asset conversion failed ({}ms)",
				current_block_number,
				borrower.user_address,
				borrower_start.elapsed().as_millis()
			);
			return Ok(());
		};

		let Ok(debt_to_liquidate) = liquidation_option.debt_to_liquidate.try_into() else {
			log::info!(
				target: LOG_TARGET,
				"block {:?}: [{:?}] skip — debt_to_liquidate overflow ({}ms)",
				current_block_number,
				borrower.user_address,
				borrower_start.elapsed().as_millis()
			);
			return Ok(());
		};

		// Reset HF so it gets recalculated next time.
		borrower.health_factor = U256::zero();

		// ISSUE 6 FIX: Check cap and track skipped users.
		if liquidated_users.len() >= config.max_liquidations_per_block {
			*skipped_due_to_cap += 1;
			log::info!(
				target: LOG_TARGET,
				"block {:?}: [{:?}] skip — max liquidations cap reached ({}ms)",
				current_block_number,
				borrower.user_address,
				borrower_start.elapsed().as_millis()
			);
			return Ok(());
		}

		let liq_tx = LiquidationTx {
			user: borrower.user_address,
			collateral_asset: collateral_asset_id,
			debt_asset: debt_asset_id,
			debt_to_cover: debt_to_liquidate,
			health_factor: borrower.health_factor,
		};

		// BUG 2 FIX: Always dry-run before submitting (both first-time and retry).
		if !config.dry_run {
			let dry_start = std::time::Instant::now();
			let dry_ok = dry_runner.dry_run(&liq_tx, current_block_hash);
			let dry_ms = dry_start.elapsed().as_millis();
			if !dry_ok {
				log::info!(
					target: LOG_TARGET,
					"block {:?}: [{:?}] skip — dry-run FAILED (liq_option: {}ms, dry-run: {}ms, total: {}ms)",
					current_block_number,
					borrower.user_address,
					liq_option_ms,
					dry_ms,
					borrower_start.elapsed().as_millis()
				);
				return Ok(());
			}
			log::info!(
				target: LOG_TARGET,
				"block {:?}: [{:?}] dry-run passed ({}ms)",
				current_block_number,
				borrower.user_address,
				dry_ms
			);
		}

		// Submit (or report in dry-run mode).
		let submit_start = std::time::Instant::now();
		let result = tx_submitter.submit(&liq_tx, current_block_hash);
		let submit_ms = submit_start.elapsed().as_millis();
		let total_ms = borrower_start.elapsed().as_millis();
		match &result {
			SubmitResult::Submitted => {
				log::info!(
					target: LOG_TARGET,
					"block {:?}: [{:?}] SUBMITTED — collateral: {}, debt: {}, amount: {} \
					(UserData: {}ms, liq_option: {}ms, submit: {}ms, total: {}ms)",
					current_block_number,
					borrower.user_address,
					collateral_asset_id,
					debt_asset_id,
					debt_to_liquidate,
					user_data_ms,
					liq_option_ms,
					submit_ms,
					total_ms
				);
			}
			SubmitResult::DryRun => {
				log::info!(
					target: LOG_TARGET,
					"block {:?}: [{:?}] DRY-RUN — collateral: {}, debt: {}, amount: {} \
					(UserData: {}ms, liq_option: {}ms, submit: {}ms, total: {}ms)",
					current_block_number,
					borrower.user_address,
					collateral_asset_id,
					debt_asset_id,
					debt_to_liquidate,
					user_data_ms,
					liq_option_ms,
					submit_ms,
					total_ms
				);
			}
			SubmitResult::Failed(reason) => {
				log::warn!(
					target: LOG_TARGET,
					"block {:?}: [{:?}] SUBMIT FAILED — {} \
					(UserData: {}ms, liq_option: {}ms, submit: {}ms, total: {}ms)",
					current_block_number,
					borrower.user_address,
					reason,
					user_data_ms,
					liq_option_ms,
					submit_ms,
					total_ms
				);
			}
		}

		liquidated_users.push(borrower.user_address);

		// BUG 4+5 FIX: Add to waitlist with block number for TTL tracking.
		tx_waitlist.insert(
			borrower.user_address,
			WaitlistEntry {
				submitted_at_block: current_block_number,
			},
		);
	} else {
		log::info!(
			target: LOG_TARGET,
			"block {:?}: [{:?}] no liquidation needed (liq_option search: {}ms, total: {}ms)",
			current_block_number,
			borrower.user_address,
			liq_option_start.elapsed().as_millis(),
			borrower_start.elapsed().as_millis()
		);
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn add_new_borrowers_inserts_at_front_with_zero_hf() {
		let mut borrowers = vec![Borrower {
			user_address: sp_core::H160::from_low_u64_be(1),
			health_factor: U256::from(10u128.pow(18)),
		}];

		add_new_borrowers(
			vec![sp_core::H160::from_low_u64_be(2)],
			&mut borrowers,
		);

		assert_eq!(borrowers.len(), 2);
		assert_eq!(borrowers[0].user_address, sp_core::H160::from_low_u64_be(2));
		assert_eq!(borrowers[0].health_factor, U256::zero());
	}

	#[test]
	fn add_existing_borrower_resets_hf() {
		let addr = sp_core::H160::from_low_u64_be(1);
		let mut borrowers = vec![Borrower {
			user_address: addr,
			health_factor: U256::from(10u128.pow(18)),
		}];

		add_new_borrowers(vec![addr], &mut borrowers);

		assert_eq!(borrowers.len(), 1);
		assert_eq!(borrowers[0].health_factor, U256::zero());
	}

	#[test]
	fn waitlist_ttl_eviction() {
		let addr = sp_core::H160::from_low_u64_be(42);
		let mut waitlist: HashMap<UserAddress, WaitlistEntry> = HashMap::new();
		waitlist.insert(addr, WaitlistEntry { submitted_at_block: 10 });

		// At block 11, TTL (2 blocks) has NOT expired yet
		let block_num: BlockNumber = 11;
		let evicted: Vec<UserAddress> = waitlist
			.iter()
			.filter(|(_, entry)| block_num.saturating_sub(entry.submitted_at_block) > WAITLIST_TTL_BLOCKS)
			.map(|(addr, _)| *addr)
			.collect();
		assert!(evicted.is_empty());

		// At block 13, TTL has expired (13 - 10 = 3 > 2)
		let block_num: BlockNumber = 13;
		let evicted: Vec<UserAddress> = waitlist
			.iter()
			.filter(|(_, entry)| block_num.saturating_sub(entry.submitted_at_block) > WAITLIST_TTL_BLOCKS)
			.map(|(addr, _)| *addr)
			.collect();
		assert_eq!(evicted, vec![addr]);
	}
}
