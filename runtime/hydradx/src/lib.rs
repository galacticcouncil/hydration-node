// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "512"]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::items_after_test_module)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(test)]
mod tests;

mod benchmarking;
mod migrations;
pub mod weights;

mod assets;
pub mod evm;
mod governance;
mod system;
pub mod xcm;

pub use assets::*;
pub use governance::*;
pub use system::*;
pub use xcm::*;

use crate::sp_api_hidden_includes_construct_runtime::hidden_include::traits::Hooks;
use codec::{Decode, Encode};
use sp_api::impl_runtime_apis;
use sp_core::{ConstU128, Get, OpaqueMetadata, H160, H256, U256};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, PostDispatchInfoOf,
		UniqueSaturatedInto,
	},
	transaction_validity::{TransactionSource, TransactionValidity, TransactionValidityError},
	ApplyExtrinsicResult, Permill,
};

use sp_std::convert::From;
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
// A few exports that help ease life for downstream crates.
use frame_support::{construct_runtime, weights::Weight};
pub use hex_literal::hex;
/// Import HydraDX pallets
pub use pallet_claims;
use pallet_ethereum::{Transaction as EthereumTransaction, TransactionStatus};
use pallet_evm::{Account as EVMAccount, FeeCalculator, GasWeightMapping, Runner};
pub use primitives::{
	AccountId, Amount, AssetId, Balance, BlockNumber, CollectionId, Hash, Index, ItemId, Price, Signature,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;
	use sp_runtime::{
		generic,
		traits::{BlakeTwo256, Hash as HashT},
	};

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
	/// Opaque block hash type.
	pub type Hash = <BlakeTwo256 as HashT>::Output;
	impl_opaque_keys! {
		pub struct SessionKeys {
			pub aura: Aura,
		}
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("hydradx"),
	impl_name: create_runtime_str!("hydradx"),
	authoring_version: 1,
	spec_version: 206,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 0,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![
		TreasuryPalletId::get().into_account_truncating(),
		VestingPalletId::get().into_account_truncating(),
		ReferralsPalletId::get().into_account_truncating(),
	]
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime
	{
		System: frame_system exclude_parts { Origin } = 1,
		Timestamp: pallet_timestamp = 3,
		//NOTE: 5 - is used by Scheduler which must be after cumulus_pallet_parachain_system
		Balances: pallet_balances = 7,
		TransactionPayment: pallet_transaction_payment exclude_parts { Config } = 9,
		Treasury: pallet_treasury = 11,
		Utility: pallet_utility = 13,
		Preimage: pallet_preimage = 15,
		Identity: pallet_identity = 17,
		Democracy: pallet_democracy exclude_parts { Config } = 19,
		Elections: pallet_elections_phragmen = 21,
		Council: pallet_collective::<Instance1> = 23,
		TechnicalCommittee: pallet_collective::<Instance2> = 25,
		Tips: pallet_tips = 27,
		Proxy: pallet_proxy = 29,
		Multisig: pallet_multisig = 31,
		Uniques: pallet_uniques = 32,

		// HydraDX related modules
		AssetRegistry: pallet_asset_registry = 51,
		Claims: pallet_claims = 53,
		GenesisHistory: pallet_genesis_history = 55,
		CollatorRewards: pallet_collator_rewards = 57,
		Omnipool: pallet_omnipool = 59,
		TransactionPause: pallet_transaction_pause = 60,
		Duster: pallet_duster = 61,
		OmnipoolWarehouseLM: warehouse_liquidity_mining::<Instance1> = 62,
		OmnipoolLiquidityMining: pallet_omnipool_liquidity_mining = 63,
		OTC: pallet_otc = 64,
		CircuitBreaker: pallet_circuit_breaker = 65,

		Router: pallet_route_executor = 67,
		DynamicFees: pallet_dynamic_fees = 68,
		Staking: pallet_staking = 69,
		Stableswap: pallet_stableswap = 70,
		Bonds: pallet_bonds = 71,
		LBP: pallet_lbp = 73,
		XYK: pallet_xyk = 74,
		Referrals: pallet_referrals = 75,
		XcmRateLimiter: pallet_xcm_rate_limiter = 76,

		// ORML related modules
		Tokens: orml_tokens = 77,
		Currencies: pallet_currencies = 79,
		Vesting: orml_vesting = 81,

		// Frontier
		EVM: pallet_evm = 90,
		EVMChainId: pallet_evm_chain_id = 91,
		Ethereum: pallet_ethereum = 92,

		// Parachain
		ParachainSystem: cumulus_pallet_parachain_system exclude_parts { Config } = 103,
		ParachainInfo: parachain_info = 105,

		//NOTE: Scheduler must be after ParachainSystem otherwise RelayChainBlockNumberProvider
		//will return 0 as current block number when used with Scheduler(democracy).
		Scheduler: pallet_scheduler = 5,

		//NOTE: DCA pallet should be declared after ParachainSystem pallet,
		//otherwise there is no data about relay chain parent hash
		DCA: pallet_dca = 66,

		PolkadotXcm: pallet_xcm = 107,
		CumulusXcm: cumulus_pallet_xcm = 109,
		XcmpQueue: cumulus_pallet_xcmp_queue exclude_parts { Call } = 111,
		DmpQueue: cumulus_pallet_dmp_queue = 113,

		// ORML XCM
		OrmlXcm: orml_xcm = 135,
		XTokens: orml_xtokens = 137,
		UnknownTokens: orml_unknown_tokens = 139,

		// Collator support
		Authorship: pallet_authorship = 161,
		CollatorSelection: pallet_collator_selection = 163,
		Session: pallet_session = 165,
		Aura: pallet_aura = 167,
		AuraExt: cumulus_pallet_aura_ext = 169,

		// Warehouse - let's allocate indices 100+ for warehouse pallets
		RelayChainInfo: pallet_relaychain_info = 201,
		EmaOracle: pallet_ema_oracle = 202,
		MultiTransactionPayment: pallet_transaction_multi_payment = 203,
	}
);

/// The address format for describing accounts.
pub type Address = AccountId;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	pallet_claims::ValidateClaim<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsReversedWithSystemFirst,
	(migrations::OnRuntimeUpgradeMigration,),
>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}

		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, BlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}


	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}

		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}

		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	// Frontier RPC support
	impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
		fn chain_id() -> u64 {
			<Runtime as pallet_evm::Config>::ChainId::get()
		}

		fn account_basic(address: H160) -> EVMAccount {
			let (account, _) = EVM::account_basic(&address);
			account
		}

		fn gas_price() -> U256 {
			let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
			gas_price
		}

		fn account_code_at(address: H160) -> Vec<u8> {
			pallet_evm::AccountCodes::<Runtime>::get(address)
		}

		fn author() -> H160 {
			<pallet_evm::Pallet<Runtime>>::find_author()
		}

		fn storage_at(address: H160, index: U256) -> H256 {
			let mut tmp = [0u8; 32];
			index.to_big_endian(&mut tmp);
			pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
		}

		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
			let mut config = <Runtime as pallet_evm::Config>::config().clone();
			config.estimate = estimate;

			let is_transactional = false;
			let validate = true;

			// Estimated encoded transaction size must be based on the heaviest transaction
			// type (EIP1559Transaction) to be compatible with all transaction types.
			let mut estimated_transaction_len = data.len() +
				// pallet ethereum index: 1
				// transact call index: 1
				// Transaction enum variant: 1
				// chain_id 8 bytes
				// nonce: 32
				// max_priority_fee_per_gas: 32
				// max_fee_per_gas: 32
				// gas_limit: 32
				// action: 21 (enum varianrt + call address)
				// value: 32
				// access_list: 1 (empty vec size)
				// 65 bytes signature
				258;

			if access_list.is_some() {
				estimated_transaction_len += access_list.encoded_size();
			}

			let gas_limit = gas_limit.min(u64::MAX.into()).low_u64();
			let without_base_extrinsic_weight = true;

			let (weight_limit, proof_size_base_cost) =
						match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
							gas_limit,
							without_base_extrinsic_weight
						) {
							weight_limit if weight_limit.proof_size() > 0 => {
								(Some(weight_limit), Some(estimated_transaction_len as u64))
							}
							_ => (None, None),
						};

			<Runtime as pallet_evm::Config>::Runner::call(
				from,
				to,
				data,
				value,
				gas_limit.unique_saturated_into(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.unwrap_or_default(),
				is_transactional,
				validate,
				weight_limit,
				proof_size_base_cost,
				&config,
			)
			.map_err(|err| err.error.into())
		}

		fn create(
			_from: H160,
			_data: Vec<u8>,
			_value: U256,
			_gas_limit: U256,
			_max_fee_per_gas: Option<U256>,
			_max_priority_fee_per_gas: Option<U256>,
			_nonce: Option<U256>,
			_estimate: bool,
			_access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
			Err(sp_runtime::DispatchError::Other(
				"Creating contracts is not currently supported",
			))
		}

		fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
			pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
		}

		fn current_block() -> Option<pallet_ethereum::Block> {
			pallet_ethereum::CurrentBlock::<Runtime>::get()
		}

		fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
			pallet_ethereum::CurrentReceipts::<Runtime>::get()
		}

		fn current_all() -> (
			Option<pallet_ethereum::Block>,
			Option<Vec<pallet_ethereum::Receipt>>,
			Option<Vec<TransactionStatus>>,
		) {
			(
				pallet_ethereum::CurrentBlock::<Runtime>::get(),
				pallet_ethereum::CurrentReceipts::<Runtime>::get(),
				pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get(),
			)
		}

		fn extrinsic_filter(xts: Vec<<Block as BlockT>::Extrinsic>) -> Vec<EthereumTransaction> {
			xts.into_iter()
				.filter_map(|xt| match xt.0.function {
					RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) => Some(transaction),
					_ => None,
				})
				.collect::<Vec<EthereumTransaction>>()
		}

		fn elasticity() -> Option<Permill> {
			None
		}

		fn gas_limit_multiplier_support() {}

		fn pending_block(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> (Option<pallet_ethereum::Block>, Option<Vec<TransactionStatus>>) {
			for ext in xts.into_iter() {
				let _ = Executive::apply_extrinsic(ext);
			}

			Ethereum::on_finalize(System::block_number() + 1);

			(
				pallet_ethereum::CurrentBlock::<Runtime>::get(),
				pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
			)
		}

		fn initialize_pending_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
		fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
			UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use orml_benchmarking::list_benchmark as orml_list_benchmark;
			use frame_system_benchmarking::Pallet as SystemBench;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmark!(list, extra, frame_system, SystemBench::<Runtime>);
			list_benchmark!(list, extra, pallet_balances, Balances);
			list_benchmark!(list, extra, pallet_collator_selection, CollatorSelection);
			list_benchmark!(list, extra, pallet_timestamp, Timestamp);
			list_benchmark!(list, extra, pallet_treasury, Treasury);
			list_benchmark!(list, extra, pallet_preimage, Preimage);
			list_benchmark!(list, extra, pallet_scheduler, Scheduler);
			list_benchmark!(list, extra, pallet_identity, Identity);
			list_benchmark!(list, extra, pallet_tips, Tips);
			list_benchmark!(list, extra, pallet_proxy, Proxy);
			list_benchmark!(list, extra, pallet_utility, Utility);
			list_benchmark!(list, extra, pallet_democracy, Democracy);
			list_benchmark!(list, extra, council, Council);
			list_benchmark!(list, extra, tech, TechnicalCommittee);
			list_benchmark!(list, extra, pallet_omnipool_liquidity_mining, OmnipoolLiquidityMining);
			list_benchmark!(list, extra, pallet_circuit_breaker, CircuitBreaker);
			list_benchmark!(list, extra, pallet_bonds, Bonds);
			list_benchmark!(list, extra, pallet_stableswap, Stableswap);

			list_benchmark!(list, extra, pallet_asset_registry, AssetRegistry);
			list_benchmark!(list, extra, pallet_claims, Claims);
			list_benchmark!(list, extra, pallet_ema_oracle, EmaOracle);
			list_benchmark!(list, extra, pallet_staking, Staking);
			list_benchmark!(list, extra, pallet_lbp, LBP);
			list_benchmark!(list, extra, pallet_xyk, XYK);
			list_benchmark!(list, extra, pallet_referrals, Referrals);

			list_benchmark!(list, extra, cumulus_pallet_xcmp_queue, XcmpQueue);
			list_benchmark!(list, extra, pallet_transaction_pause, TransactionPause);

			list_benchmark!(list, extra, pallet_otc, OTC);
			list_benchmark!(list, extra, pallet_xcm, PolkadotXcm);

			orml_list_benchmark!(list, extra, pallet_currencies, benchmarking::currencies);
			orml_list_benchmark!(list, extra, orml_tokens, benchmarking::tokens);
			orml_list_benchmark!(list, extra, orml_vesting, benchmarking::vesting);
			orml_list_benchmark!(list, extra, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_list_benchmark!(list, extra, pallet_duster, benchmarking::duster);
			orml_list_benchmark!(list, extra, pallet_omnipool, benchmarking::omnipool);
			orml_list_benchmark!(list, extra, pallet_route_executor, benchmarking::route_executor);
			orml_list_benchmark!(list, extra, pallet_dca, benchmarking::dca);

			let storage_info = AllPalletsWithSystem::storage_info();

			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{BenchmarkError, Benchmarking, BenchmarkBatch, add_benchmark};
			use frame_support::traits::TrackedStorageKey;
			use orml_benchmarking::add_benchmark as orml_add_benchmark;
			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {
				fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
					ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
					Ok(())
				}

				fn verify_set_code() {
					System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				}
			}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			// Substrate pallets
			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, pallet_collator_selection, CollatorSelection);
			add_benchmark!(params, batches, pallet_timestamp, Timestamp);
			add_benchmark!(params, batches, pallet_treasury, Treasury);
			add_benchmark!(params, batches, pallet_preimage, Preimage);
			add_benchmark!(params, batches, pallet_scheduler, Scheduler);
			add_benchmark!(params, batches, pallet_identity, Identity);
			add_benchmark!(params, batches, pallet_tips, Tips);
			add_benchmark!(params, batches, pallet_proxy, Proxy);
			add_benchmark!(params, batches, pallet_utility, Utility);
			add_benchmark!(params, batches, pallet_democracy, Democracy);
			add_benchmark!(params, batches, council, Council);
			add_benchmark!(params, batches, tech, TechnicalCommittee);
			add_benchmark!(params, batches, pallet_omnipool_liquidity_mining, OmnipoolLiquidityMining);
			add_benchmark!(params, batches, pallet_circuit_breaker, CircuitBreaker);
			add_benchmark!(params, batches, pallet_asset_registry, AssetRegistry);
			add_benchmark!(params, batches, pallet_claims, Claims);
			add_benchmark!(params, batches, pallet_ema_oracle, EmaOracle);
			add_benchmark!(params, batches, pallet_bonds, Bonds);
			add_benchmark!(params, batches, pallet_staking, Staking);
			add_benchmark!(params, batches, pallet_lbp, LBP);
			add_benchmark!(params, batches, pallet_xyk, XYK);
			add_benchmark!(params, batches, pallet_stableswap, Stableswap);
			add_benchmark!(params, batches, pallet_referrals, Referrals);

			add_benchmark!(params, batches, cumulus_pallet_xcmp_queue, XcmpQueue);
			add_benchmark!(params, batches, pallet_transaction_pause, TransactionPause);

			add_benchmark!(params, batches, pallet_otc, OTC);
			add_benchmark!(params, batches, pallet_xcm, PolkadotXcm);

			orml_add_benchmark!(params, batches, pallet_currencies, benchmarking::currencies);
			orml_add_benchmark!(params, batches, orml_tokens, benchmarking::tokens);
			orml_add_benchmark!(params, batches, orml_vesting, benchmarking::vesting);
			orml_add_benchmark!(params, batches, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_add_benchmark!(params, batches, pallet_duster, benchmarking::duster);
			orml_add_benchmark!(params, batches, pallet_omnipool, benchmarking::omnipool);
			orml_add_benchmark!(params, batches, pallet_route_executor, benchmarking::route_executor);
			orml_add_benchmark!(params, batches, pallet_dca, benchmarking::dca);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data = cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
			relay_chain_slot,
			sp_std::time::Duration::from_secs(6),
		)
		.create_inherent_data()
		.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}

impl fp_self_contained::SelfContainedCall for RuntimeCall {
	type SignedInfo = H160;

	fn is_self_contained(&self) -> bool {
		match self {
			RuntimeCall::Ethereum(call) => call.is_self_contained(),
			_ => false,
		}
	}

	fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
		match self {
			RuntimeCall::Ethereum(call) => call.check_self_contained(),
			_ => None,
		}
	}

	fn validate_self_contained(
		&self,
		info: &Self::SignedInfo,
		dispatch_info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Option<TransactionValidity> {
		match self {
			RuntimeCall::Ethereum(call) => call.validate_self_contained(info, dispatch_info, len),
			_ => None,
		}
	}

	fn pre_dispatch_self_contained(
		&self,
		info: &Self::SignedInfo,
		dispatch_info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Option<Result<(), TransactionValidityError>> {
		match self {
			RuntimeCall::Ethereum(call) => call.pre_dispatch_self_contained(info, dispatch_info, len),
			_ => None,
		}
	}

	fn apply_self_contained(
		self,
		info: Self::SignedInfo,
	) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
		match self {
			call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) => Some(call.dispatch(
				RuntimeOrigin::from(pallet_ethereum::RawOrigin::EthereumTransaction(info)),
			)),
			_ => None,
		}
	}
}

pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
	fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
	}
}

impl fp_rpc::ConvertTransaction<sp_runtime::OpaqueExtrinsic> for TransactionConverter {
	fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> sp_runtime::OpaqueExtrinsic {
		let extrinsic =
			UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into());
		let encoded = extrinsic.encode();
		sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid")
	}
}
