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
pub mod governance;
mod helpers;
mod hyperbridge;
mod system;
pub mod types;
pub mod xcm;

pub use assets::*;
pub use cumulus_primitives_core::{GeneralIndex, Here, Junctions::X1, NetworkId, NonFungible, Response};
pub use frame_support::{assert_ok, parameter_types, storage::with_transaction, traits::TrackedStorageKey};
pub use frame_system::RawOrigin;
pub use governance::origins::pallet_custom_origins;
pub use governance::*;
pub use pallet_asset_registry::AssetType;
pub use pallet_currencies_rpc_runtime_api::AccountData;
pub use pallet_referrals::{FeeDistribution, Level};
pub use polkadot_xcm::opaque::lts::InteriorLocation;
pub use system::*;
pub use xcm::*;

use codec::{Decode, Encode};
use hydradx_traits::evm::InspectEvmAccounts;
use sp_core::{ConstU128, Get, H160, H256, U256};
use sp_genesis_builder::PresetId;
pub use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, PostDispatchInfoOf,
		UniqueSaturatedInto,
	},
	transaction_validity::{TransactionValidity, TransactionValidityError},
	DispatchError, Permill, TransactionOutcome,
};

use sp_std::{convert::From, prelude::*};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
// A few exports that help ease life for downstream crates.
use frame_support::{construct_runtime, pallet_prelude::Hooks, weights::Weight};
pub use hex_literal::hex;
use orml_traits::MultiCurrency;
/// Import HydraDX pallets
pub use pallet_claims;
use pallet_ethereum::{Transaction as EthereumTransaction, TransactionStatus};
use pallet_evm::{Account as EVMAccount, FeeCalculator, GasWeightMapping, Runner};
pub use pallet_genesis_history::Chain;
pub use primitives::{
	constants::time::SLOT_DURATION, AccountId, Amount, AssetId, Balance, BlockNumber, CollectionId, Hash, Index,
	ItemId, Price, Signature,
};
use sp_api::impl_runtime_apis;
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
	spec_version: 317,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
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
		BondsPalletId::get().into_account_truncating(),
		pallet_route_executor::Pallet::<Runtime>::router_account(),
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
		// due to multi payment pallet prices, this needs to be initialized at the very beginning
		MultiTransactionPayment: pallet_transaction_multi_payment = 203,
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
		StateTrieMigration: pallet_state_trie_migration = 35,

		// OpenGov
		ConvictionVoting: pallet_conviction_voting = 36,
		Referenda: pallet_referenda = 37,
		Origins: pallet_custom_origins = 38,
		Whitelist: pallet_whitelist = 39,
		Dispatcher: pallet_dispatcher = 40,

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
		OtcSettlements: pallet_otc_settlements = 72,
		LBP: pallet_lbp = 73,
		XYK: pallet_xyk = 74,
		Referrals: pallet_referrals = 75,
		Liquidation: pallet_liquidation = 76,
		HSM: pallet_hsm = 82,

		// ORML related modules
		Tokens: orml_tokens = 77,
		Currencies: pallet_currencies = 79,
		Vesting: orml_vesting = 81,

		// Frontier and EVM pallets
		EVM: pallet_evm = 90,
		EVMChainId: pallet_evm_chain_id = 91,
		Ethereum: pallet_ethereum = 92,
		EVMAccounts: pallet_evm_accounts = 93,
		DynamicEvmFee: pallet_dynamic_evm_fee = 94,

		XYKLiquidityMining: pallet_xyk_liquidity_mining = 95,
		XYKWarehouseLM: warehouse_liquidity_mining::<Instance2> = 96,

		RelayChainInfo: pallet_relaychain_info = 201,
		//NOTE: DCA pallet should be declared before ParachainSystem pallet,
		//otherwise there is no data about relay chain parent hash
		DCA: pallet_dca = 66,
		//NOTE: Scheduler must be before ParachainSystem otherwise RelayChainBlockNumberProvider
		//will return 0 as current block number when used with Scheduler(democracy).
		Scheduler: pallet_scheduler = 5,

		// Parachain
		ParachainSystem: cumulus_pallet_parachain_system exclude_parts { Config } = 103,
		ParachainInfo: staging_parachain_info = 105,

		PolkadotXcm: pallet_xcm = 107,
		CumulusXcm: cumulus_pallet_xcm = 109,
		XcmpQueue: cumulus_pallet_xcmp_queue exclude_parts { Call } = 111,
		// 113 was used by DmpQueue which is now replaced by MessageQueue
		MessageQueue: pallet_message_queue = 114,

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

		// Hyperbridge
		Ismp: pallet_ismp = 180,
		IsmpParachain: ismp_parachain = 181,
		Hyperbridge: pallet_hyperbridge = 182,

		// Warehouse - let's allocate indices 100+ for warehouse pallets
		EmaOracle: pallet_ema_oracle = 202,
		Broadcast: pallet_broadcast = 204,
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
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	pallet_claims::ValidateClaim<Runtime>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	cumulus_primitives_storage_weight_reclaim::StorageWeightReclaim<Runtime>,
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
	AllPalletsWithSystem,
	migrations::Migrations,
>;

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_support::parameter_types! {
		pub const BenchmarkMaxBalance: crate::Balance = crate::Balance::MAX;
	}
	frame_benchmarking::define_benchmarks!(
		[pallet_lbp, LBP]
		[pallet_asset_registry, AssetRegistry]
		[pallet_transaction_pause, TransactionPause]
		[pallet_circuit_breaker, CircuitBreaker]
		[pallet_bonds, Bonds]
		[pallet_stableswap, Stableswap]
		[pallet_claims, Claims]
		[pallet_staking, Staking]
		[pallet_referrals, Referrals]
		[pallet_evm_accounts, EVMAccounts]
		[pallet_otc, OTC]
		[pallet_otc_settlements, OtcSettlements]
		[pallet_liquidation, Liquidation]
		[pallet_state_trie_migration, StateTrieMigration]
		[frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_timestamp, Timestamp]
		[pallet_democracy, Democracy]
		[pallet_elections_phragmen, Elections]
		[pallet_treasury, Treasury]
		[pallet_scheduler, Scheduler]
		[pallet_utility, Utility]
		[pallet_tips, Tips]
		[pallet_identity, Identity]
		[pallet_collective_council, Council]
		[pallet_collective_technical_committee, TechnicalCommittee]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[pallet_message_queue, MessageQueue]
		[pallet_preimage, Preimage]
		[pallet_multisig, Multisig]
		[pallet_proxy, Proxy]
		[cumulus_pallet_parachain_system, ParachainSystem]
		[pallet_collator_selection, CollatorSelection]
		[pallet_xcm, PalletXcmExtrinsiscsBenchmark::<Runtime>]
		[pallet_xcm_benchmarks::fungible, XcmBalances]
		[pallet_xcm_benchmarks::generic, XcmGeneric]
		[pallet_conviction_voting, ConvictionVoting]
		[pallet_referenda, Referenda]
		[pallet_whitelist, Whitelist]
		[pallet_dispatcher, Dispatcher]
		[pallet_hsm, HSM]
		[ismp_parachain, IsmpParachain]
	);
}

struct CheckInherents;

#[allow(deprecated)]
#[allow(dead_code)]
// There is some controversy around this deprecation. We can keep it as it is for now.
// See issue: https://github.com/paritytech/polkadot-sdk/issues/2841
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

use crate::evm::aave_trade_executor::AaveTradeExecutor;
use crate::evm::aave_trade_executor::PoolData;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use cumulus_pallet_parachain_system::RelayChainState;
use frame_support::{
	genesis_builder_helper::{build_state, get_preset},
	sp_runtime::{
		traits::Convert, transaction_validity::TransactionSource, ApplyExtrinsicResult, ExtrinsicInclusionMode,
		FixedPointNumber,
	},
	weights::WeightToFee as _,
};
use hydradx_traits::evm::Erc20Mapping;
use ismp::{
	consensus::{ConsensusClientId, StateMachineHeight, StateMachineId},
	host::StateMachine,
};
use pallet_liquidation::BorrowingContract;
use pallet_route_executor::TradeExecution;
pub use polkadot_xcm::latest::Junction;
use polkadot_xcm::{IntoVersion, VersionedAssetId, VersionedAssets, VersionedLocation, VersionedXcm};
use primitives::constants::chain::CORE_ASSET_ID;
pub use sp_arithmetic::FixedU128;
use sp_core::OpaqueMetadata;
use xcm_runtime_apis::{
	dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects},
	fees::Error as XcmPaymentApiError,
};

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
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
			sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
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

	impl pallet_currencies_rpc_runtime_api::CurrenciesApi<
		Block,
		AssetId,
		AccountId,
		Balance,
	> for Runtime {
		fn account(asset_id: AssetId, who: AccountId) -> AccountData<Balance> {
			if asset_id == NativeAssetId::get() {
				let data = System::account(&who).data;
				AccountData {
					free: data.free,
					reserved: data.reserved,
					frozen: data.frozen,
				}
			} else {
				let tokens_data = Tokens::accounts(who.clone(), asset_id);
				let mut data = AccountData {
					free: tokens_data.free,
					reserved: tokens_data.reserved,
					frozen: tokens_data.frozen,
				};
				if matches!(AssetRegistry::asset_type(asset_id), Some(AssetKind::Erc20)) {
					data.free = Self::free_balance(asset_id, who);
				}
				data
			}
		}

		fn accounts(who: AccountId) -> Vec<(AssetId, AccountData<Balance>)> {
			let mut result = Vec::new();

			// Add native token (HDX)
			let balance = System::account(&who).data;
			result.push((
				NativeAssetId::get(),
				AccountData {
					free: balance.free,
					reserved: balance.reserved,
					frozen: balance.frozen,
				}
			));

			// Add tokens from orml_tokens
			result.extend(
				orml_tokens::Accounts::<Runtime>::iter_prefix(&who)
					.map(|(asset_id, data)| {
						let mut account_data = AccountData {
							free: data.free,
							reserved: data.reserved,
							frozen: data.frozen,
						};

						// Update free balance for ERC20 tokens
						if matches!(AssetRegistry::asset_type(asset_id), Some(AssetKind::Erc20)) {
							account_data.free = Currencies::free_balance(asset_id, &who);
						}

						(asset_id, account_data)
					})
			);

			// Add ERC20 tokens with non-zero balance not yet added previously
			let existing_ids: Vec<_> = result.iter().map(|(id, _)| *id).collect();
			result.extend(
				pallet_asset_registry::Assets::<Runtime>::iter()
					.filter(|(_, info)| info.asset_type == AssetType::Erc20)
					.filter_map(|(asset_id, _)| {
						if existing_ids.contains(&asset_id) {
							return None;
						}

						let free = Currencies::free_balance(asset_id, &who);
						if free > 0 {
							Some((
								asset_id,
								AccountData {
									free,
									reserved: 0,
									frozen: 0,
								}
							))
						} else {
							None
						}
					})
			);

			result
		}

		fn free_balance(asset_id: AssetId, who: AccountId) -> Balance {
			Currencies::free_balance(asset_id, &who)
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

			// don't allow calling EVM RPC or Runtime API from a bound address
			if !estimate && EVMAccounts::bound_account_id(from).is_some() {
				return Err(pallet_evm_accounts::Error::<Runtime>::BoundAddressCannotBeUsed.into())
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
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
			let config = if estimate {
				let mut config = <Runtime as pallet_evm::Config>::config().clone();
				config.estimate = true;
				Some(config)
			} else {
				None
			};

			let is_transactional = false;
			let validate = true;

			// Reused approach from Moonbeam since Frontier implementation doesn't support this
			let mut estimated_transaction_len = data.len() +
				// to: 20
				// from: 20
				// value: 32
				// gas_limit: 32
				// nonce: 32
				// 1 byte transaction action variant
				// chain id 8 bytes
				// 65 bytes signature
				210;
			if max_fee_per_gas.is_some() {
				estimated_transaction_len += 32;
			}
			if max_priority_fee_per_gas.is_some() {
				estimated_transaction_len += 32;
			}
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

			// don't allow calling EVM RPC or Runtime API from a bound address
			if !estimate && EVMAccounts::bound_account_id(from).is_some() {
				return Err(pallet_evm_accounts::Error::<Runtime>::BoundAddressCannotBeUsed.into())
			};

			// the address needs to have a permission to deploy smart contract
			if !EVMAccounts::can_deploy_contracts(from) {
				return Err(pallet_evm_accounts::Error::<Runtime>::AddressNotWhitelisted.into())
			};

			#[allow(clippy::or_fun_call)] // suggestion not helpful here
			<Runtime as pallet_evm::Config>::Runner::create(
				from,
				data,
				value,
				gas_limit.unique_saturated_into(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				Vec::new(),
				is_transactional,
				validate,
				weight_limit,
				proof_size_base_cost,
				config
					.as_ref()
					.unwrap_or(<Runtime as pallet_evm::Config>::config()),
				)
				.map_err(|err| err.error.into())
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
			Executive::initialize_block(header);
		}
	}

	impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
		fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
			UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
		}
	}

	impl pallet_evm_accounts_rpc_runtime_api::EvmAccountsApi<Block, AccountId, H160> for Runtime {
		fn evm_address(account_id: AccountId) -> H160 {
			EVMAccounts::evm_address(&account_id)
		}
		fn bound_account_id(evm_address: H160) -> Option<AccountId> {
			EVMAccounts::bound_account_id(evm_address)
		}
		fn account_id(evm_address: H160) -> AccountId {
			EVMAccounts::account_id(evm_address)
		}
	}

	impl xcm_runtime_apis::fees::XcmPaymentApi<Block> for Runtime {
		fn query_acceptable_payment_assets(xcm_version: polkadot_xcm::Version) -> Result<Vec<VersionedAssetId>, XcmPaymentApiError> {
			if !matches!(xcm_version, 3 | 4) {
				return Err(XcmPaymentApiError::UnhandledXcmVersion);
			}

			let mut asset_locations = vec![
		AssetLocation(polkadot_xcm::v3::MultiLocation {
				parents: 1,
				interior: [
					polkadot_xcm::v3::Junction::Parachain(ParachainInfo::get().into()),
					polkadot_xcm::v3::Junction::GeneralIndex(CORE_ASSET_ID.into()),
				]
				.into(),
			}),
			AssetLocation(polkadot_xcm::v3::MultiLocation {
				parents: 0,
				interior: [
					polkadot_xcm::v3::Junction::GeneralIndex(CORE_ASSET_ID.into()),
				]
				.into(),
			})];

			let mut asset_registry_locations: Vec<AssetLocation> = pallet_asset_registry::LocationAssets::<Runtime>::iter_keys().collect();
			asset_locations.append(&mut asset_registry_locations);

			let versioned_locations = asset_locations.iter().map(|loc| VersionedAssetId::V3(polkadot_xcm::v3::AssetId::Concrete(loc.0)));

			Ok(versioned_locations
				.filter_map(|asset| asset.into_version(xcm_version).ok())
				.collect())
		}

		fn query_weight_to_asset_fee(weight: Weight, asset: VersionedAssetId) -> Result<u128, XcmPaymentApiError> {
			let v4_xcm_asset_id = asset.into_version(4).map_err(|_| XcmPaymentApiError::VersionedConversionFailed)?;

			// get nested polkadot_xcm::AssetId type
			let xcm_asset_id: &polkadot_xcm::v4::AssetId = v4_xcm_asset_id.try_as().map_err(|_| XcmPaymentApiError::WeightNotComputable)?;

			let asset_id: AssetId = CurrencyIdConvert::convert(xcm_asset_id.clone().0).ok_or(XcmPaymentApiError::AssetNotFound)?;

			let price = MultiTransactionPayment::price(asset_id).ok_or(XcmPaymentApiError::WeightNotComputable)?;

			let fee = WeightToFee::weight_to_fee(&weight);

			let converted_fee = price.checked_mul_int(fee).ok_or(XcmPaymentApiError::WeightNotComputable)?;

			Ok(converted_fee)
		}

		fn query_xcm_weight(message: VersionedXcm<()>) -> Result<Weight, XcmPaymentApiError> {
			PolkadotXcm::query_xcm_weight(message)
		}

		fn query_delivery_fees(destination: VersionedLocation, message: VersionedXcm<()>) -> Result<VersionedAssets, XcmPaymentApiError> {
			PolkadotXcm::query_delivery_fees(destination, message)
		}
	}

	impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
		fn can_build_upon(
				included_hash: <Block as BlockT>::Hash,
				slot: cumulus_primitives_aura::Slot,
		) -> bool {
				ConsensusHook::can_build_upon(included_hash, slot)
		}
	}

	impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller> for Runtime {
		fn dry_run_call(origin: OriginCaller, call: RuntimeCall) -> Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_call::<Runtime, xcm::XcmRouter, OriginCaller, RuntimeCall>(origin, call)
		}

		fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<XcmDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_xcm::<Runtime, xcm::XcmRouter, RuntimeCall, xcm::XcmConfig>(origin_location, xcm)
		}
	}

	impl xcm_runtime_apis::conversions::LocationToAccountApi<Block, AccountId> for Runtime {
		fn convert_location(location: VersionedLocation) -> Result<
			AccountId,
			xcm_runtime_apis::conversions::Error
		> {
			xcm_runtime_apis::conversions::LocationToAccountHelper::<
				AccountId,
				xcm::LocationToAccountId,
			>::convert_location(location)
		}
	}

	impl evm::precompiles::chainlink_adapter::runtime_api::ChainlinkAdapterApi<Block, AccountId, evm::EvmAddress> for Runtime {
		fn encode_oracle_address(asset_id_a: AssetId, asset_id_b: AssetId, period: OraclePeriod, source: Source) -> evm::EvmAddress {
			evm::precompiles::chainlink_adapter::encode_oracle_address(asset_id_a, asset_id_b, period, source)
		}

		fn decode_oracle_address(oracle_address: evm::EvmAddress) -> Option<(AssetId, AssetId, OraclePeriod, Source)> {
			evm::precompiles::chainlink_adapter::decode_oracle_address(oracle_address)
		}
	}

	impl evm::aave_trade_executor::runtime_api::AaveTradeExecutor<Block, Balance> for Runtime {
		fn pairs() -> Vec<(AssetId, AssetId)> {
			let pool = <BorrowingContract<Runtime>>::get();
			let reserves = match AaveTradeExecutor::<Runtime>::get_reserves_list(pool) {
				Ok(reserves) => reserves,
				Err(_) => return vec![]
			};
			reserves.into_iter()
				.filter_map(|reserve| {
					let data = AaveTradeExecutor::<Runtime>::get_reserve_data(pool, reserve).ok()?;
					let reserve_asset = HydraErc20Mapping::address_to_asset(reserve)?;
					let atoken_asset = HydraErc20Mapping::address_to_asset(data.atoken_address)?;
					Some((reserve_asset, atoken_asset))
				})
				.collect()
		}

		fn liquidity_depth(asset_in: AssetId, asset_out: AssetId) -> Option<Balance> {
			AaveTradeExecutor::<Runtime>::get_liquidity_depth(PoolType::Aave, asset_in, asset_out).ok()
		}

		fn pool(reserve: AssetId, atoken: AssetId) -> PoolData<Balance> {
			PoolData {
				reserve,
				atoken,
				liqudity_in: Self::liquidity_depth(reserve, atoken).unwrap(),
				liqudity_out: Self::liquidity_depth(atoken, reserve).unwrap(),
			}
		}

		fn pools() -> Vec<PoolData<Balance>> {
			Self::pairs().into_iter().map(|p| Self::pool(p.0, p.1)).collect()
		}
	}

	// Hyperbridge
	impl pallet_ismp_runtime_api::IsmpRuntimeApi<Block, <Block as BlockT>::Hash> for Runtime {
		fn host_state_machine() -> StateMachine {
			<Runtime as pallet_ismp::Config>::HostStateMachine::get()
		}

		fn challenge_period(state_machine_id: StateMachineId) -> Option<u64> {
			pallet_ismp::Pallet::<Runtime>::challenge_period(state_machine_id)
		}

		/// Fetch all ISMP events in the block, should only be called from runtime-api.
		fn block_events() -> Vec<::ismp::events::Event> {
			pallet_ismp::Pallet::<Runtime>::block_events()
		}

		/// Fetch all ISMP events and their extrinsic metadata, should only be called from runtime-api.
		fn block_events_with_metadata() -> Vec<(::ismp::events::Event, Option<u32>)> {
			pallet_ismp::Pallet::<Runtime>::block_events_with_metadata()
		}

		/// Return the scale encoded consensus state
		fn consensus_state(id: ConsensusClientId) -> Option<Vec<u8>> {
			pallet_ismp::Pallet::<Runtime>::consensus_states(id)
		}

		/// Return the timestamp this client was last updated in seconds
		fn state_machine_update_time(height: StateMachineHeight) -> Option<u64> {
			pallet_ismp::Pallet::<Runtime>::state_machine_update_time(height)
		}

		/// Return the latest height of the state machine
		fn latest_state_machine_height(id: StateMachineId) -> Option<u64> {
			pallet_ismp::Pallet::<Runtime>::latest_state_machine_height(id)
		}

		/// Get actual requests
		fn requests(commitments: Vec<H256>) -> Vec<ismp::router::Request> {
			pallet_ismp::Pallet::<Runtime>::requests(commitments)
		}

		/// Get actual requests
		fn responses(commitments: Vec<H256>) -> Vec<ismp::router::Response> {
			pallet_ismp::Pallet::<Runtime>::responses(commitments)
		}
	}

	impl ismp_parachain_runtime_api::IsmpParachainApi<Block> for Runtime {
		fn para_ids() -> Vec<u32> {
			IsmpParachain::para_ids()
		}

		fn current_relay_chain_state() -> RelayChainState {
			IsmpParachain::current_relay_chain_state()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {

		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use orml_benchmarking::list_benchmark as orml_list_benchmark;

			use frame_system_benchmarking::Pallet as SystemBench;
			use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsiscsBenchmark;

			// This is defined once again in dispatch_benchmark, because list_benchmarks!
			// and add_benchmarks! are macros exported by define_benchmarks! macros and those types
			// are referenced in that call.
			type XcmBalances = pallet_xcm_benchmarks::fungible::Pallet::<Runtime>;
			type XcmGeneric = pallet_xcm_benchmarks::generic::Pallet::<Runtime>;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmarks!(list, extra);

			orml_list_benchmark!(list, extra, pallet_currencies, benchmarking::currencies);
			orml_list_benchmark!(list, extra, orml_tokens, benchmarking::tokens);
			orml_list_benchmark!(list, extra, orml_vesting, benchmarking::vesting);
			orml_list_benchmark!(list, extra, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_list_benchmark!(list, extra, pallet_duster, benchmarking::duster);
			orml_list_benchmark!(list, extra, pallet_omnipool, benchmarking::omnipool);
			orml_list_benchmark!(list, extra, pallet_route_executor, benchmarking::route_executor);
			orml_list_benchmark!(list, extra, pallet_dca, benchmarking::dca);
			orml_list_benchmark!(list, extra, pallet_xyk, benchmarking::xyk);
			orml_list_benchmark!(list, extra, pallet_dynamic_evm_fee, benchmarking::dynamic_evm_fee);
			orml_list_benchmark!(list, extra, pallet_xyk_liquidity_mining, benchmarking::xyk_liquidity_mining);
			orml_list_benchmark!(list, extra, pallet_omnipool_liquidity_mining, benchmarking::omnipool_liquidity_mining);
			orml_list_benchmark!(list, extra, pallet_ema_oracle, benchmarking::ema_oracle);

			let storage_info = AllPalletsWithSystem::storage_info();

			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{BenchmarkError, Benchmarking, BenchmarkBatch};

			use orml_benchmarking::add_benchmark as orml_add_benchmark;
			use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsiscsBenchmark;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_primitives_core::ParaId;
			use primitives::constants::chain::CORE_ASSET_ID;
			use sp_std::sync::Arc;
			 use polkadot_runtime_common::xcm_sender::ExponentialPrice;
			 use primitives::constants::currency::CENTS;

			impl frame_system_benchmarking::Config for Runtime {
				fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
					ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
					Ok(())
				}

				fn verify_set_code() {
					System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				}
			}

			frame_support::parameter_types! {
				pub const RandomParaId: ParaId = ParaId::new(22_222_222);
				pub const ExistentialDeposit: u128 = 1_000_000_000_000;
				pub CoreAssetLocation: Location = Location::new(0, cumulus_primitives_core::Junctions::X1(
					Arc::new([
						cumulus_primitives_core::Junction::GeneralIndex(CORE_ASSET_ID.into())
						])
				));
				pub DaiLocation: Location = Location::new(0, cumulus_primitives_core::Junctions::X1(
					Arc::new([
						cumulus_primitives_core::Junction::GeneralIndex(2)
						])
				));
			}

			use polkadot_xcm::latest::prelude::{Location, AssetId, Fungible, Asset, Assets, Parent, ParentThen, Parachain};

			impl pallet_xcm::benchmarking::Config for Runtime {
				type DeliveryHelper = ();

				fn reachable_dest() -> Option<Location> {
					Some(Parent.into())
				}

				fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
					None
				}

				fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
					ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
								RandomParaId::get()
							);

					Some((
						Asset {
							fun: Fungible(ExistentialDeposit::get()),
							id: AssetId(CoreAssetLocation::get())
						},
						ParentThen(Parachain(RandomParaId::get().into()).into()).into(),
					))
				}

				fn set_up_complex_asset_transfer() -> Option<(Assets, u32, Location, Box<dyn FnOnce()>)> {
					ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
								RandomParaId::get()
							);

					let destination = ParentThen(Parachain(RandomParaId::get().into()).into()).into();

					let fee_asset: Asset = (
						   CoreAssetLocation::get(),
						   ExistentialDeposit::get(),
					 ).into();

					let who = frame_benchmarking::whitelisted_caller();
					let balance = 10 * ExistentialDeposit::get();
					let _ = <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(&who, balance );

					assert_eq!(Balances::free_balance(&who), balance);

					let transfer_asset: Asset = (
						   CoreAssetLocation::get(),
						   ExistentialDeposit::get(),
					 ).into();

					let assets: Assets = vec![fee_asset.clone(), transfer_asset].into();

					let fee_index: u32 = 0;

					let verify: Box<dyn FnOnce()> = Box::new(move || {
						assert!(Balances::free_balance(&who) <= balance - ExistentialDeposit::get());
					});

					Some((assets, fee_index, destination, verify))
			   }

				fn get_asset() -> Asset {
					Asset {
						id: AssetId(PolkadotLocation::get()),
						fun: Fungible(ExistentialDeposit::get()),
					}
				}
			}

			use primitives::constants::currency::UNITS;

			frame_support::parameter_types! {
				/// The asset ID for the asset that we use to pay for message delivery fees.
			pub FeeAssetId: cumulus_primitives_core::AssetId = AssetId(xcm::PolkadotLocation::get());
			/// The base fee for the message delivery fees.
			pub const BaseDeliveryFee: u128 = CENTS.saturating_mul(3);
				pub ExistentialDepositAsset: Option<Asset> = Some((
					CoreAssetLocation::get(),
					ExistentialDeposit::get()
				).into());
			}

			pub type PriceForParentDelivery = ExponentialPrice<FeeAssetId, BaseDeliveryFee, TransactionByteFee, ParachainSystem>;

			impl pallet_xcm_benchmarks::Config for Runtime {
				type XcmConfig = xcm::XcmConfig;
				type AccountIdConverter = xcm::LocationToAccountId;
				type DeliveryHelper = cumulus_primitives_utility::ToParentDeliveryHelper<
					xcm::XcmConfig,
					ExistentialDepositAsset,
					PriceForParentDelivery,
				>;
				fn valid_destination() -> Result<Location, BenchmarkError> {
					Ok(PolkadotLocation::get())
				}
				fn worst_case_holding(depositable_count: u32) -> Assets {
					// A mix of fungible and non-fungible assets
					let holding_non_fungibles = MaxAssetsIntoHolding::get() / 2 - depositable_count;
					let holding_fungibles = holding_non_fungibles - 2; // -2 for two `iter::once` bellow
					let fungibles_amount: u128 = UNITS;
					(0..holding_fungibles)
						.map(|i| {
							Asset {
								id: AssetId(cumulus_primitives_core::GeneralIndex(i as u128).into()),
								fun: Fungible(fungibles_amount * (i + 1) as u128), // non-zero amount
							}
						})
						.chain(core::iter::once(Asset { id: AssetId(Here.into()), fun: Fungible(u128::MAX) }))
						.chain(core::iter::once(Asset { id: AssetId(PolkadotLocation::get()), fun: Fungible(1_000_000 * UNITS) }))
						.chain((0..holding_non_fungibles).map(|i| Asset {
							id: AssetId(cumulus_primitives_core::GeneralIndex(i as u128).into()),
							fun: NonFungible(pallet_xcm_benchmarks::asset_instance_from(i)),
						}))
						.collect::<Vec<_>>()
						.into()
				}
			}

			frame_support::parameter_types! {
				pub const TrustedTeleporter: Option<(Location, Asset)> = Some((
					PolkadotLocation::get(),
					Asset { fun: Fungible(UNITS), id: AssetId(PolkadotLocation::get()) },
				));
				pub const CheckedAccount: Option<(AccountId, xcm_builder::MintLocation)> = None;
				pub TrustedReserve: Option<(Location, Asset)> = Some((
					PolkadotLocation::get(),
					Asset { fun: Fungible(UNITS), id: AssetId(PolkadotLocation::get()) },
				));
			}

			impl pallet_xcm_benchmarks::fungible::Config for Runtime {
				type TransactAsset = Balances;

				type CheckedAccount = CheckedAccount;
				type TrustedTeleporter = TrustedTeleporter;
				type TrustedReserve = TrustedReserve;

				fn get_asset() -> Asset {
					Asset {
						id: AssetId(CoreAssetLocation::get()),
						fun: Fungible(UNITS),
					}
				}
			}

			impl pallet_xcm_benchmarks::generic::Config for Runtime {
				type TransactAsset = Balances;
				type RuntimeCall = RuntimeCall;

				fn worst_case_response() -> (u64, Response) {
					(0u64, Response::Version(Default::default()))
				}

				fn worst_case_asset_exchange() -> Result<(Assets, Assets), BenchmarkError> {
					//We can only exchange from single asset to another single one at worst case
					let amount_to_sell = UNITS;
					let received = init_omnipool(amount_to_sell);
					let give : Assets = (AssetId(CoreAssetLocation::get()), amount_to_sell).into();
					let want : Assets = (AssetId(DaiLocation::get()),  received).into();//We need to set the exact amount as pallet_xcm_benchmarks::fungibles exchange_asset benchmark test requires to have original wanted fungible amount in holding, but we always put the exact amount received
					Ok((give, want))
				}

				fn universal_alias() -> Result<(Location, Junction), BenchmarkError> {
										Err(BenchmarkError::Skip)
				}

				fn transact_origin_and_runtime_call() -> Result<(Location, RuntimeCall), BenchmarkError> {
					Ok((PolkadotLocation::get(), frame_system::Call::remark_with_event { remark: vec![] }.into()))
				}

				fn subscribe_origin() -> Result<Location, BenchmarkError> {
					Ok(PolkadotLocation::get())
				}

				fn claimable_asset() -> Result<(Location, Location, Assets), BenchmarkError> {
					let origin = PolkadotLocation::get();
					let assets: Assets = (AssetId(PolkadotLocation::get()), 1_000 * UNITS).into();
					let ticket = Location { parents: 0, interior: Here };
					Ok((origin, ticket, assets))
				}

				fn fee_asset() -> Result<Asset, BenchmarkError> {
					Ok(Asset {
						id: AssetId(CoreAssetLocation::get()),
						fun: Fungible(UNITS),
					})
				}

				fn unlockable_asset() -> Result<(Location, Location, Asset), BenchmarkError> {
					Err(BenchmarkError::Skip)
				}

				fn export_message_origin_and_destination(
				) -> Result<(Location, NetworkId, InteriorLocation), BenchmarkError> {
					Err(BenchmarkError::Skip)
				}

				fn alias_origin() -> Result<(Location, Location), BenchmarkError> {
					Err(BenchmarkError::Skip)
				}
			}

			type XcmBalances = pallet_xcm_benchmarks::fungible::Pallet::<Runtime>;
			type XcmGeneric = pallet_xcm_benchmarks::generic::Pallet::<Runtime>;

			#[allow(unused_variables)] // TODO: this variable is not used
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

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmarks!(params, batches);

			orml_add_benchmark!(params, batches, pallet_currencies, benchmarking::currencies);
			orml_add_benchmark!(params, batches, orml_tokens, benchmarking::tokens);
			orml_add_benchmark!(params, batches, orml_vesting, benchmarking::vesting);
			orml_add_benchmark!(params, batches, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_add_benchmark!(params, batches, pallet_duster, benchmarking::duster);
			orml_add_benchmark!(params, batches, pallet_omnipool, benchmarking::omnipool);
			orml_add_benchmark!(params, batches, pallet_route_executor, benchmarking::route_executor);
			orml_add_benchmark!(params, batches, pallet_dca, benchmarking::dca);
			orml_add_benchmark!(params, batches, pallet_xyk, benchmarking::xyk);
			orml_add_benchmark!(params, batches, pallet_dynamic_evm_fee, benchmarking::dynamic_evm_fee);
			orml_add_benchmark!(params, batches, pallet_xyk_liquidity_mining, benchmarking::xyk_liquidity_mining);
			orml_add_benchmark!(params, batches, pallet_omnipool_liquidity_mining, benchmarking::omnipool_liquidity_mining);
			orml_add_benchmark!(params, batches, pallet_ema_oracle, benchmarking::ema_oracle);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<PresetId> {
			Default::default()
		}
	}
}

#[cfg(feature = "runtime-benchmarks")] //Used only for benchmarking pallet_xcm_benchmarks::generic exchane_asset instruction
fn init_omnipool(amount_to_sell: Balance) -> Balance {
	use hydradx_traits::Mutate;
	let caller: AccountId = frame_benchmarking::account("caller", 0, 1);
	let hdx = 0;
	let dai = 2;
	let token_amount = 2000000000000u128 * 1_000_000_000;

	//let loc : MultiLocation = Location::new(1, cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GeneralIndex(dai.into());1]))).into();
	//			polkadot_xcm::opaque::lts::Junctions::X1(Arc::new([polkadot_xcm::opaque::lts::Junction::GeneralIndex(dai.into())]))

	use frame_support::assert_ok;
	use polkadot_xcm::v3::Junction::GeneralIndex;
	use polkadot_xcm::v3::Junctions::X1;
	use polkadot_xcm::v3::MultiLocation;
	assert_ok!(AssetRegistry::set_location(
		dai,
		AssetLocation(MultiLocation::new(0, X1(GeneralIndex(dai.into()))))
	));
	/*
		assert_ok!(AssetRegistry::set_location(
		dai,
		AssetLocation(MultiLocation::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GeneralIndex(dai.into());1]))
		))
	));
	*/

	Currencies::update_balance(
		RuntimeOrigin::root(),
		Omnipool::protocol_account(),
		hdx,
		(token_amount as i128) * 100,
	)
	.unwrap();
	Currencies::update_balance(
		RuntimeOrigin::root(),
		Omnipool::protocol_account(),
		dai,
		(token_amount as i128) * 100,
	)
	.unwrap();
	Currencies::update_balance(RuntimeOrigin::root(), caller.clone(), hdx, token_amount as i128).unwrap();
	Currencies::update_balance(RuntimeOrigin::root(), caller.clone(), dai, token_amount as i128).unwrap();
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	let native_position_id = Omnipool::next_position_id();

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		hdx,
		native_price,
		Permill::from_percent(10),
		caller.clone(),
	));

	let stable_position_id = Omnipool::next_position_id();

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		dai,
		stable_price,
		Permill::from_percent(100),
		caller.clone(),
	));

	assert_ok!(Omnipool::sacrifice_position(
		RuntimeOrigin::signed(caller.clone()),
		native_position_id,
	));

	assert_ok!(Omnipool::sacrifice_position(
		RuntimeOrigin::signed(caller),
		stable_position_id,
	));

	assert_ok!(Referrals::set_reward_percentage(
		RawOrigin::Root.into(),
		0,
		Level::None,
		FeeDistribution::default(),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RawOrigin::Root.into(),
		1,
		Level::None,
		FeeDistribution::default(),
	));

	assert_ok!(Omnipool::set_asset_tradable_state(
		RuntimeOrigin::root(),
		hdx,
		pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
	));

	assert_ok!(Omnipool::set_asset_tradable_state(
		RuntimeOrigin::root(),
		dai,
		pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
	));

	with_transaction::<Balance, DispatchError, _>(|| {
		let caller2: AccountId = frame_benchmarking::account("caller2", 0, 1);
		Currencies::update_balance(RuntimeOrigin::root(), caller2.clone(), hdx, token_amount as i128).unwrap();

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(caller2.clone()),
			hdx,
			dai,
			amount_to_sell,
			0,
			vec![].try_into().unwrap(),
		));
		let received = Currencies::free_balance(dai, &caller2);
		TransactionOutcome::Rollback(Ok(received))
	})
	.unwrap()
}
