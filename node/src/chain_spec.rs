// This file is part of HydraDX-node.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

#![allow(clippy::or_fun_call)]
#![allow(clippy::too_many_arguments)]

use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use hydradx_runtime::{
	AccountId, AuraId, Balance, BalancesConfig, CollatorSelectionConfig, GenesisConfig, ParachainInfoConfig,
	SessionConfig, Signature, SudoConfig, SystemConfig, UNITS, WASM_BINARY,
};
use primitives::{AssetId, BlockNumber, Price};
use sc_chain_spec::ChainSpecExtension;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use serde_json::map::Map;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

const TOKEN_DECIMALS: u8 = 12;
const TOKEN_SYMBOL: &str = "HDX";
const PROTOCOL_ID: &str = "hdx";
// The URL for the telemetry server.
const TELEMETRY_URLS: [&str; 2] = [
	"wss://telemetry.polkadot.io/submit/",
	"wss://telemetry.hydradx.io:9000/submit/",
];
//Kusama parachain id
const PARA_ID: u32 = 2090;

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	#[allow(clippy::borrowed_box)]
	pub fn try_get(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn hydradx_parachain_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../res/hydradx.json")[..])
}

pub fn kusama_staging_parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX",
		// ID
		"hydradx",
		ChainType::Live,
		move || {
			parachain_genesis(
				wasm_binary,
				// Sudo account
				hex!["bca8eeb9c7cf74fc28ebe4091d29ae1c12ed622f7e3656aae080b54d5ff9a23c"].into(), //TODO intergalactic
				//initial authorities & invulnerables
				vec![
					(
						hex!["f25e5d7b43266a5b4cca762c9be917f18852d7a5db85e734776206eeb539dd4f"].into(),
						hex!["f25e5d7b43266a5b4cca762c9be917f18852d7a5db85e734776206eeb539dd4f"].unchecked_into(),
					),
					(
						hex!["e84a7090cb18fe39eafebdae9a3ac1111c955247a202a3ab2a3cfe8573c03c60"].into(),
						hex!["e84a7090cb18fe39eafebdae9a3ac1111c955247a202a3ab2a3cfe8573c03c60"].unchecked_into(),
					),
					(
						hex!["c49e3fbebac92027e0d19c2fc1ddc288eb549971831e336550832a476727f601"].into(),
						hex!["c49e3fbebac92027e0d19c2fc1ddc288eb549971831e336550832a476727f601"].unchecked_into(),
					),
					(
						hex!["c856aabea6e433be2dfe233c6118d156133e4e663a1223da06421058ddb56712"].into(),
						hex!["c856aabea6e433be2dfe233c6118d156133e4e663a1223da06421058ddb56712"].unchecked_into(),
					),
					(
						hex!["e02a753fc885bde7ea5839df8619ab80b67be6c869bc19b41f20f865a2f90578"].into(),
						hex!["e02a753fc885bde7ea5839df8619ab80b67be6c869bc19b41f20f865a2f90578"].unchecked_into(),
					),
				],
				// Pre-funded accounts
				vec![],
				true,
				PARA_ID.into(),
				//technical committee
				hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(), // TREASURY - Fallback for multi tx payment
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		Some(
			TelemetryEndpoints::new(vec![
				(TELEMETRY_URLS[0].to_string(), 0),
				(TELEMETRY_URLS[1].to_string(), 0),
			])
			.expect("Telemetry url is valid"),
		),
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "kusama".into(),
			para_id: PARA_ID,
		},
	))
}

pub fn testnet_parachain_config(para_id: ParaId) -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Egg",
		// ID
		"hydradx_egg",
		ChainType::Live,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into(),
				//initial authorities & invulnerables
				vec![
					(
						hex!["da0fa4ab419def66fb4ac5224e594e82c34ee795268fc7787c8a096c4ff14f11"].into(),
						hex!["da0fa4ab419def66fb4ac5224e594e82c34ee795268fc7787c8a096c4ff14f11"].unchecked_into(),
					),
					(
						hex!["ecd7a5439c6ab0cd6550bc2f1cef5299d425bb95bb6d7afb32aa3d95ee4f7f1f"].into(),
						hex!["ecd7a5439c6ab0cd6550bc2f1cef5299d425bb95bb6d7afb32aa3d95ee4f7f1f"].unchecked_into(),
					),
					(
						hex!["f0ad6f1aae7a445c1e80cac883096ec8177eda276fec53ad9ccbe570f3090a26"].into(),
						hex!["f0ad6f1aae7a445c1e80cac883096ec8177eda276fec53ad9ccbe570f3090a26"].unchecked_into(),
					),
				],
				// Pre-funded accounts
				vec![hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into()],
				true,
				para_id,
				//council
				vec![hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into()],
				//technical committee
				vec![hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into()],
				hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into(), // SAME AS ROOT
				vec![],
				vec![(b"KSM".to_vec(), 1_000u128), (b"KUSD".to_vec(), 1_000u128)],
				vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		Some(
			TelemetryEndpoints::new(vec![
				(TELEMETRY_URLS[0].to_string(), 0),
				(TELEMETRY_URLS[1].to_string(), 0),
			])
			.expect("Telemetry url is valid"),
		),
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "westend".into(),
			para_id: para_id.into(),
		},
	))
}

pub fn parachain_development_config(para_id: ParaId) -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Development",
		// ID
		"dev",
		ChainType::Development,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				//initial authorities & invulnerables
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
				],
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Duster"),
				],
				true,
				para_id,
				//council
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
				//technical_committe
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				get_account_id_from_seed::<sr25519::Public>("Alice"), // SAME AS ROOT
				vec![],
				vec![(b"KSM".to_vec(), 1_000u128), (b"KUSD".to_vec(), 1_000u128)],
				vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo-dev".into(),
			para_id: para_id.into(),
		},
	))
}

// This is used when benchmarking pallets
// Originally dev config was used - but benchmarking needs empty asset registry
pub fn benchmarks_development_config(para_id: ParaId) -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Benchmarks",
		// ID
		"benchmarks",
		ChainType::Development,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				//initial authorities & invulnerables
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
				],
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Duster"),
				],
				true,
				para_id,
				//council
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
				//technical_committe
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				get_account_id_from_seed::<sr25519::Public>("Alice"), // SAME AS ROOT
				vec![],
				vec![],
				vec![],
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo-dev".into(),
			para_id: para_id.into(),
		},
	))
}

pub fn local_parachain_config(para_id: ParaId) -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Local Testnet",
		// ID
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				//initial authorities & invulnerables
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
				],
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
				true,
				para_id,
				//council
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
				//technical_committe
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				get_account_id_from_seed::<sr25519::Public>("Alice"), // SAME AS ROOT
				vec![],
				vec![(b"KSM".to_vec(), 1_000u128), (b"KUSD".to_vec(), 1_000u128)],
				vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: para_id.into(),
		},
	))
}

/// Configure initial storage state for FRAME modules.
fn parachain_genesis(
	wasm_binary: &[u8],
	root_key: AccountId,
	initial_authorities: Vec<(AccountId, AuraId)>,
	_endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
	parachain_id: ParaId,
	tx_fee_payment_account: AccountId,
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of a lot.
			balances: vec![
				(
					// Intergalactic HDX Tokens 15%
					hex!["bca8eeb9c7cf74fc28ebe4091d29ae1c12ed622f7e3656aae080b54d5ff9a23c"].into(),
					15_000_000_000u128 * UNITS,
				),
				(
					// Treasury 9%
					hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
					9_000_000_000 * UNITS,
				),
			],
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: 10_000,
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                                   // account id
						acc,                                           // validator id
						hydradx_runtime::opaque::SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},

		// no need to pass anything, it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		treasury: Default::default(),
		parachain_info: ParachainInfoConfig { parachain_id },
		aura_ext: Default::default(),
	}
}

fn testnet_parachain_genesis(
	wasm_binary: &[u8],
	root_key: AccountId,
	initial_authorities: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
	parachain_id: ParaId,
	council_members: Vec<AccountId>,
	tech_committee_members: Vec<AccountId>,
	tx_fee_payment_account: AccountId,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	registered_assets: Vec<(Vec<u8>, Balance)>, // (Asset name, Existential deposit)
	accepted_assets: Vec<(AssetId, Price)>,     // (Asset id, Fallback price) - asset which fee can be paid with
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of a lot.
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1_000_000_000u128 * UNITS))
				.collect(),
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: 10_000,
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                                   // account id
						acc,                                           // validator id
						hydradx_runtime::opaque::SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},

		// no need to pass anything, it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		treasury: Default::default(),
		parachain_info: ParachainInfoConfig { parachain_id },
		aura_ext: Default::default(),
	}
}
