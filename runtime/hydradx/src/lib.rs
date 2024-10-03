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
pub mod weights;

pub mod apis;
mod assets;
pub mod evm;
mod governance;
mod system;
pub mod types;
pub mod xcm;

pub use assets::*;
pub use governance::*;
use pallet_asset_registry::AssetType;
use pallet_currencies_rpc_runtime_api::AccountData;
pub use system::*;
pub use xcm::*;

use codec::{Decode, Encode};
use hydradx_traits::evm::InspectEvmAccounts;
use sp_core::{ConstU128, Get, H160, H256, U256};
use sp_genesis_builder::PresetId;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, PostDispatchInfoOf,
		UniqueSaturatedInto,
	},
	transaction_validity::{TransactionValidity, TransactionValidityError},
	Permill,
};

use sp_std::{convert::From, prelude::*};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
// A few exports that help ease life for downstream crates.
use crate::evm::precompiles::erc20_mapping::SetCodeForErc20Precompile;
use frame_support::{construct_runtime, pallet_prelude::Hooks, weights::Weight};
pub use hex_literal::hex;
use orml_traits::MultiCurrency;
/// Import HydraDX pallets
pub use pallet_claims;
use pallet_ethereum::{Transaction as EthereumTransaction, TransactionStatus};
use pallet_evm::{Account as EVMAccount, FeeCalculator, GasWeightMapping, Runner};
pub use pallet_genesis_history::Chain;
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
	spec_version: 261,
	impl_version: 0,
	apis: apis::RUNTIME_API_VERSIONS,
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

		// Warehouse - let's allocate indices 100+ for warehouse pallets
		EmaOracle: pallet_ema_oracle = 202,
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
	(
		pallet_collator_selection::migration::v2::MigrationToV2<Runtime>,
		SetCodeForErc20Precompile,
	),
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
		pub const BenchmarkMaxBalance: crate::Balance = crate::Balance::max_value();
	}
	frame_benchmarking::define_benchmarks!(
		[pallet_lbp, LBP]
		[pallet_asset_registry, AssetRegistry]
		[pallet_omnipool_liquidity_mining, OmnipoolLiquidityMining]
		[pallet_transaction_pause, TransactionPause]
		[pallet_ema_oracle, EmaOracle]
		[pallet_circuit_breaker, CircuitBreaker]
		[pallet_bonds, Bonds]
		[pallet_stableswap, Stableswap]
		[pallet_claims, Claims]
		[pallet_staking, Staking]
		[pallet_referrals, Referrals]
		[pallet_evm_accounts, EVMAccounts]
		[pallet_otc, OTC]
		[pallet_otc_settlements, OtcSettlements]
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
	);
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
