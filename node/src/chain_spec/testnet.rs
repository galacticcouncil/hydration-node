use super::*;

pub fn parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"Hydradx testnet",
		// ID
		"hydradx_testnet",
		ChainType::Live,
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
				// Endowed accounts
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), 1_000_000_000),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), 1_000_000_000),
					(get_account_id_from_seed::<sr25519::Public>("Charlie"), 1_000_000_000),
					(get_account_id_from_seed::<sr25519::Public>("Dave"), 1_000_000_000),
					(get_account_id_from_seed::<sr25519::Public>("Eve"), 1_000_000_000),
					(get_account_id_from_seed::<sr25519::Public>("Ferdie"), 1_000_000_000),
					(
						get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
						1_000_000_000,
					),
					(get_account_id_from_seed::<sr25519::Public>("Bob//stash"), 1_000_000_000),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
						1_000_000_000,
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
						1_000_000_000,
					),
					(get_account_id_from_seed::<sr25519::Public>("Eve//stash"), 1_000_000_000),
					(
						get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
						1_000_000_000,
					),
				],
				// enable println
				true,
				// para ID
				PARA_ID.into(),
				//council
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
				//technical_committe
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
				],
				// TX fee payment account
				get_account_id_from_seed::<sr25519::Public>("Alice"), // SAME AS ROOT
				// vesting
				vec![],
				// registered assets
				vec![(b"KSM".to_vec(), 1_000u128), (b"KUSD".to_vec(), 1_000u128)],
				// accepted assets
				vec![(1, Price::from_float(0.0000212)), (2, Price::from_float(0.000806))],
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
