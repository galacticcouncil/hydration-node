use super::*;

use hex_literal::hex;
use sc_telemetry::TelemetryEndpoints;

// The URL for the telemetry server.
const _TELEMETRY_URLS: [&str; 2] = [
	"wss://telemetry.polkadot.io/submit/",
	"wss://telemetry.hydradx.io:9000/submit/",
];

pub fn parachain_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../res/rococo.json")[..])
}

pub fn _parachain_config_rococo() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX testnet",
		// ID
		"hydra_rococo",
		ChainType::Live,
		move || {
			parachain_genesis(
				wasm_binary,
				// Sudo account
				// Galactic Council
				// 7JcAAB6cXQxVQyVLksPUdthJwcoEGm8SW9hsNgdP6hjme5J1
				hex!["2cb1a0ef4ce819893905e3a6a8e46b652c43aee6c154921220902cabfdcfdd07"].into(),
				// initial authorities & invulnerable collators
				(
					vec![
						(
							// 5DvFqJq182asuR9EKoBBJBGEnZwpMHvuBrYpiMdr4hS8B6Eh
							hex!["5206e6a18c96bab98f459ab636226481699220cf94346d766cd1142557a2fc66"].into(),
							hex!["5206e6a18c96bab98f459ab636226481699220cf94346d766cd1142557a2fc66"].unchecked_into(),
						),
						(
							// 5Fbc5bQp1bHfNpx8yuTshYK1pNhBbajZyaosRNvuyM3KRiMz
							hex!["9c45dc3b15cd55531cad1e4c21cacb47611be54e3da6bf5be451f5e578a68344"].into(),
							hex!["9c45dc3b15cd55531cad1e4c21cacb47611be54e3da6bf5be451f5e578a68344"].unchecked_into(),
						),
						(
							// 5CAP1unPwP9RnvNcJn7YSZy1A8Snv5fGebuWMU99vpaJcfjh
							hex!["04540c9406af2f5bf34f948dd1c7f029892247c2e1472b7cd51c188e4e0c2f2b"].into(),
							hex!["04540c9406af2f5bf34f948dd1c7f029892247c2e1472b7cd51c188e4e0c2f2b"].unchecked_into(),
						),
						(
							// 5GrzdLdMFuV6ZvWQjYUhVEpZQFoSxkLboKY3xeEdEjNUPoH7
							hex!["d43ead05edb218c199fc0dfe36ec9389f509d8447e43109e1cb04305de4f9359"].into(),
							hex!["d43ead05edb218c199fc0dfe36ec9389f509d8447e43109e1cb04305de4f9359"].unchecked_into(),
						),
						(
							// 5G9gRv1GE8HPsjXnNm8By2SWuasTan5gU2AZm8pundyduHTM
							hex!["b4bc5b99d9207ab2aadd222581809b53372e7e17c4eaa88742f3501f3044bf27"].into(),
							hex!["b4bc5b99d9207ab2aadd222581809b53372e7e17c4eaa88742f3501f3044bf27"].unchecked_into(),
						),
					],
					10_000 * UNITS,
				),
				// Pre-funded accounts
				vec![(
					// Galactic Council
					// 7JcAAB6cXQxVQyVLksPUdthJwcoEGm8SW9hsNgdP6hjme5J1
					hex!["2cb1a0ef4ce819893905e3a6a8e46b652c43aee6c154921220902cabfdcfdd07"].into(),
					1_500_000_000,
				)],
				// council members
				// GC - same as sudo
				vec![hex!["2cb1a0ef4ce819893905e3a6a8e46b652c43aee6c154921220902cabfdcfdd07"].into()],
				// technical committee
				// GC - same as sudo
				vec![hex!["2cb1a0ef4ce819893905e3a6a8e46b652c43aee6c154921220902cabfdcfdd07"].into()],
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
				vec![(
					hex!["2cb1a0ef4ce819893905e3a6a8e46b652c43aee6c154921220902cabfdcfdd07"].into(),
					1_200_000_000 * UNITS,
				)],
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
		vec![
			"/dns/rococo-hydradx-p2p01.hydration.dev/tcp/30333/p2p/12D3KooWCtBQpwnWV7yMaEyBRkcAcAej78Q2uZawk5RcDrYktVQS"
				.parse()
				.unwrap(),
			"/dns/rococo-hydradx-p2p02.hydration.dev/tcp/30333/p2p/12D3KooWLfojmwK6cAFDhzewCjUsyzYAKrpL4Ze42D1bLo8gvS4j"
				.parse()
				.unwrap(),
			"/dns/rococo-hydradx-p2p03.hydration.dev/tcp/30333/p2p/12D3KooWEuEVHGrntL4Anje1k9z85V7thwhgPd9WnX4EJP5i13Xc"
				.parse()
				.unwrap(),
		],
		// Telemetry
		Some(
			TelemetryEndpoints::new(vec![
				(_TELEMETRY_URLS[0].to_string(), 0),
				(_TELEMETRY_URLS[1].to_string(), 0),
			])
			.expect("Telemetry url is valid"),
		),
		// Protocol ID
		Some(PROTOCOL_ID),
		// Fork ID
		None,
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo".into(),
			para_id: PARA_ID,
			evm_since: 1,
		},
	))
}
