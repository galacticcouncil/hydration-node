use super::*;

use hex_literal::hex;
use sc_telemetry::TelemetryEndpoints;

// The URL for the telemetry server.
const _TELEMETRY_URLS: [&str; 1] = ["wss://telemetry.hydradx.io:9000/submit/"];

pub fn parachain_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../res/moonbase.json")[..])
	//_parachain_config()
}

pub fn _parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	// 5Ff7b9jmr82abw4DQY4tYkiK86PDvvrwFPtDf6YAyDJUV67f
	let sudo = hex!["9ef2ef47e76bba5852ade186c9b804829140dcb0ca123bf10a3b01ce20462c78"];
	let collators = vec![
		hex!["2ed11b4fb48995e82d71e609e6931ca2e953bd4279c7ecdcb016eb17658e9548"], // 5D86BFRPyjAd6rbKABEA93RMhEzfJ9BAnaAdvHZMwhWuMFkL
		hex!["e4e267488371cff034cb97bba493f6bbca76a290e4fc8ba2ebdd03767c966e1c"], // 5HEp2jga16m6VWGqWRZR4W1KoEHpDxzr9dXoZy7beHHqiHWH
	];
	let bootnodes = vec![
		"/dns/51.178.65.60/tcp/30333/p2p/12D3KooWFzC43KqqWBTWosAEfWgWGHqcNGysB6ThZEzmDGVJGNHc"
			.parse()
			.unwrap(),
		"/dns/145.239.10.151/tcp/30333/p2p/12D3KooWMFmKPj7kXnoAGNWWfDv45eqjR5s2hfQrTRDn9J42MzBs"
			.parse()
			.unwrap(),
	];

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX testnet",
		// ID
		"hydra_moonbase",
		ChainType::Live,
		move || {
			parachain_genesis(
				wasm_binary,
				// Sudo account
				sudo.into(),
				// initial authorities & invulnerable collators
				(
					vec![
						(collators[0].into(), collators[0].unchecked_into()),
						(collators[1].into(), collators[1].unchecked_into()),
					],
					10_000 * UNITS,
				),
				// Pre-funded accounts
				vec![(sudo.into(), 1_500_000_000)],
				// council members
				vec![sudo.into()],
				// technical committee
				vec![sudo.into()],
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
				vec![(sudo.into(), 1_200_000_000 * UNITS)],
				// parachain ID
				PARA_ID.into(),
				// duster
				DusterConfig {
					// treasury
					account_blacklist: vec![
						hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into()
					],
					reward_account: Some(
						hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
					),
					dust_account: Some(hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into()),
				},
			)
		},
		// Bootnodes
		bootnodes,
		// Telemetry
		Some(TelemetryEndpoints::new(vec![(_TELEMETRY_URLS[0].to_string(), 0)]).expect("Telemetry url is valid")),
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
