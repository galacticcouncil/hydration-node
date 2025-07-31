use super::*;

const INITIAL_BALANCE: u128 = 10_000;
const INITIAL_TOKEN_BALANCE: Balance = 1_000 * UNITS;

pub fn parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	let genesis_json = parachain_genesis(
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
			10_000 * UNITS,
		),
		// Pre-funded accounts
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
		// technical committee members
		vec![
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_account_id_from_seed::<sr25519::Public>("Bob"),
			get_account_id_from_seed::<sr25519::Public>("Eve"),
		],
		// vestings
		vec![],
		// registered assets
		vec![
			(
				Some(1),
				Some(b"KSM".to_vec().try_into().expect("Name is too long")),
				1_000u128,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"KUSD".to_vec().try_into().expect("Name is too long")),
				1_000u128,
				None,
				None,
				None,
				true,
			),
		],
		// accepted assets
		vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
		// token balances
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
		// claims data
		create_testnet_claims(),
		// parachain ID
		PARA_ID.into(),
		DusterConfig {
			account_blacklist: vec![get_account_id_from_seed::<sr25519::Public>("Duster")],
			reward_account: Some(get_account_id_from_seed::<sr25519::Public>("Duster")),
			dust_account: Some(get_account_id_from_seed::<sr25519::Public>("Duster")),
		},
	);

	let chain_spec = ChainSpec::builder(
		wasm_binary,
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
			evm_since: 1,
		},
	)
	.with_name("Hydration Local Testnet")
	.with_id("local_testnet")
	.with_chain_type(ChainType::Local)
	.with_boot_nodes(vec![])
	.with_properties(properties)
	.with_protocol_id(PROTOCOL_ID)
	.with_genesis_config_patch(genesis_json)
	.build();

	Ok(chain_spec)
}
