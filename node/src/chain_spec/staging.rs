use super::*;

use hex_literal::hex;
use sc_telemetry::TelemetryEndpoints;

// The URL for the telemetry server.
const TELEMETRY_URLS: [&str; 2] = [
	"wss://telemetry.polkadot.io/submit/",
	"wss://telemetry.hydradx.io:9000/submit/",
];

pub fn parachain_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	let genesis_json = parachain_genesis(
		// Sudo account
		// Galactic Council
		// 7HqdGVRB4MXz1osLR77mfWoo536cWasTYsuAbVuicHdiKQXf
		hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into(),
		// initial authorities & invulnerable collators
		(
			vec![
				(
					// 5G3t6yhAonQHGUEqrByWQPgP9R8fcSSL6Vujphc89ysdTpKF
					hex!["b0502e92d738d528922e8963b8a58a3c7c3b693db51b0972a6981836d67b8835"].into(),
					hex!["b0502e92d738d528922e8963b8a58a3c7c3b693db51b0972a6981836d67b8835"].unchecked_into(),
				),
				(
					// 5CVBHPAjhcVVAvL3AYpa9MB6kWDwoJbBwu7q4MqbhKwNnrV4
					hex!["12aa36d6c1b055b9a7ab5d39f4fd9a9fe42912163c90e122fb7997e890a53d7e"].into(),
					hex!["12aa36d6c1b055b9a7ab5d39f4fd9a9fe42912163c90e122fb7997e890a53d7e"].unchecked_into(),
				),
				(
					// 5DFGmHjpxS6Xveg4YDw2hSp62JJ9h8oLCkeZUAoVR7hVtQ3k
					hex!["344b7693389189ad0be0c83630b02830a568f7cb0f2d4b3483bcea323cc85f70"].into(),
					hex!["344b7693389189ad0be0c83630b02830a568f7cb0f2d4b3483bcea323cc85f70"].unchecked_into(),
				),
				(
					// 5H178NL4DLM9DGgAgZz1kbrX2TReP3uPk7svPtsg1VcYnuXH
					hex!["da6e859211b1140369a73af533ecea4e4c0e985ad122ac4c663cc8b81d4fcd12"].into(),
					hex!["da6e859211b1140369a73af533ecea4e4c0e985ad122ac4c663cc8b81d4fcd12"].unchecked_into(),
				),
				(
					// 5Ca1iV2RNV253FzYJo12XtKJMPWCjv5CsPK9HdmwgJarD1sJ
					hex!["165a3c2eb21341bf170fd1fa728bd9a7d02b7dc3b4968a46f2b1d494ee8c2b5d"].into(),
					hex!["165a3c2eb21341bf170fd1fa728bd9a7d02b7dc3b4968a46f2b1d494ee8c2b5d"].unchecked_into(),
				),
			],
			10_000 * UNITS,
		),
		// Pre-funded accounts
		vec![(
			// Galactic Council
			// 7HqdGVRB4MXz1osLR77mfWoo536cWasTYsuAbVuicHdiKQXf
			hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into(),
			1_500_000_000 * UNITS,
		)],
		// technical committee
		vec![],
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
		// parachain ID
		PARA_ID.into(),
		// duster
		DusterConfig {
			// treasury
			account_blacklist: vec![hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into()],
			reward_account: Some(hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into()),
			dust_account: Some(hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into()),
		},
	);

	let chain_spec = ChainSpec::builder(
		wasm_binary,
		Extensions {
			relay_chain: "polkadot".into(),
			para_id: PARA_ID,
			evm_since: 1,
		},
	)
	.with_name("Hydration")
	.with_id("hydra")
	.with_chain_type(ChainType::Live)
	.with_boot_nodes(vec![
		"/dns/p2p-01.hydra.hydradx.io/tcp/30333/p2p/12D3KooWHzv7XVVBwY4EX1aKJBU6qzEjqGk6XtoFagr5wEXx6MsH"
			.parse()
			.unwrap(),
		"/dns/p2p-02.hydra.hydradx.io/tcp/30333/p2p/12D3KooWR72FwHrkGNTNes6U5UHQezWLmrKu6b45MvcnRGK8J3S6"
			.parse()
			.unwrap(),
		"/dns/p2p-03.hydra.hydradx.io/tcp/30333/p2p/12D3KooWFDwxZinAjgmLVgsideCmdB2bz911YgiQdLEiwKovezUz"
			.parse()
			.unwrap(),
	])
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![
			(TELEMETRY_URLS[0].to_string(), 0),
			(TELEMETRY_URLS[1].to_string(), 0),
		])
		.expect("Telemetry url is valid"),
	)
	.with_properties(properties)
	.with_protocol_id(PROTOCOL_ID)
	.with_genesis_config_patch(genesis_json)
	.build();

	Ok(chain_spec)
}
