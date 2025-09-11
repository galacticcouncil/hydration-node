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
	pallet_claims::EthereumAddress, AccountId, AuraId, Balance, DusterConfig, RegistryStrLimit, Signature, WASM_BINARY,
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
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	BoundedVec,
};

const PARA_ID: u32 = 2034;
const TOKEN_DECIMALS: u8 = 12;
const TOKEN_SYMBOL: &str = "HDX";
const PROTOCOL_ID: &str = "hdx";

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, ChainSpecExtension, ChainSpecGroup)]
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
pub type ChainSpec = sc_service::GenericChainSpec<Extensions>;

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

#[allow(clippy::type_complexity)]
pub fn parachain_genesis(
	_root_key: AccountId,
	initial_authorities: (Vec<(AccountId, AuraId)>, Balance), // (initial auths, candidacy bond)
	endowed_accounts: Vec<(AccountId, Balance)>,
	tech_committee_members: Vec<AccountId>,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	registered_assets: Vec<(
		Option<AssetId>,
		Option<BoundedVec<u8, RegistryStrLimit>>,
		Balance,
		Option<BoundedVec<u8, RegistryStrLimit>>,
		Option<u8>,
		Option<Balance>,
		bool,
	)>, // (asset_id, name, existential deposit, symbol, decimals, xcm_rate_limit, is_sufficient)
	accepted_assets: Vec<(AssetId, Price)>, // (Asset id, Fallback price) - asset which fee can be paid with
	token_balances: Vec<(AccountId, Vec<(AssetId, Balance)>)>,
	claims_data: Vec<(EthereumAddress, Balance)>,
	parachain_id: ParaId,
	duster: DusterConfig,
) -> serde_json::Value {
	serde_json::json!({
	"system": {},
	"session": {
		"keys": initial_authorities
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
			.collect::<Vec<_>>(),
	},
	"aura": {
		"authorities": Vec::<sp_consensus_aura::sr25519::AuthorityId>::new()
	},
	"collatorSelection": {
		"invulnerables": initial_authorities.0.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
		"candidacyBond": initial_authorities.1,
		"desiredCandidates": 0u32,
	},
	"balances": {
		"balances": endowed_accounts
			.iter()
			.cloned()
			.map(|k| (k.0.clone(), k.1 * UNITS))
			.collect::<Vec<_>>(),
	},
	"technicalCommittee": {
		"members": tech_committee_members,
	},
	"vesting": { "vesting": vesting_list },
	"assetRegistry": {
		"registeredAssets": registered_assets.clone(),
		"nativeAssetName": <Vec<u8> as TryInto<BoundedVec<u8, hydradx_runtime::RegistryStrLimit>>>::try_into(TOKEN_SYMBOL.as_bytes().to_vec())
			.expect("Native asset name is too long."),
		"nativeExistentialDeposit": NATIVE_EXISTENTIAL_DEPOSIT,
		"nativeSymbol": <Vec<u8> as TryInto<BoundedVec<u8, hydradx_runtime::RegistryStrLimit>>>::try_into(TOKEN_SYMBOL.as_bytes().to_vec())
			.expect("Native symbol is too long."),
		"nativeDecimals": TOKEN_DECIMALS,
	},
	"multiTransactionPayment": {
		"currencies": accepted_assets,
		"accountCurrencies": Vec::<(AccountId, AssetId)>::new(),
	},
	"tokens": {
		"balances": if registered_assets.is_empty() {
			vec![]
		} else {
			token_balances
				.iter()
				.flat_map(|x| {
					x.1.clone()
						.into_iter()
						.map(|(asset_id, amount)| (x.0.clone(), asset_id, amount))
				})
			.collect::<Vec<_>>()
		},
	},
	"treasury": {
	},
	"genesisHistory": {
		"previousChain": hydradx_runtime::Chain::default()
	},
	"claims": { "claims": claims_data },
	"parachainInfo": {
		"parachainId": parachain_id,
	},
	"auraExt": {
	},
	"polkadotXcm": {
		"safeXcmVersion": hydradx_runtime::xcm::XcmGenesisConfig::<hydradx_runtime::Runtime>::default().safe_xcm_version
	},
	"emaOracle": {
		"initialData": Vec::<(hydradx_runtime::Source, (AssetId, AssetId), Price, hydradx_runtime::Liquidity<Balance>)>::new()
	},
	"duster": {
		"accountBlacklist": duster.account_blacklist,
		"rewardAccount": duster.reward_account,
		"dustAccount": duster.dust_account
	},
	"omnipoolWarehouseLm": {
	},
	"omnipoolLiquidityMining": {
	},
	"evmChainId": {
		"chainId": 2_222_222u64,
	},
	"ethereum": {
	},
	"evm": {
		"accounts": sp_std::collections::btree_map::BTreeMap::<sp_core::H160, hydradx_runtime::evm::EvmGenesisAccount>::new()
	},
	"xykWarehouseLm": {
	},
	"xykLiquidityMining": {
	},
	}
	)
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
