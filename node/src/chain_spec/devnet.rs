use super::*;

use hex_literal::hex;
use sc_telemetry::TelemetryEndpoints;

// The URL for the telemetry server.
const _TELEMETRY_URLS: [&str; 2] = [
	"wss://telemetry.polkadot.io/submit/",
	"wss://telemetry.hydradx.io:9000/submit/",
];

// pub fn _parachain_config() -> Result<ChainSpec, String> {
// 	ChainSpec::from_json_bytes(&include_bytes!("../../res/devnet.json")[..])
// }

pub fn parachain_config_devnet() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), TOKEN_DECIMALS.into());
	properties.insert("tokenSymbol".into(), TOKEN_SYMBOL.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX devnet",
		// ID
		"hydra_devnet",
		ChainType::Live,
		move || {
			parachain_genesis(
				wasm_binary,
				// Sudo account
				// Galactic Council
				// 5GjfiRa32G5YhQja854QooT6fJimjDJUQhTywSwBSXeKbnsQ
				hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
				// initial authorities & invulnerable collators
				(
					vec![
						(
							// 5GncEYtdyriWMHsMFX25S85hq76Ys4WtLSuoNvZbqCfdj5wd
							hex!["d0e650219621b1bcfbf8f258ee59b2d90e341a24986a71d348bf8318cb8d3a71"].into(),
							hex!["d0e650219621b1bcfbf8f258ee59b2d90e341a24986a71d348bf8318cb8d3a71"].unchecked_into(),
						),
						(
							// 5Ei2oEJUQZVa7TVjixWo9rfDzPpcDzb2tfSkAdVLF1mxNwzh
							hex!["74f04e971f06aceb4ce21d9c75e532e2e740355ba58057e1fb873a519dd6fb4a"].into(),
							hex!["74f04e971f06aceb4ce21d9c75e532e2e740355ba58057e1fb873a519dd6fb4a"].unchecked_into(),
						),
						(
							// 5H757HRp2uYNFGDd8uTry9q8krMrCeadBGK2MgMv9UyCMVeQ
							hex!["defb32da3955b83bd674ab5c1192ea52883482a18c7331654ef97a523b5ca41e"].into(),
							hex!["defb32da3955b83bd674ab5c1192ea52883482a18c7331654ef97a523b5ca41e"].unchecked_into(),
						),
						(
							// 5Chfpu26SchBuXCxsXEbJKJQCU5cbqUJZdp47q6QzQuPUffd
							hex!["1c313e9c1d704a99c25393f72f97c9a3124ef4fcf060496ae558f4d63372c351"].into(),
							hex!["1c313e9c1d704a99c25393f72f97c9a3124ef4fcf060496ae558f4d63372c351"].unchecked_into(),
						),
						(
							// 5GNo4Dm2AnPhnvQWiwh5iU3oHGC4wM9rG2yvbfDFRFfe6qLv
							hex!["bebcda62a44b4e08ee20a1bdb856aeee5c896dca65053c0e45e267fa78b1631c"].into(),
							hex!["bebcda62a44b4e08ee20a1bdb856aeee5c896dca65053c0e45e267fa78b1631c"].unchecked_into(),
						),
					],
					10_000 * UNITS,
				),
				// Pre-funded accounts
				vec![(
					// Galactic Council
					// 5GjfiRa32G5YhQja854QooT6fJimjDJUQhTywSwBSXeKbnsQ
					hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
					1_500_000_000,
				)],
				// council members
				// GC - same as sudo
				vec![hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into()],
				// technical committee
				// GC - same as sudo
				vec![hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into()],
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
				// GC - same as sudo
				vec![(
					hex!["cea84b21c8f4c2160b9be66cb43309bf76dce0d9f3c6687a0475c8f96394835b"].into(),
					1_200_000_000 * UNITS,
				)],
				// parachain ID
				PARA_ID.into(),
			)
		},
		// Bootnodes
		vec![
			"/dns/devnet-hydradx-p2p01.intergalactic.limited/tcp/30333/p2p/12D3KooWJYVVHudGGvJUQ98cHLKAr47LonxTckP498FMiFD3XfWw"
				.parse()
				.unwrap(),
			"/dns/devnet-hydradx-p2p02.intergalactic.limited/tcp/30333/p2p/12D3KooWQ42FDCxisiPZLvbE8JdEoQvcUUp6oaJte41ZGhAsZKHi"
				.parse()
				.unwrap(),
			"/dns/devnet-hydradx-p2p03.intergalactic.limited/tcp/30333/p2p/12D3KooWSbDL1xmE1tAUJ4zvUgUBPHni2FA7nNj2ZPWNSBEpMFzS"
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
			relay_chain: "westend".into(),
			para_id: PARA_ID,
		},
	))
}
