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

pub mod dev;
pub mod hydradx;
pub mod local;
pub mod staging;
pub mod testnet;

use cumulus_primitives_core::ParaId;
use hydradx_runtime::{
	AccountId, AuraId, Balance, BalancesConfig, CollatorSelectionConfig, GenesisConfig, ParachainInfoConfig,
	SessionConfig, Signature, SudoConfig, SystemConfig, UNITS, WASM_BINARY,
};
use primitives::{AssetId, BlockNumber, Price};
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

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecExtension, ChainSpecGroup)]
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

pub fn parachain_genesis(
	wasm_binary: &[u8],
	root_key: AccountId,
	initial_authorities: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, Balance)>,
	_enable_println: bool,
	parachain_id: ParaId,
	_council_members: Vec<AccountId>,
	_tech_committee_members: Vec<AccountId>,
	_tx_fee_payment_account: AccountId, // Account use multi-payment pallet to send fees to in pool does not exists
	_vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	_registered_assets: Vec<(Vec<u8>, Balance)>, // (Asset name, Existential deposit)
	_accepted_assets: Vec<(AssetId, Price)>,     // (Asset id, Fallback price) - asset which fee can be paid with
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of a lot.
			balances: endowed_accounts.iter().cloned().map(|k| (k.0, k.1 * UNITS)).collect(),
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: 10_000 * UNITS,
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
