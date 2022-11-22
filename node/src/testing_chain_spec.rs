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

use crate::chain_spec::Extensions;
use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use primitives::{constants::currency::NATIVE_EXISTENTIAL_DEPOSIT, AssetId, BlockNumber, Price};
use sc_service::ChainType;
use serde_json::map::Map;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use testing_hydradx_runtime::{
	pallet_claims::EthereumAddress, AccountId, AssetRegistryConfig, AuraId, Balance, BalancesConfig, ClaimsConfig,
	CollatorSelectionConfig, CouncilConfig, ElectionsConfig, GenesisConfig, GenesisHistoryConfig,
	MultiTransactionPaymentConfig, ParachainInfoConfig, SessionConfig, Signature, SudoConfig, SystemConfig,
	TechnicalCommitteeConfig, TokensConfig, VestingConfig, UNITS, WASM_BINARY,
};

const PARA_ID: u32 = 2034;
const TOKEN_DECIMALS: u8 = 12;
const TOKEN_SYMBOL: &str = "HDX";
const PROTOCOL_ID: &str = "hdx";
const STASH: Balance = 100 * UNITS;
const INITIAL_BALANCE: u128 = 1_000_000 * UNITS;
const INITIAL_TOKEN_BALANCE: Balance = 1_000 * UNITS;

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

pub fn local_parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"Testing HydraDX Local Testnet",
		// ID
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				// initial authorities & invulnerables
				(
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
					// candidacy bond
					10_000,
				),
				// pre-funded accounts
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>("Charlie"), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>("Dave"), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>("Eve"), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>("Ferdie"), INITIAL_BALANCE),
					(
						get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
						INITIAL_BALANCE,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
						INITIAL_BALANCE,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
						INITIAL_BALANCE,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
						INITIAL_BALANCE,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
						INITIAL_BALANCE,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
						INITIAL_BALANCE,
					),
				],
				// council
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				// technical_committe
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				vec![],
				vec![(b"KSM".to_vec(), 1_000u128), (b"KUSD".to_vec(), 1_000u128)],
				vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						vec![(1, INITIAL_TOKEN_BALANCE), (2, INITIAL_TOKEN_BALANCE)],
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						vec![(1, INITIAL_TOKEN_BALANCE), (2, INITIAL_TOKEN_BALANCE)],
					),
				],
				create_testnet_claims(),
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), STASH / 5),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), STASH / 5),
					(get_account_id_from_seed::<sr25519::Public>("Eve"), STASH / 5),
				],
				PARA_ID.into(),
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Fork ID
		None,
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
		},
	))
}

pub fn devnet_parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX devnet",
		// ID
		"hydra_devnet",
		ChainType::Live,
		move || {
			testnet_parachain_genesis(
				wasm_binary,
				// Sudo account
				// Galactic Council
				// 5GjfiRa32G5YhQja854QooT6fJimjDJUQhTywSwBSXeKbnsQ
				hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
				// initial authorities & invulnerable collators
				(
					vec![
						(
							// 5GncEYtdyriWMHsMFX25S85hq76Ys4WtLSuoNvZbqCfdj5wd
							hex!["d0e650219621b1bcfbf8f258ee59b2d90e341a24986a71d348bf8318cb8d3a71"].into(),
							hex!["d0e650219621b1bcfbf8f258ee59b2d90e341a24986a71d348bf8318cb8d3a71"].unchecked_into(),
						),
						(
							// 5Ei2oEJUQZVa7TVjixWo9rfDzPpcDzb2tfSkAdVLF1mxNwzh
							hex!["74f04e971f06aceb4ce21d9c75e532e2e740355ba58057e1fb873a519dd6fb4a"].into(),
							hex!["74f04e971f06aceb4ce21d9c75e532e2e740355ba58057e1fb873a519dd6fb4a"].unchecked_into(),
						),
						(
							// 5H757HRp2uYNFGDd8uTry9q8krMrCeadBGK2MgMv9UyCMVeQ
							hex!["defb32da3955b83bd674ab5c1192ea52883482a18c7331654ef97a523b5ca41e"].into(),
							hex!["defb32da3955b83bd674ab5c1192ea52883482a18c7331654ef97a523b5ca41e"].unchecked_into(),
						),
						(
							// 5Chfpu26SchBuXCxsXEbJKJQCU5cbqUJZdp47q6QzQuPUffd
							hex!["1c313e9c1d704a99c25393f72f97c9a3124ef4fcf060496ae558f4d63372c351"].into(),
							hex!["1c313e9c1d704a99c25393f72f97c9a3124ef4fcf060496ae558f4d63372c351"].unchecked_into(),
						),
						(
							// 5GNo4Dm2AnPhnvQWiwh5iU3oHGC4wM9rG2yvbfDFRFfe6qLv
							hex!["bebcda62a44b4e08ee20a1bdb856aeee5c896dca65053c0e45e267fa78b1631c"].into(),
							hex!["bebcda62a44b4e08ee20a1bdb856aeee5c896dca65053c0e45e267fa78b1631c"].unchecked_into(),
						),
					],
					10_000 * UNITS,
				),
				// Pre-funded accounts
				vec![(
					// Galactic Council
					// 5GjfiRa32G5YhQja854QooT6fJimjDJUQhTywSwBSXeKbnsQ
					hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
					1_500_000_000 * UNITS,
				)],
				// council members
				// GC - same as sudo
				vec![hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into()],
				// technical committee
				// GC - same as sudo
				vec![hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into()],
				// vestings
				vec![],
				// registered_assets
				vec![],
				// accepted_assets
				vec![],
				// token balances
				vec![],
				// claims data
				Default::default(),
				// elections
				// GC - same as sudo
				vec![(
					hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
					1_200_000_000 * UNITS,
				)],
				// parachain ID
				PARA_ID.into(),
			)
		},
		// Bootnodes
		vec![
			"/dns/p2p01.hydradx.dev/tcp/30333/p2p/12D3KooWJYVVHudGGvJUQ98cHLKAr47LonxTckP498FMiFD3XfWw"
				.parse()
				.unwrap(),
			"/dns/p2p02.hydradx.dev/tcp/30333/p2p/12D3KooWQ42FDCxisiPZLvbE8JdEoQvcUUp6oaJte41ZGhAsZKHi"
				.parse()
				.unwrap(),
			"/dns/p2p03.hydradx.dev/tcp/30333/p2p/12D3KooWSbDL1xmE1tAUJ4zvUgUBPHni2FA7nNj2ZPWNSBEpMFzS"
				.parse()
				.unwrap(),
		],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Fork ID
		None,
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "westend".into(),
			para_id: PARA_ID,
		},
	))
}

fn testnet_parachain_genesis(
	wasm_binary: &[u8],
	root_key: AccountId,
	initial_authorities: (Vec<(AccountId, AuraId)>, Balance),
	endowed_accounts: Vec<(AccountId, Balance)>,
	council_members: Vec<AccountId>,
	tech_committee_members: Vec<AccountId>,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	registered_assets: Vec<(Vec<u8>, Balance)>, // (Asset name, Existential deposit)
	accepted_assets: Vec<(AssetId, Price)>,     // (Asset id, Fallback price) - asset which fee can be paid with
	token_balances: Vec<(AccountId, Vec<(AssetId, Balance)>)>,
	claims_data: Vec<(EthereumAddress, Balance)>,
	elections: Vec<(AccountId, Balance)>,
	parachain_id: ParaId,
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key),
		},
		session: SessionConfig {
			keys: initial_authorities
				.0
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                                           // account id
						acc,                                                   // validator id
						testing_hydradx_runtime::opaque::SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},

		// no need to pass anything, it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.0.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: initial_authorities.1,
			..Default::default()
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of a lot.
			balances: endowed_accounts.iter().cloned().map(|k| (k.0, k.1)).collect(),
		},
		council: CouncilConfig {
			// Intergalactic council member
			members: council_members,
			phantom: Default::default(),
		},
		technical_committee: TechnicalCommitteeConfig {
			members: tech_committee_members,
			phantom: Default::default(),
		},
		vesting: VestingConfig { vesting: vesting_list },
		asset_registry: AssetRegistryConfig {
			asset_names: registered_assets.clone(),
			native_asset_name: TOKEN_SYMBOL.as_bytes().to_vec(),
			native_existential_deposit: NATIVE_EXISTENTIAL_DEPOSIT,
		},
		multi_transaction_payment: MultiTransactionPaymentConfig {
			currencies: accepted_assets,
			account_currencies: vec![],
		},
		tokens: TokensConfig {
			balances: if registered_assets.is_empty() {
				vec![]
			} else {
				token_balances
					.iter()
					.flat_map(|x| {
						x.1.clone()
							.into_iter()
							.map(|(asset_id, amount)| (x.0.clone(), asset_id, amount))
					})
					.collect()
			},
		},
		treasury: Default::default(),
		elections: ElectionsConfig {
			// Intergalactic elections
			members: elections,
		},

		genesis_history: GenesisHistoryConfig::default(),
		claims: ClaimsConfig { claims: claims_data },
		parachain_info: ParachainInfoConfig { parachain_id },
		aura_ext: Default::default(),
		polkadot_xcm: Default::default(),
	}
}

pub fn create_testnet_claims() -> Vec<(EthereumAddress, Balance)> {
	let mut claims = Vec::<(EthereumAddress, Balance)>::new();

	// Alice's claim
	// Signature: 0xbcae7d4f96f71cf974c173ae936a1a79083af7f76232efbf8a568b7f990eceed73c2465bba769de959b7f6ac5690162b61eb90949901464d0fa158a83022a0741c
	// Message: "I hereby claim all my HDX tokens to wallet:d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
	let claim_address_1 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/0) : 0xdd75dd5f4a9e964d1c4cc929768947859a98ae2c08100744878a4b6b6d853cc0
		EthereumAddress(hex!["8202C0aF5962B750123CE1A9B12e1C30A4973557"]),
		UNITS / 1_000,
	);

	// Bob's claim
	// Signature: 0x60f3d2541b0ff09982f70844a7f645f4681cbbad2f138fee18404c932bd02cb738d577d53ce94cf067bae87a0b6fa1ec532ceea78d71f4e81a9c27193649c6291b
	// Message: "I hereby claim all my HDX tokens to wallet:8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
	let claim_address_2 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/1) : 0x9b5ef380c0a59008df32ba71ab3c7645950f986fc3f43fd4f9dffc8b2b4e7a5d
		EthereumAddress(hex!["8aF7764663644989671A71Abe9738a3cF295f384"]),
		UNITS,
	);

	// Charlie's claim
	// Signature: 0x52485aece74eb503fb998f0ca08bcc283fa731613db213af4e7fe153faed3de97ea0873d3889622b41d2d989a9e2a0bef160cff1ba8845875d4bc15431136a811c
	// Message: "I hereby claim all my HDX tokens to wallet:90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22"
	let claim_address_3 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/2) : 0x653a29ac0c93de0e9f7d7ea2d60338e68f407b18d16d6ff84db996076424f8fa
		EthereumAddress(hex!["C19A2970A13ac19898c47d59Cbd0278D428EBC7c"]),
		1_000 * UNITS,
	);

	claims.push(claim_address_1);
	claims.push(claim_address_2);
	claims.push(claim_address_3);
	claims
}
