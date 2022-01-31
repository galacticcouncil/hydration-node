use crate::chain_spec::*;
use testing_hydra_dx_runtime as testing_runtime;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<testing_runtime::GenesisConfig>;

fn testing_session_keys(
	grandpa: GrandpaId,
	babe: BabeId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> testing_runtime::opaque::SessionKeys {
	testing_runtime::opaque::SessionKeys {
		grandpa,
		babe,
		im_online,
		authority_discovery,
	}
}

pub fn development_config() -> Result<ChainSpec, String> {
	let wasm_binary =
		testing_runtime::WASM_BINARY.ok_or_else(|| "Testing and development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("tokenSymbol".into(), "HDX".into());
	properties.insert("ss58Format".into(), 63.into());

	Ok(ChainSpec::from_genesis(
		// Config names for the testing runtime have to start with `Testing` string literal
		// because ChainSpecs are used to identify runtimes.
		// Name
		"Testing HydraDX Development chain",
		// ID
		"dev",
		ChainType::Development,
		move || {
			testnet_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					// Treasury
					hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
				],
				true,
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(DEFAULT_PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		None,
	))
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary =
		testing_runtime::WASM_BINARY.ok_or_else(|| "Development wasm binary not available".to_string())?;

	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("tokenSymbol".into(), "HDX".into());
	properties.insert("ss58Format".into(), 63.into());

	Ok(ChainSpec::from_genesis(
		// Config names for the testing runtime have to start with `Testing` string literal
		// because ChainSpecs are used to identify runtimes.
		// Name
		"Testing HydraDX Local Testnet",
		// ID
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
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
					// Treasury
					hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
				],
				true,
			)
		},
		// Bootnodes
		vec![],
		// Telemetry
		None,
		// Protocol ID
		Some(DEFAULT_PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		None,
	))
}

fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		BabeId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
) -> testing_runtime::GenesisConfig {
	testing_runtime::GenesisConfig {
		system: testing_runtime::SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		balances: testing_runtime::BalancesConfig {
			// Configure endowed accounts with initial balance of 1_000_000.
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1_000_000u128 * HDX))
				.collect(),
		},
		grandpa: testing_runtime::GrandpaConfig { authorities: vec![] },
		sudo: testing_runtime::SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		asset_registry: testing_runtime::AssetRegistryConfig {
			core_asset_id: CORE_ASSET_ID,
			asset_ids: vec![
				(b"tKSM".to_vec(), 1),
				(b"tDOT".to_vec(), 2),
				(b"tETH".to_vec(), 3),
				(b"tACA".to_vec(), 4),
				(b"tEDG".to_vec(), 5),
				(b"tUSD".to_vec(), 6),
				(b"tPLM".to_vec(), 7),
				(b"tFIS".to_vec(), 8),
				(b"tPHA".to_vec(), 9),
				(b"tUSDT".to_vec(), 10),
			],
			next_asset_id: 11,
		},
		multi_transaction_payment: testing_runtime::MultiTransactionPaymentConfig {
			currencies: vec![],
			fallback_account: hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
			account_currencies: vec![],
		},
		tokens: testing_runtime::TokensConfig {
			balances: endowed_accounts
				.iter()
				.flat_map(|x| {
					vec![
						(x.clone(), 1, 100_000u128 * HDX),
						(x.clone(), 2, 100_000u128 * HDX),
						(x.clone(), 3, 100_000u128 * HDX),
					]
				})
				.collect(),
		},
		faucet: testing_runtime::FaucetConfig {
			rampage: true,
			mint_limit: 5,
			mintable_currencies: vec![0, 1, 2],
		},
		babe: testing_runtime::BabeConfig {
			authorities: vec![],
			epoch_config: Some(hydra_dx_runtime::BABE_GENESIS_EPOCH_CONFIG),
		},
		authority_discovery: testing_runtime::AuthorityDiscoveryConfig { keys: vec![] },
		im_online: testing_runtime::ImOnlineConfig { keys: vec![] },
		treasury: Default::default(),
		session: testing_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						testing_session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		staking: testing_runtime::StakingConfig {
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		elections: testing_runtime::ElectionsConfig {
			members: vec![
				(get_account_id_from_seed::<sr25519::Public>("Alice"), STASH / 5),
				(get_account_id_from_seed::<sr25519::Public>("Bob"), STASH / 5),
				(get_account_id_from_seed::<sr25519::Public>("Eve"), STASH / 5),
			],
		},
		council: testing_runtime::CouncilConfig {
			members: vec![
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_account_id_from_seed::<sr25519::Public>("Eve"),
			],
			phantom: Default::default(),
		},
		technical_committee: testing_runtime::TechnicalCommitteeConfig {
			members: vec![
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_account_id_from_seed::<sr25519::Public>("Eve"),
			],
			phantom: Default::default(),
		},
		claims: testing_runtime::ClaimsConfig {
			claims: create_testnet_claims(),
		},
		genesis_history: testing_runtime::GenesisHistoryConfig::default(),
		vesting: testing_runtime::VestingConfig { vesting: vec![] },
	}
}
