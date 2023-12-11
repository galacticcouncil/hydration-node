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
#![allow(clippy::derive_partial_eq_without_eq)] //Needed due to bug 'https://github.com/rust-lang/rust-clippy/issues/8867'

pub mod hydradx;
pub mod local;
pub mod moonbase;
pub mod rococo;
pub mod staging;

use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use hydradx_runtime::{
	pallet_claims::EthereumAddress, AccountId, AssetRegistryConfig, AuraId, Balance, BalancesConfig, ClaimsConfig,
	CollatorSelectionConfig, CouncilConfig, DusterConfig, ElectionsConfig, GenesisHistoryConfig,
	MultiTransactionPaymentConfig, ParachainInfoConfig, RuntimeGenesisConfig, SessionConfig, Signature, SystemConfig,
	TechnicalCommitteeConfig, TokensConfig, VestingConfig, WASM_BINARY,
};
use primitives::{
	constants::currency::{NATIVE_EXISTENTIAL_DEPOSIT, UNITS},
	AssetId, BlockNumber, Price,
};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use serde_json::map::Map;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

const PARA_ID: u32 = 2034;
const TOKEN_DECIMALS: u8 = 12;
const TOKEN_SYMBOL: &str = "HDX";
const PROTOCOL_ID: &str = "hdx";
const STASH: Balance = 100 * UNITS;

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, ChainSpecExtension, ChainSpecGroup)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
	pub evm_since: BlockNumber,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	#[allow(clippy::borrowed_box)]
	pub fn try_get(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig, Extensions>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{seed}"), None)
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

pub fn parachain_genesis(
	wasm_binary: &[u8],
	_root_key: AccountId,
	initial_authorities: (Vec<(AccountId, AuraId)>, Balance), // (initial auths, candidacy bond)
	endowed_accounts: Vec<(AccountId, Balance)>,
	council_members: Vec<AccountId>,
	tech_committee_members: Vec<AccountId>,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	registered_assets: Vec<(Vec<u8>, Balance, Option<AssetId>)>, // (Asset name, Existential deposit, Chosen asset id)
	accepted_assets: Vec<(AssetId, Price)>, // (Asset id, Fallback price) - asset which fee can be paid with
	token_balances: Vec<(AccountId, Vec<(AssetId, Balance)>)>,
	claims_data: Vec<(EthereumAddress, Balance)>,
	elections: Vec<(AccountId, Balance)>,
	parachain_id: ParaId,
	duster: DusterConfig,
) -> RuntimeGenesisConfig {
	RuntimeGenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.0
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
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.0.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: initial_authorities.1,
			..Default::default()
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of a lot.
			balances: endowed_accounts.iter().cloned().map(|k| (k.0, k.1 * UNITS)).collect(),
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
			registered_assets: registered_assets.clone(),
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
		parachain_info: ParachainInfoConfig {
			parachain_id,
			..Default::default()
		},
		aura_ext: Default::default(),
		polkadot_xcm: Default::default(),
		ema_oracle: Default::default(),
		duster,
		omnipool_warehouse_lm: Default::default(),
		omnipool_liquidity_mining: Default::default(),
		evm_chain_id: hydradx_runtime::EVMChainIdConfig {
			chain_id: 2_222_222u32.into(),
			..Default::default()
		},
		ethereum: Default::default(),
		evm: Default::default(),
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
