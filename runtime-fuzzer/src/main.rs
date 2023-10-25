use codec::{DecodeLimit, Encode};
use frame_support::{
    dispatch::GetDispatchInfo,
    pallet_prelude::Weight,
    traits::{IntegrityTest, TryState, TryStateSelect},
    weights::constants::WEIGHT_REF_TIME_PER_SECOND,
};
use node_template_runtime::{
    AccountId, AllPalletsWithSystem, BlockNumber, Executive, Runtime, RuntimeCall, RuntimeOrigin,
    UncheckedExtrinsic, SLOT_DURATION,
};
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_runtime::{
    traits::{Dispatchable, Header},
    Digest, DigestItem, Storage,
};
use std::time::{Duration, Instant};

// We use a simple Map-based Externalities implementation
type Externalities = sp_state_machine::BasicExternalities;

// The initial timestamp at the start of an input run.
const INITIAL_TIMESTAMP: u64 = 0;

/// The maximum number of blocks per fuzzer input.
/// If set to 0, then there is no limit at all.
/// Feel free to set this to a low number (e.g. 4) when you begin your fuzzing campaign and then set it back to 32 once you have good coverage.
const MAX_BLOCKS_PER_INPUT: usize = 4;

/// The maximum number of extrinsics per block.
/// If set to 0, then there is no limit at all.
/// Feel free to set this to a low number (e.g. 4) when you begin your fuzzing campaign and then set it back to 0 once you have good coverage.
const MAX_EXTRINSICS_PER_BLOCK: usize = 4;

/// Max number of seconds a block should run for.
const MAX_TIME_FOR_BLOCK: u64 = 6;

// We do not skip more than DEFAULT_STORAGE_PERIOD to avoid pallet_transaction_storage from
// panicking on finalize.
const MAX_BLOCK_LAPSE: u32 = sp_transaction_storage_proof::DEFAULT_STORAGE_PERIOD;

// Extrinsic delimiter: `********`
const DELIMITER: [u8; 8] = [42; 8];

struct Data<'a> {
    data: &'a [u8],
    pointer: usize,
    size: usize,
}

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

fn main() {
    let endowed_accounts: Vec<AccountId> = (0..5).map(|i| [i; 32].into()).collect();

    let genesis_storage: Storage = {
        use node_template_runtime::{
            AuraConfig, BalancesConfig, GrandpaConfig, RuntimeGenesisConfig, SudoConfig,
        };
        use pallet_grandpa::AuthorityId as GrandpaId;
        use sp_consensus_aura::sr25519::AuthorityId as AuraId;
        use sp_runtime::app_crypto::ByteArray;
        use sp_runtime::BuildStorage;

        let initial_authorities: Vec<(AuraId, GrandpaId)> = vec![(
            AuraId::from_slice(&[0; 32]).unwrap(),
            GrandpaId::from_slice(&[0; 32]).unwrap(),
        )];

        RuntimeGenesisConfig {
            system: Default::default(),
            balances: BalancesConfig {
                // Configure endowed accounts with initial balance of 1 << 60.
                balances: endowed_accounts
                    .iter()
                    .cloned()
                    .map(|k| (k, 1 << 60))
                    .collect(),
            },
            aura: AuraConfig {
                authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
            },
            grandpa: GrandpaConfig {
                authorities: initial_authorities
                    .iter()
                    .map(|x| (x.1.clone(), 1))
                    .collect(),
                ..Default::default()
            },
            sudo: SudoConfig {
                // Assign no network admin rights.
                key: None,
            },
            transaction_payment: Default::default(),
        }
        .build_storage()
        .unwrap()
    };

    ziggy::fuzz!(|data: &[u8]| {
        let iteratable = Data {
            data,
            pointer: 0,
            size: 0,
        };

        // Max weight for a block.
        let max_weight: Weight = Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND * 2, 0);

        let mut block_count = 0;
        let mut extrinsics_in_block = 0;

        let extrinsics: Vec<(Option<u32>, usize, RuntimeCall)> = iteratable
            .filter_map(|data| {
                // We have reached the limit of block we want to decode
                if MAX_BLOCKS_PER_INPUT != 0 && block_count >= MAX_BLOCKS_PER_INPUT {
                    return None;
                }
                // lapse is u32 (4 bytes), origin is u16 (2 bytes) -> 6 bytes minimum
                let min_data_len = 4 + 2;
                if data.len() <= min_data_len {
                    return None;
                }
                let lapse: u32 = u32::from_ne_bytes(data[0..4].try_into().unwrap());
                let origin: usize = u16::from_ne_bytes(data[4..6].try_into().unwrap()) as usize;
                let mut encoded_extrinsic: &[u8] = &data[6..];

                // If the lapse is in the range [1, MAX_BLOCK_LAPSE] it is valid.
                let maybe_lapse = match lapse {
                    1..=MAX_BLOCK_LAPSE => Some(lapse),
                    _ => None,
                };
                // We have reached the limit of extrinsics for this block
                if maybe_lapse.is_none()
                    && MAX_EXTRINSICS_PER_BLOCK != 0
                    && extrinsics_in_block >= MAX_EXTRINSICS_PER_BLOCK
                {
                    return None;
                }

                match DecodeLimit::decode_all_with_depth_limit(64, &mut encoded_extrinsic) {
                    Ok(decoded_extrinsic) => {
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
                    }
                    Err(_) => None,
                }
            })
            .collect();

        if extrinsics.is_empty() {
            return;
        }

        // `externalities` represents the state of our mock chain.
        let mut externalities = Externalities::new(genesis_storage.clone());

        let mut current_block: u32 = 1;
        let mut current_timestamp: u64 = INITIAL_TIMESTAMP;
        let mut current_weight: Weight = Weight::zero();
        // let mut already_seen = 0; // This must be uncommented if you want to print events
        let mut elapsed: Duration = Duration::ZERO;

        let start_block = |block: u32, current_timestamp: u64| {
            #[cfg(not(fuzzing))]
            println!("\ninitializing block {block}");

            let pre_digest = match current_timestamp {
                INITIAL_TIMESTAMP => Default::default(),
                _ => Digest {
                    logs: vec![DigestItem::PreRuntime(
                        AURA_ENGINE_ID,
                        Slot::from(current_timestamp / SLOT_DURATION).encode(),
                    )],
                },
            };

            Executive::initialize_block(&Header::new(
                block,
                Default::default(),
                Default::default(),
                Default::default(),
                pre_digest,
            ));

            #[cfg(not(fuzzing))]
            println!("  setting timestamp");
            // We apply the timestamp extrinsic for the current block.
            Executive::apply_extrinsic(UncheckedExtrinsic::new_unsigned(RuntimeCall::Timestamp(
                pallet_timestamp::Call::set {
                    now: current_timestamp,
                },
            )))
            .unwrap()
            .unwrap();

            // Calls that need to be called before each block starts (init_calls) go here
        };

        let end_block = |current_block: u32, _current_timestamp: u64| {
            #[cfg(not(fuzzing))]
            println!("  finalizing block {current_block}");
            Executive::finalize_block();

            #[cfg(not(fuzzing))]
            println!("  testing invariants for block {current_block}");
            <AllPalletsWithSystem as TryState<BlockNumber>>::try_state(
                current_block,
                TryStateSelect::All,
            )
            .unwrap();
        };

        externalities.execute_with(|| start_block(current_block, current_timestamp));

        for (maybe_lapse, origin, extrinsic) in extrinsics {
            // If the lapse is in the range [0, MAX_BLOCK_LAPSE] we finalize the block and initialize
            // a new one.
            if let Some(lapse) = maybe_lapse {
                // We end the current block
                externalities.execute_with(|| end_block(current_block, current_timestamp));

                // We update our state variables
                current_block += lapse;
                current_timestamp += lapse as u64 * SLOT_DURATION;
                current_weight = Weight::zero();
                elapsed = Duration::ZERO;

                // We start the next block
                externalities.execute_with(|| start_block(current_block, current_timestamp));
            }

            // We get the current time for timing purposes.
            let now = Instant::now();

            let mut call_weight = Weight::zero();
            // We compute the weight to avoid overweight blocks.
            externalities.execute_with(|| {
                call_weight = extrinsic.get_dispatch_info().weight;
            });

            current_weight = current_weight.saturating_add(call_weight);
            if current_weight.ref_time() >= max_weight.ref_time() {
                #[cfg(not(fuzzing))]
                println!("Skipping because of max weight {}", max_weight);
                continue;
            }

            externalities.execute_with(|| {
                let origin_account = endowed_accounts[origin % endowed_accounts.len()].clone();
                #[cfg(not(fuzzing))]
                {
                    println!("\n    origin:     {:?}", origin_account);
                    println!("    call:       {:?}", extrinsic);
                }
                let _res = extrinsic
                    .clone()
                    .dispatch(RuntimeOrigin::signed(origin_account));
                #[cfg(not(fuzzing))]
                println!("    result:     {:?}", _res);

                // Uncomment to print events for debugging purposes
                /*
                #[cfg(not(fuzzing))]
                {
                    let all_events = node_template_runtime::System::events();
                    let events: Vec<_> = all_events.clone().into_iter().skip(already_seen).collect();
                    already_seen = all_events.len();
                    println!("  events:     {:?}\n", events);
                }
                */
            });

            elapsed += now.elapsed();
        }

        #[cfg(not(fuzzing))]
        println!("\n  time spent: {:?}", elapsed);
        if elapsed.as_secs() > MAX_TIME_FOR_BLOCK {
            panic!("block execution took too much time")
        }

        // We end the final block
        externalities.execute_with(|| end_block(current_block, current_timestamp));

        // After execution of all blocks.
        externalities.execute_with(|| {
            // We keep track of the sum of balance of accounts
            let mut counted_free = 0;
            let mut counted_reserved = 0;
            let mut _counted_frozen = 0;

            for acc in frame_system::Account::<Runtime>::iter() {
                // Check that the consumer/provider state is valid.
                let acc_consumers = acc.1.consumers;
                let acc_providers = acc.1.providers;
                if acc_consumers > 0 && acc_providers == 0 {
                    panic!("Invalid state");
                }

                // Increment our balance counts
                counted_free += acc.1.data.free;
                counted_reserved += acc.1.data.reserved;
                _counted_frozen += acc.1.data.frozen;
            }

            let total_issuance = pallet_balances::TotalIssuance::<Runtime>::get();
            let counted_issuance = counted_free + counted_reserved;
            if total_issuance != counted_issuance {
                panic!(
                    "Inconsistent total issuance: {total_issuance} but counted {counted_issuance}"
                );
            }

            #[cfg(not(fuzzing))]
            println!("\nrunning integrity tests\n");
            // We run all developer-defined integrity tests
            <AllPalletsWithSystem as IntegrityTest>::integrity_test();
        });
    });
}
