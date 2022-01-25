use super::*;

const INITIAL_BALANCE: u128 = 10_000;

pub fn parachain_config() -> Result<ChainSpec, String> {
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
			parachain_genesis(
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
				true,
				PARA_ID.into(),
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
			para_id: PARA_ID.into(),
		},
	))
}
