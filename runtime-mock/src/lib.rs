mod accounts;
mod omnipool;
mod registry;
mod stableswap;
mod staking;
pub mod traits;

use accounts::{
	get_council_members, get_duster_dest_account, get_duster_reward_account, get_native_endowed_accounts,
	get_nonnative_endowed_accounts, get_omnipool_position_owner, get_technical_committee,
};
use hydradx_runtime::Runtime as MockedRuntime;
use hydradx_runtime::*;
use omnipool::omnipool_initial_state;
use primitives::constants::currency::UNITS;
use registry::AssetRegistrySetup;
use sp_io::TestExternalities;
use sp_runtime::{traits::Dispatchable, Storage};
use stableswap::stablepools;

#[cfg(test)]
mod tests;

const PARA_ID: u32 = 2034;

pub fn hydradx_mocked_runtime() -> TestExternalities {
	let asset_registry_setup = AssetRegistrySetup::new();

	// Omnipool state
	let omnipool_account = pallet_omnipool::Pallet::<MockedRuntime>::protocol_account();
	let omnipool_setup = omnipool_initial_state();
	let stableswap_pool = stablepools();
	let stablepools_creator: AccountId = [222; 32].into();
	let stable_account_balacnes = stableswap_pool.endowed_account(stablepools_creator.clone());

	let (omnipool_native_balance, omnipool_balances) = omnipool_setup.get_omnipool_reserves();

	// Staking
	let staking_initial = staking::staking_state();

	// Endowed accounts - Native and non-native
	let mut native_endowed_accounts = get_native_endowed_accounts();
	// Extend with omnipool initial state of HDX
	native_endowed_accounts.push((omnipool_account.clone(), omnipool_native_balance));
	native_endowed_accounts.extend(staking_initial.get_native_endowed_accounts());

	let mut non_native_endowed_accounts = get_nonnative_endowed_accounts(asset_registry_setup.assets.clone());
	// Extend with omnipool initial state of each asset in omnipool
	non_native_endowed_accounts.push((omnipool_account, omnipool_balances));
	non_native_endowed_accounts.extend(stable_account_balacnes);

	let storage: Storage = {
		use sp_runtime::app_crypto::ByteArray;
		use sp_runtime::BuildStorage;

		let initial_authorities: Vec<(AccountId, AuraId)> = vec![
			([0; 32].into(), AuraId::from_slice(&[0; 32]).unwrap()),
			([1; 32].into(), AuraId::from_slice(&[1; 32]).unwrap()),
		];

		//TODO: dump from HydraDX production too
		let accepted_assets: Vec<(AssetId, Price)> =
			vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))];

		let token_balances: Vec<(AccountId, Vec<(AssetId, Balance)>)> = non_native_endowed_accounts;

		RuntimeGenesisConfig {
			system: Default::default(),
			session: SessionConfig {
				keys: initial_authorities
					.iter()
					.map(|x| {
						(
							x.0.clone(),
							x.0.clone(),
							hydradx_runtime::opaque::SessionKeys { aura: x.1.clone() },
						)
					})
					.collect::<Vec<_>>(),
				non_authority_keys: Default::default(),
			},
			aura: Default::default(),
			collator_selection: CollatorSelectionConfig {
				invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
				candidacy_bond: 10_000 * UNITS,
				..Default::default()
			},
			balances: BalancesConfig {
				balances: native_endowed_accounts,
			},
			council: CouncilConfig {
				members: get_council_members(),
				phantom: Default::default(),
			},
			technical_committee: TechnicalCommitteeConfig {
				members: get_technical_committee(),
				phantom: Default::default(),
			},
			vesting: VestingConfig { vesting: vec![] },
			asset_registry: asset_registry_setup.config(),
			multi_transaction_payment: MultiTransactionPaymentConfig {
				currencies: accepted_assets,
				account_currencies: vec![],
			},
			tokens: TokensConfig {
				balances: token_balances
					.iter()
					.flat_map(|x| {
						x.1.clone()
							.into_iter()
							.map(|(asset_id, amount)| (x.0.clone(), asset_id, amount))
					})
					.collect(),
			},
			treasury: Default::default(),
			elections: Default::default(),
			genesis_history: GenesisHistoryConfig::default(),
			claims: ClaimsConfig {
				claims: Default::default(),
			},
			parachain_info: ParachainInfoConfig {
				parachain_id: PARA_ID.into(),
				..Default::default()
			},
			aura_ext: Default::default(),
			polkadot_xcm: Default::default(),
			ema_oracle: Default::default(),
			duster: DusterConfig {
				account_blacklist: vec![],
				reward_account: Some(get_duster_reward_account()),
				dust_account: Some(get_duster_dest_account()),
			},
			omnipool_warehouse_lm: Default::default(),
			omnipool_liquidity_mining: Default::default(),
			evm_chain_id: hydradx_runtime::EVMChainIdConfig {
				chain_id: 2_222_222u32.into(),
				..Default::default()
			},
			ethereum: Default::default(),
			evm: Default::default(),
			xyk_warehouse_lm: Default::default(),
			xyk_liquidity_mining: Default::default(),
		}
		.build_storage()
		.unwrap()
	};

	let mut externalities = TestExternalities::new(storage);

	externalities.execute_with(|| {
		let staking_calls = staking_initial.calls();
		let stableswap_calls = stableswap_pool.calls();
		let omnipool_calls = omnipool_setup.calls(&get_omnipool_position_owner());
		for call in staking_calls
			.into_iter()
			.chain(stableswap_calls.into_iter())
			.chain(omnipool_calls.into_iter())
		{
			call.dispatch(RuntimeOrigin::root()).unwrap();
		}
		let stableswap_liquidity = stableswap_pool.add_liquid_calls();
		for call in stableswap_liquidity.into_iter() {
			call.dispatch(RuntimeOrigin::signed(stablepools_creator.clone()))
				.unwrap();
		}
		let add_stable_tokens = stableswap_pool.add_token_calls(stablepools_creator.clone());
		for call in add_stable_tokens.into_iter() {
			call.dispatch(RuntimeOrigin::root()).unwrap();
		}
	});

	externalities.commit_all().unwrap();

	externalities
}
