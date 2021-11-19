use super::*;

use hex_literal::hex;

pub fn parachain_config(para_id: ParaId) -> Result<ChainSpec, String> {
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
				hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into(),
				//initial authorities & invulnerables
				vec![
					(
						hex!["da0fa4ab419def66fb4ac5224e594e82c34ee795268fc7787c8a096c4ff14f11"].into(),
						hex!["da0fa4ab419def66fb4ac5224e594e82c34ee795268fc7787c8a096c4ff14f11"].unchecked_into(),
					),
					(
						hex!["ecd7a5439c6ab0cd6550bc2f1cef5299d425bb95bb6d7afb32aa3d95ee4f7f1f"].into(),
						hex!["ecd7a5439c6ab0cd6550bc2f1cef5299d425bb95bb6d7afb32aa3d95ee4f7f1f"].unchecked_into(),
					),
					(
						hex!["f0ad6f1aae7a445c1e80cac883096ec8177eda276fec53ad9ccbe570f3090a26"].into(),
						hex!["f0ad6f1aae7a445c1e80cac883096ec8177eda276fec53ad9ccbe570f3090a26"].unchecked_into(),
					),
				],
				// Pre-funded accounts
				vec![(
					hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into(),
					1_000_000_000,
				)],
				true,
				para_id.into(),
				//Endowd  accounts
				vec![],
				vec![],
				hex!["30035c21ba9eda780130f2029a80c3e962f56588bc04c36be95a225cb536fb55"].into(),
				vec![],
				vec![],
				vec![],
			)
		},
		// Bootnodes
		vec![
			"/dns/p2p-01.para-testnet.hydradx.io/tcp/30333/p2p/12D3KooW9qapYrocm6W1meShf8eQfeJzbry9PN2CN6SfBGbymxPL"
				.parse()
				.unwrap(),
			"/dns/p2p-02.para-testnet.hydradx.io/tcp/30333/p2p/12D3KooWPS16BYW173YxmxEJpQBoDz1t3Ht4yaPwwg5qCTED7N66"
				.parse()
				.unwrap(),
			"/dns/p2p-03.para-testnet.hydradx.io/tcp/30333/p2p/12D3KooWRMgQRtYrWsLvuwg3V3aQEvMgsbb88T29cKCTH6RAxTaj"
				.parse()
				.unwrap(),
		],
		// Telemetry
		None,
		// Protocol ID
		Some(PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "westend".into(),
			para_id: para_id.into(),
		},
	))
}
