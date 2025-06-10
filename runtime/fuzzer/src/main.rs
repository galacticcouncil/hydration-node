/// hydration fuzzer v2.2.2
/// Inspired by the harness sent to HydraDX from srlabs.de on 01.11.2023
use codec::{DecodeLimit, Encode};
#[cfg(all(not(feature = "deprecated-substrate"), feature = "try-runtime"))]
#[allow(unused_imports)]
use frame_support::traits::{TryState, TryStateSelect};
#[cfg(not(feature = "deprecated-substrate"))]
use frame_support::{
	dispatch::GetDispatchInfo, pallet_prelude::Weight, traits::IntegrityTest,
	weights::constants::WEIGHT_REF_TIME_PER_SECOND,
};
use hydradx_runtime::*;
use primitives::constants::time::SLOT_DURATION;
use runtime_mock::hydradx_mocked_runtime;
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_runtime::{
	traits::{Dispatchable, Header},
	Digest, DigestItem,
};
use std::path::PathBuf;
#[cfg(feature = "deprecated-substrate")]
use {frame_support::weights::constants::WEIGHT_PER_SECOND as WEIGHT_REF_TIME_PER_SECOND, sp_runtime::traits::Zero};

/// Types from the fuzzed runtime.
type FuzzedRuntime = hydradx_runtime::Runtime;

type Balance = <FuzzedRuntime as pallet_balances::Config>::Balance;
#[cfg(feature = "deprecated-substrate")]
type RuntimeOrigin = <FuzzedRuntime as frame_system::Config>::Origin;
#[cfg(not(feature = "deprecated-substrate"))]
type RuntimeOrigin = <FuzzedRuntime as frame_system::Config>::RuntimeOrigin;
type AccountId = <FuzzedRuntime as frame_system::Config>::AccountId;

/// The maximum number of blocks per fuzzer input.
/// If set to 0, then there is no limit at all.
/// Feel free to set this to a low number (e.g. 4) when you begin your fuzzing campaign and then set
/// it back to 32 once you have good coverage.
const MAX_BLOCKS_PER_INPUT: usize = 32;

/// The maximum number of extrinsics per block.
/// If set to 0, then there is no limit at all.
/// Feel free to set this to a low number (e.g. 8) when you begin your fuzzing campaign and then set
/// it back to 0 once you have good coverage.
const MAX_EXTRINSICS_PER_BLOCK: usize = 0;

/// Max number of seconds a block should run for.
#[cfg(not(fuzzing))]
const MAX_TIME_FOR_BLOCK: u64 = 6;

// We do not skip more than DEFAULT_STORAGE_PERIOD to avoid pallet_transaction_storage from
// panicking on finalize.
// Set to number of blocks in two months
const MAX_BLOCK_LAPSE: u32 = 864_000;

// Extrinsic delimiter: `********`
const DELIMITER: [u8; 8] = [42; 8];

/// Constants for the fee-memory mapping
#[cfg(not(fuzzing))]
const FILENAME_MEMORY_MAP: &str = "memory_map.output";

const SNAPSHOT_PATH: &str = "data/MOCK_SNAPSHOT";

// We won't analyse those native Substrate pallets
#[cfg(not(fuzzing))]
const BLACKLISTED_CALLS: [&str; 9] = [
	"RuntimeCall::System",
	"RuntimeCall::Utility",
	"RuntimeCall::Proxy",
	"RuntimeCall::Uniques",
	"RuntimeCall::Balances",
	"RuntimeCall::Timestamp",
	// to prevent false negatives from debug_assert_ne
	"RuntimeCall::XTokens",
	"RuntimeCall::Council",
	"RuntimeCall::Referenda",
];

struct Data<'a> {
	data: &'a [u8],
	pointer: usize,
	size: usize,
}

#[allow(clippy::absurd_extreme_comparisons)]
impl<'a> Data<'a> {
	fn size_limit_reached(&self) -> bool {
		!(MAX_BLOCKS_PER_INPUT == 0 || MAX_EXTRINSICS_PER_BLOCK == 0)
			&& self.size >= MAX_BLOCKS_PER_INPUT * MAX_EXTRINSICS_PER_BLOCK
	}
}

impl<'a> Iterator for Data<'a> {
	type Item = &'a [u8];

	fn next(&mut self) -> Option<Self::Item> {
		if self.data.len() <= self.pointer || self.size_limit_reached() {
			return None;
		}
		let next_delimiter = self.data[self.pointer..]
			.windows(DELIMITER.len())
			.position(|window| window == DELIMITER);
		let next_pointer = match next_delimiter {
			Some(delimiter) => self.pointer + delimiter,
			None => self.data.len(),
		};
		let res = Some(&self.data[self.pointer..next_pointer]);
		self.pointer = next_pointer + DELIMITER.len();
		self.size += 1;
		res
	}
}

fn recursively_find_call(call: RuntimeCall, matches_on: fn(RuntimeCall) -> bool) -> bool {
	if let RuntimeCall::Utility(
		pallet_utility::Call::batch { calls }
		| pallet_utility::Call::force_batch { calls }
		| pallet_utility::Call::batch_all { calls },
	) = call
	{
		for call in calls {
			if recursively_find_call(call.clone(), matches_on) {
				return true;
			}
		}
	} else if let RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. })
	| RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. })
	| RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. })
	| RuntimeCall::Council(pallet_collective::Call::propose { proposal: call, .. }) = call
	{
		return recursively_find_call(*call.clone(), matches_on);
	} else if matches_on(call) {
		return true;
	}
	false
}
fn try_specific_extrinsic(identifier: u8, data: &[u8], assets: &[u32]) -> Option<RuntimeCall> {
	for handler in extrinsics_handlers() {
		if let Some(call) = handler.try_extrinsic(identifier, data, assets) {
			return Some(call);
		}
	}
	None
}

fn main() {
	// We ensure that on each run, the mapping is a fresh one
	#[cfg(not(any(fuzzing, coverage)))]
	if std::fs::remove_file(FILENAME_MEMORY_MAP).is_err() {
		// println!("Can't remove the map file, but it's not a problem.");
	}

	// Create SNAPSHOT from runtime_mock state
	let mocked_externalities = hydradx_mocked_runtime();
	let snapshot_path = PathBuf::from(SNAPSHOT_PATH);
	scraper::save_externalities::<hydradx_runtime::Block>(mocked_externalities, snapshot_path).unwrap();

	// List of accounts to choose as origin
	let accounts: Vec<AccountId> = (0..20).map(|i| [i; 32].into()).collect();

	ziggy::fuzz!(|data: &[u8]| {
		let iteratable = Data {
			data,
			pointer: 0,
			size: 0,
		};

		// Max weight for a block.
		#[cfg(feature = "deprecated-substrate")]
		let max_weight: Weight = WEIGHT_REF_TIME_PER_SECOND * 2;
		#[cfg(not(feature = "deprecated-substrate"))]
		let max_weight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND * 2, 0);

		let mut block_count = 0;
		let mut extrinsics_in_block = 0;

		// We need to load snapshot first to obtain list of registereted assets
		// This might be a bit unnecessary, especially if there is not valid extrinsic in the input
		// TODO: consider reordering the code to avoid this and retrieve list of assets another way

		// `externalities` represents the state of our mock chain.
		let snapshot_path = PathBuf::from(SNAPSHOT_PATH);
		let mut externalities;
		if let Ok(snapshot) = scraper::load_snapshot::<Block>(snapshot_path) {
			externalities = snapshot;
		} else {
			externalities = hydradx_mocked_runtime();
		}

		// load AssetIds
		let mut assets: Vec<u32> = Vec::new();
		externalities.execute_with(|| {
			// lets assert that the mock is correctly setup, just in case
			let asset_ids = pallet_asset_registry::Assets::<FuzzedRuntime>::iter_keys();
			for asset_id in asset_ids {
				assets.push(asset_id);
			}
		});

		let extrinsics: Vec<(Option<u32>, usize, RuntimeCall)> = iteratable
			.filter_map(|data| {
				// We have reached the limit of block we want to decode
				#[allow(clippy::absurd_extreme_comparisons)]
				if MAX_BLOCKS_PER_INPUT != 0 && block_count >= MAX_BLOCKS_PER_INPUT {
					return None;
				}
				// Min lengths required for the data
				// - lapse is u32 (4 bytes),
				// - origin is u16 (2 bytes)
				// - structured fuzzer (1 byte)
				// -> 7 bytes minimum
				let min_data_len = 4 + 2 + 1;
				if data.len() <= min_data_len {
					return None;
				}
				let lapse: u32 = u32::from_ne_bytes(data[0..4].try_into().unwrap());
				let origin: usize = u16::from_ne_bytes(data[4..6].try_into().unwrap()) as usize;
				let specific_extrinsic: u8 = data[6];
				let mut encoded_extrinsic: &[u8] = &data[7..];

				// If the lapse is in the range [1, MAX_BLOCK_LAPSE] it is valid.
				let maybe_lapse = match lapse {
					1..=MAX_BLOCK_LAPSE => Some(lapse),
					_ => None,
				};
				// We have reached the limit of extrinsics for this block
				#[allow(clippy::absurd_extreme_comparisons)]
				if maybe_lapse.is_none()
					&& MAX_EXTRINSICS_PER_BLOCK != 0
					&& extrinsics_in_block >= MAX_EXTRINSICS_PER_BLOCK
				{
					return None;
				}

				let maybe_extrinsic =
					if let Some(extrinsic) = try_specific_extrinsic(specific_extrinsic, encoded_extrinsic, &assets) {
						Ok(extrinsic)
					} else {
						DecodeLimit::decode_all_with_depth_limit(32, &mut encoded_extrinsic)
					};

				if let Ok(decoded_extrinsic) = maybe_extrinsic {
					if maybe_lapse.is_some() {
						block_count += 1;
						extrinsics_in_block = 1;
					} else {
						extrinsics_in_block += 1;
					}
					// We have reached the limit of block we want to decode
					if MAX_BLOCKS_PER_INPUT != 0 && block_count >= MAX_BLOCKS_PER_INPUT {
						return None;
					}

					Some((maybe_lapse, origin, decoded_extrinsic))
				} else {
					None
				}
			})
			.collect();

		if extrinsics.is_empty() {
			return;
		}

		let mut current_block: u32 = 1;
		let mut current_timestamp: u64 = SLOT_DURATION;
		let mut current_weight: Weight = Weight::zero();

		let start_block = |block: u32, current_timestamp: u64| {
			#[cfg(not(fuzzing))]
			println!("Initializing block {block}");

			let pre_digest = Digest {
				logs: vec![DigestItem::PreRuntime(
					AURA_ENGINE_ID,
					Slot::from(block as u64).encode(),
				)],
			};

			Executive::initialize_block(&Header::new(
				block,
				Default::default(),
				Default::default(),
				Default::default(),
				pre_digest,
			));

			#[cfg(not(fuzzing))]
			println!("Setting Timestamp");
			// We apply the timestamp extrinsic for the current block.
			Executive::apply_extrinsic(UncheckedExtrinsic::new_unsigned(RuntimeCall::Timestamp(
				pallet_timestamp::Call::set { now: current_timestamp },
			)))
			.unwrap()
			.unwrap();

			println!("Parachain validation data");

			let parachain_validation_data = {
				use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;

				let parent_head_data = {
					let header = cumulus_primitives_core::relay_chain::Header::new(
						block,
						sp_core::H256::from_low_u64_be(0),
						sp_core::H256::from_low_u64_be(0),
						Default::default(),
						Default::default(),
					);
					cumulus_primitives_core::relay_chain::HeadData(header.encode())
				};

				let mut sproof_builder = RelayStateSproofBuilder::default();

				sproof_builder.para_id = hydradx_runtime::ParachainInfo::get();
				sproof_builder.included_para_head = Some(parent_head_data.clone());

				let (relay_storage_root, proof) = sproof_builder.into_state_root_and_proof();

				cumulus_pallet_parachain_system::Call::set_validation_data {
					data: cumulus_primitives_parachain_inherent::ParachainInherentData {
						validation_data: cumulus_primitives_core::PersistedValidationData {
							parent_head: Default::default(),
							relay_parent_number: block,
							relay_parent_storage_root: relay_storage_root,
							max_pov_size: Default::default(),
						},
						relay_chain_state: proof,
						downward_messages: Default::default(),
						horizontal_messages: Default::default(),
					},
				}
			};

			println!("Setting new validation data");

			Executive::apply_extrinsic(UncheckedExtrinsic::new_unsigned(RuntimeCall::ParachainSystem(
				parachain_validation_data,
			)))
			.unwrap()
			.unwrap();

			println!("Setting new validation data done");

			// Calls that need to be executed before each block starts (init_calls) go here
		};

		let end_block = |_block: u32| {
			#[cfg(not(fuzzing))]
			println!("Finalizing block {_block}");
			Executive::finalize_block();

			#[cfg(not(fuzzing))]
			println!("Testing invariants for block {_block}");

			<AllPalletsWithSystem as TryState<BlockNumber>>::try_state(_block, TryStateSelect::All).unwrap();
		};

		#[cfg(not(any(fuzzing, coverage)))]
		let mut mapper = MemoryMapper::new();

		externalities.execute_with(|| {
			// lets assert that the mock is correctly setup, just in case
			let omnipool_asset = pallet_omnipool::Pallet::<FuzzedRuntime>::assets(&0);
			assert!(omnipool_asset.is_some());
		});

		externalities.execute_with(|| start_block(current_block, current_timestamp));

		// Calls that need to be executed in the first block go here
		for (maybe_lapse, origin, extrinsic) in extrinsics {
			if recursively_find_call(extrinsic.clone(), |call| matches!(&call, RuntimeCall::XTokens(..))) {
				#[cfg(not(fuzzing))]
				println!("    Skipping because of custom filter");
				continue;
			}
			// If the lapse is in the range [0, MAX_BLOCK_LAPSE] we finalize the block and
			// initialize a new one.
			if let Some(lapse) = maybe_lapse {
				// We end the current block
				externalities.execute_with(|| end_block(current_block));

				// We update our state variables
				current_block += lapse;
				current_timestamp += lapse as u64 * SLOT_DURATION;
				current_weight = Weight::zero();

				// We start the next block
				externalities.execute_with(|| start_block(current_block, current_timestamp));
			}

			// We get the current time for timing purposes.
			#[cfg(not(fuzzing))]
			println!("  call:       {:?}", extrinsic);

			let mut call_weight = Weight::zero();
			// We compute the weight to avoid overweight blocks.
			externalities.execute_with(|| {
				call_weight = extrinsic.get_dispatch_info().weight;
			});

			current_weight = current_weight.saturating_add(call_weight);
			#[cfg(feature = "deprecated-substrate")]
			if current_weight >= max_weight {
				#[cfg(not(fuzzing))]
				println!("Skipping because of max weight {}", max_weight);
				continue;
			}

			#[cfg(not(feature = "deprecated-substrate"))]
			if current_weight.ref_time() >= max_weight.ref_time() {
				#[cfg(not(fuzzing))]
				println!("Skipping because of max weight {}", max_weight);
				continue;
			}

			externalities.execute_with(|| {
				// We use given list of accounts to choose from, not a random account from the system
				let origin_account = accounts[origin % accounts.len()].clone();

				#[cfg(not(any(fuzzing, coverage)))]
				mapper.initialize_extrinsic(origin_account.clone(), format!("{:?}", extrinsic));

				let _res = extrinsic
					.clone()
					.dispatch(RuntimeOrigin::signed(origin_account.clone()));

				#[cfg(not(fuzzing))]
				println!("    result:     {:?}", &_res);

				#[cfg(not(any(fuzzing, coverage)))]
				mapper.finalize_extrinsic(_res, extrinsic, origin_account);

				#[cfg(not(any(fuzzing, coverage)))]
				{
					let elapsed = Duration::from_nanos(mapper.get_elapsed().try_into().unwrap()).as_secs();
					if elapsed > MAX_TIME_FOR_BLOCK {
						panic!("block execution took too much time - {}", elapsed)
					}
				}
			});
		}

		// We end the final block
		externalities.execute_with(|| end_block(current_block));

		// After execution of all blocks.
		externalities.execute_with(|| {
			// Check that the consumer/provider state is valid.
			for acc in frame_system::Account::<FuzzedRuntime>::iter() {
				let acc_consumers = acc.1.consumers;
				let acc_providers = acc.1.providers;
				if acc_consumers > 0 && acc_providers == 0 {
					panic!("Invalid state");
				}
			}

			#[cfg(not(fuzzing))]
			println!("Running integrity tests\n");
			// We run all developer-defined integrity tests
			<AllPalletsWithSystem as IntegrityTest>::integrity_test();
		});

		// Exporting the map
		#[cfg(not(any(fuzzing, coverage)))]
		{
			let helper = MapHelper { mapper };
			helper.save();
		}
	});
}

use frame_support::pallet_prelude::Get;
#[cfg(not(any(fuzzing, coverage)))]
use frame_support::{dispatch::DispatchResultWithPostInfo, traits::Currency};
#[cfg(not(any(fuzzing, coverage)))]
use stats_alloc::{StatsAlloc, INSTRUMENTED_SYSTEM};
#[cfg(not(any(fuzzing, coverage)))]
use std::{
	alloc::System,
	collections::HashMap,
	fmt::{self, Display, Formatter},
	fs::OpenOptions,
	io::prelude::*,
	ops::Add,
	time::{Duration, Instant},
};

/// A type to represent a big integer. This is mainly used to avoid overflow
#[cfg(not(any(fuzzing, coverage)))]
type DeltaSize = i128;

/// Represents the different statistics that will be captured during the analysis
///
/// # Fields
/// - `fee`: Fees used to execute the extrinsic
/// - `balance_delta`: The difference of balance before and after executing an extrinsic
/// - `reserve_delta`: The difference of the reserved balance while executing an extrinsic
/// - `lock_delta`: The difference of the locked balance before and after executing an extrinsic
/// - `memory_delta`: Memory used to execute a specific extrinsic, based on the allocator stats
/// - `elapsed`: Time spent to execute the extrinsic
#[cfg(not(any(fuzzing, coverage)))]
#[derive(Copy, Clone, Debug)]
pub struct MappingData {
	fee: Balance,
	balance_delta: DeltaSize,
	reserve_delta: DeltaSize,
	lock_delta: DeltaSize,
	memory_delta: DeltaSize,
	elapsed: u128,
}

/// This struct is used to record important information about the memory allocator, timer,
/// and balance before processing an extrinsic **BEFORE** the executing of the extrinsic. It will
/// be used to calculate the deltas in a later stage.
///
/// # Fields
/// - `balance_before`: A struct holding information about weights, fees, and size before the
///   extrinsic execution.
/// - `reserved_before`: A struct holding information about reserved memory before the extrinsic
///   execution.
/// - `locked_before`: A struct holding information about locked memory before the extrinsic
///   execution.
/// - `allocated_before`: A struct holding information about allocated memory before the extrinsic
///   execution.
/// - `deallocated_before`: A struct holding information about deallocated memory before the
///   extrinsic execution.
/// - `timer`: An optional `Instant` capturing the time before the extrinsic execution starts.
#[cfg(not(any(fuzzing, coverage)))]
pub struct ExtrinsicInfoSnapshot {
	balance_before: DeltaSize,
	reserved_before: DeltaSize,
	locked_before: DeltaSize,
	allocated_before: DeltaSize,
	deallocated_before: DeltaSize,
	timer: Option<Instant>,
}

/// `MemoryMapper` is responsible for mapping different statistics captured during the analysis
/// of extrinsics' execution. It holds data such as fees, balance deltas, memory usage, and elapsed
/// time for each extrinsic. The `MemoryMapper` works in conjunction with `ExtrinsicInfoSnapshot`
/// to record important information about the memory allocator, timer, and balance before
/// processing an extrinsic.
///
/// # Fields
/// - `map`: The map between an extrinsic' string and its associated statistics
/// - `snapshot`: Backup of statistics used to calculate deltas
/// - `extrinsic_name`. Full name of the executed extrinsic with its parameters and origins
/// - `allocator`. Struct pointing to the memory allocator
#[cfg(not(any(fuzzing, coverage)))]
pub struct MemoryMapper<'a> {
	map: HashMap<String, MappingData>,
	snapshot: ExtrinsicInfoSnapshot,
	extrinsic_name: String,
	allocator: Option<&'a StatsAlloc<System>>,
}

/// `MapHelper` is a utility struct that simplifies the management of a memory map, providing
/// features such as `save`. It works in conjunction with `MemoryMapper`, providing an easier way to
/// interact with the data stored in the `MemoryMapper` instance.
///
/// # Fields
/// - `mapper`: Reference to the `MemoryMapper` instance for which `MapHelper` acts as a helper
#[cfg(not(any(fuzzing, coverage)))]
pub struct MapHelper<'a> {
	mapper: MemoryMapper<'a>,
}

#[cfg(not(any(fuzzing, coverage)))]
impl Display for MappingData {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		writeln!(
			f,
			";{};{};{};{};{};{}\n",
			self.fee, self.balance_delta, self.reserve_delta, self.lock_delta, self.memory_delta, self.elapsed
		)
	}
}

#[cfg(not(any(fuzzing, coverage)))]
impl MemoryMapper<'_> {
	fn new() -> Self {
		MemoryMapper {
			map: HashMap::new(),
			snapshot: ExtrinsicInfoSnapshot {
				balance_before: 0,
				reserved_before: 0,
				locked_before: 0,
				allocated_before: 0,
				deallocated_before: 0,
				timer: None,
			},
			allocator: None,
			extrinsic_name: String::new(),
		}
	}

	fn get_elapsed(&self) -> u128 {
		self.map.get(&self.extrinsic_name).map_or(0, |data| data.elapsed)
	}

	fn initialize_extrinsic(&mut self, origin: AccountId, extrinsic_name: String) {
		//TODO: Use the default WASM allocator instead of the default one
		#[global_allocator]
		static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

		self.allocator = Some(GLOBAL);
		self.extrinsic_name = extrinsic_name;
		self.snapshot.deallocated_before = GLOBAL.stats().bytes_deallocated as DeltaSize;
		self.snapshot.allocated_before = GLOBAL.stats().bytes_allocated as DeltaSize;
		self.snapshot.locked_before = <pallet_balances::Pallet<Runtime>>::locks(&origin)
			.iter()
			.map(|lock| lock.amount as DeltaSize)
			.sum();
		self.snapshot.balance_before = <pallet_balances::Pallet<Runtime>>::total_balance(&origin)
			.try_into()
			.unwrap();
		self.snapshot.reserved_before = <pallet_balances::Pallet<Runtime>>::reserved_balance(&origin)
			.try_into()
			.unwrap();

		self.snapshot.timer = Some(Instant::now());

		println!("  origin:     {:?}", origin);
	}

	fn finalize_extrinsic(&mut self, res: DispatchResultWithPostInfo, extrinsic: RuntimeCall, origin: AccountId) {
		if res.is_err() {
			return;
		}

		let refreshed_alloc = self.allocator.expect("Allocator should be set at that point").stats();

		let memory_delta: DeltaSize = (refreshed_alloc.bytes_allocated as DeltaSize - self.snapshot.allocated_before)
			- (refreshed_alloc.bytes_deallocated as DeltaSize - self.snapshot.deallocated_before);

		let elapsed = self
			.snapshot
			.timer
			.expect("Timer should be set at that point")
			.elapsed()
			.as_nanos();

		println!("    memory:     {:?}", memory_delta);

		let balance_after: DeltaSize = <pallet_balances::Pallet<Runtime>>::total_balance(&origin)
			.try_into()
			.unwrap();

		let locked_after: DeltaSize = <pallet_balances::Pallet<Runtime>>::locks(&origin)
			.iter()
			.map(|lock| lock.amount as DeltaSize)
			.sum();

		let reserved_after: DeltaSize = <pallet_balances::Pallet<Runtime>>::reserved_balance(&origin)
			.try_into()
			.unwrap();

		let extrinsic_name = format!("{:?}", extrinsic);

		let fee: Balance = pallet_transaction_payment::Pallet::<Runtime>::compute_actual_fee(
			extrinsic_name.len() as u32, // TODO: Should use `get_encoded_size()`
			&extrinsic.get_dispatch_info(),
			&res.unwrap(),
			0,
		);

		// We allow using basic math operators instead of saturated_sub() for example
		// We assume that an overflow would be an invariant, and a panic would be needed
		let balance_delta = self.snapshot.balance_before - balance_after;
		let reserve_delta = self.snapshot.reserved_before - reserved_after;
		let lock_delta = self.snapshot.locked_before - locked_after;

		// Analyzing the extrinsic only if it passes.
		if res.is_err() {
			// If the extrinsic is an `Err()` but still has a not null `balance_delta`, `reserve_delta`, or `lock_delta` values, we panic!
			if balance_delta != 0 || reserve_delta != 0 || lock_delta != 0 {
				panic!(
					"Invariant panic! One of those values are not zero as it should be. \
                It should not happen since the extrinsic returned an Err(). \
                Debug values: balance_delta: {}, reserve_delta: {}, lock_delta: {}",
					balance_delta, reserve_delta, lock_delta
				);
			}
			return;
		}

		let map: MappingData = MappingData {
			fee,
			balance_delta,
			reserve_delta,
			lock_delta,
			memory_delta,
			elapsed,
		};

		self.map.insert(extrinsic_name, map);
	}
}

#[cfg(not(any(fuzzing, coverage)))]
impl MapHelper<'_> {
	fn save(&self) {
		let inner_save = || -> std::io::Result<()> {
			let mut map_file = OpenOptions::new().create(true).append(true).open(FILENAME_MEMORY_MAP)?;
			// Skip writing if extrinsic_name contains any blocklisted calls
			for (extrinsic_name, extrinsic_infos) in self.mapper.map.iter() {
				if BLACKLISTED_CALLS.iter().any(|&call| extrinsic_name.contains(call)) {
					continue;
				}
				let _ = map_file.write(&extrinsic_name.clone().add(&extrinsic_infos.to_string()).into_bytes())?;
			}
			Ok(())
		};

		if let Err(_err) = inner_save() {
			eprintln!("Failed to save {} ({:?})", &FILENAME_MEMORY_MAP, _err);
		} else {
			println!(
				"Map saved in {}.\nYou can now run `cargo stardust memory`",
				&FILENAME_MEMORY_MAP
			);
		}
	}
}

pub trait TryExtrinsic<Call, AssetId> {
	fn try_extrinsic(&self, identifier: u8, data: &[u8], assets: &[AssetId]) -> Option<Call>;
}

pub fn extrinsics_handlers() -> Vec<Box<dyn TryExtrinsic<RuntimeCall, u32>>> {
	vec![Box::new(OmnipoolHandler {}), Box::new(StableswapHandler {})]
}

pub struct OmnipoolHandler;

impl TryExtrinsic<RuntimeCall, u32> for OmnipoolHandler {
	fn try_extrinsic(&self, identifier: u8, data: &[u8], assets: &[u32]) -> Option<RuntimeCall> {
		match identifier {
			0 if data.len() > 18 => {
				let asset_in = assets[data[0] as usize % assets.len()];
				let asset_out = assets[data[1] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[2..18].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::sell {
					asset_in,
					asset_out,
					amount,
					min_buy_amount: 0,
				}))
			}
			1 if data.len() > 18 => {
				let asset_in = assets[data[0] as usize % assets.len()];
				let asset_out = assets[data[1] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[2..18].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::buy {
					asset_in,
					asset_out,
					amount,
					max_sell_amount: u128::MAX,
				}))
			}
			2 if data.len() > 17 => {
				let asset = assets[data[0] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[1..17].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::add_liquidity {
					asset,
					amount,
				}))
			}
			_ => None,
		}
	}
}

pub struct StableswapHandler;

impl TryExtrinsic<RuntimeCall, u32> for StableswapHandler {
	fn try_extrinsic(&self, identifier: u8, data: &[u8], assets: &[u32]) -> Option<RuntimeCall> {
		match identifier {
			10 if data.len() > 19 => {
				let pool_id = 100 + data[0] as u32 % 3; //TODO: make as parameter, currently ids of pools are 100,101,102
				let asset_in = assets[data[1] as usize % assets.len()];
				let asset_out = assets[data[2] as usize % assets.len()];
				let amount_in = u128::from_ne_bytes(data[3..19].try_into().ok()?);
				Some(RuntimeCall::Stableswap(pallet_stableswap::Call::sell {
					pool_id,
					asset_in,
					asset_out,
					amount_in,
					min_buy_amount: 0,
				}))
			}
			11 if data.len() > 19 => {
				let pool_id = data[0] as u32 % 3; //TODO: make as parameter
				let asset_in = assets[data[1] as usize % assets.len()];
				let asset_out = assets[data[2] as usize % assets.len()];
				let amount_out = u128::from_ne_bytes(data[3..19].try_into().ok()?);
				Some(RuntimeCall::Stableswap(pallet_stableswap::Call::buy {
					pool_id,
					asset_in,
					asset_out,
					amount_out,
					max_sell_amount: u128::MAX,
				}))
			}
			_ => None,
		}
	}
}
