#![allow(clippy::or_fun_call)]

use hydra_dx_runtime::constants::currency::{Balance, HDX};
use hydra_dx_runtime::opaque::SessionKeys;
use hydra_dx_runtime::pallet_claims::EthereumAddress;
use hydra_dx_runtime::{
	AccountId, AssetRegistryConfig, AuthorityDiscoveryConfig, BabeConfig, BalancesConfig, ClaimsConfig, CouncilConfig,
	ElectionsConfig, FaucetConfig, GenesisConfig, GenesisHistoryConfig, GrandpaConfig, ImOnlineConfig, Perbill,
	SessionConfig, Signature, StakerStatus, StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeConfig,
	TokensConfig, CORE_ASSET_ID, WASM_BINARY,
};
use pallet_staking::Forcing;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

use hex_literal::hex;
use hydra_dx_runtime::pallet_genesis_history::Chain;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;

// The URL for the telemetry server.
const TELEMETRY_URLS: [&str; 2] = [
	"wss://telemetry.polkadot.io/submit/",
	"wss://telemetry.hydradx.io:9000/submit/",
];
/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	GrandpaId,
	BabeId,
	ImOnlineId,
	AuthorityDiscoveryId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<ImOnlineId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

fn session_keys(
	grandpa: GrandpaId,
	babe: BabeId,
	im_online: ImOnlineId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys {
		grandpa,
		babe,
		im_online,
		authority_discovery,
	}
}

const STASH: Balance = 100 * HDX;
const DEFAULT_PROTOCOL_ID: &str = "hdx";

pub fn development_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("tokenSymbol".into(), "HDX".into());
	properties.insert("ss58Format".into(), 63.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Development chain",
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

pub fn lerna_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../res/lerna.json")[..])
}

pub fn lerna_staging_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Stakenet wasm binary not available".to_string())?;
	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("tokenSymbol".into(), "HDX".into());
	properties.insert("ss58Format".into(), 63.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Snakenet Gen2",
		// ID
		"lerna",
		ChainType::Live,
		move || {
			lerna_genesis(
				wasm_binary,
				vec![
					(
						//5DvaWvPYpPo6aMKBZhCTtCmfbZntA9y5tmsMvVg5sD75aPRQ
						hex!["5245cb1e9e810f66940ec82a23a485491347bdbdc2726f3e2d40d9650cbc4103"].into(),
						//5DvaWvPYpPo6aMKBZhCTtCmfbZntA9y5tmsMvVg5sD75aPRQ
						hex!["5245cb1e9e810f66940ec82a23a485491347bdbdc2726f3e2d40d9650cbc4103"].into(),
						//5DdKFiVQx8R7cNW5FckvftE7NfQCPW1GF9p8FUKnB3o6AvMu
						hex!["451b3e9b67ffea5e90b61e23396451a336e1449620bba3e13fbb96e187007c1a"].unchecked_into(),
						//5GTyALyDv9EFARPWrotf8yBJ3F3zSyk8NtUqcrtiBDVkbFLb
						hex!["c2af193a251dee1765136b0ae47647c110ac1225b23a157d6ef6629b1c93fe39"].unchecked_into(),
						//5GTyALyDv9EFARPWrotf8yBJ3F3zSyk8NtUqcrtiBDVkbFLb
						hex!["c2af193a251dee1765136b0ae47647c110ac1225b23a157d6ef6629b1c93fe39"].unchecked_into(),
						//5GTyALyDv9EFARPWrotf8yBJ3F3zSyk8NtUqcrtiBDVkbFLb
						hex!["c2af193a251dee1765136b0ae47647c110ac1225b23a157d6ef6629b1c93fe39"].unchecked_into(),
					),
					(
						//5GNR5oNz2ouy3vpKvfb79u9yZ5WW1fpX9aS9vMHbqcuhUkDC
						hex!["be72e2daa41acfd97eed4c09a086dc84b99df8e8ddddb67e90b71c36e4826378"].into(),
						//5GNR5oNz2ouy3vpKvfb79u9yZ5WW1fpX9aS9vMHbqcuhUkDC
						hex!["be72e2daa41acfd97eed4c09a086dc84b99df8e8ddddb67e90b71c36e4826378"].into(),
						//5HWDxcXHPxSowKDXSSKLEkUxXymXw2FA9zKyAwYw7nJ8KpYL
						hex!["f0a3a2eab48b0e51e8d89732d15da0164eb36951c4db3bd33879b0b343619ba7"].unchecked_into(),
						//5Fgn5eu1dhHemGLbHRgFuhdjjTHPuGt6UbLmwd2bi7JonwAG
						hex!["a037c0f83b7ebea2179165f987c6094d5b39e7addc1d2e09edf4a5fa6ebcac32"].unchecked_into(),
						//5Fgn5eu1dhHemGLbHRgFuhdjjTHPuGt6UbLmwd2bi7JonwAG
						hex!["a037c0f83b7ebea2179165f987c6094d5b39e7addc1d2e09edf4a5fa6ebcac32"].unchecked_into(),
						//5Fgn5eu1dhHemGLbHRgFuhdjjTHPuGt6UbLmwd2bi7JonwAG
						hex!["a037c0f83b7ebea2179165f987c6094d5b39e7addc1d2e09edf4a5fa6ebcac32"].unchecked_into(),
					),
					(
						//5Hiqm2wJATfFWdq9oDzQXBA7LhPbBNPRz4axdg4APjcRhUdQ
						hex!["fa431893b2d8196ab179793714d653ce840fcac1847c1cb32522496989c0e556"].into(),
						//5Hiqm2wJATfFWdq9oDzQXBA7LhPbBNPRz4axdg4APjcRhUdQ
						hex!["fa431893b2d8196ab179793714d653ce840fcac1847c1cb32522496989c0e556"].into(),
						//5H1TccKGpCsVM4STCELgHQAq5cMXXXBRSnJETy7hiZAUGZav
						hex!["dab37ca3624720b03aa2fdf4f2b436041ff151f0e3975f7b9c79e52030ae781e"].unchecked_into(),
						//5HGxatQ8j4HtoDiwUvT8gL3HMrXBwP4dMBQQPaYpvR6W2Ztc
						hex!["7a256c0498e35373006232ae18e18ec44c80c9d73aed563100fc8b7e0cf99001"].unchecked_into(),
						//5HGxatQ8j4HtoDiwUvT8gL3HMrXBwP4dMBQQPaYpvR6W2Ztc
						hex!["7a256c0498e35373006232ae18e18ec44c80c9d73aed563100fc8b7e0cf99001"].unchecked_into(),
						//5HGxatQ8j4HtoDiwUvT8gL3HMrXBwP4dMBQQPaYpvR6W2Ztc
						hex!["7a256c0498e35373006232ae18e18ec44c80c9d73aed563100fc8b7e0cf99001"].unchecked_into(),
					),
				],
				// Sudo account
				hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into(),
				// Pre-funded accounts
				vec![],
				true,
			)
		},
		// Bootnodes TODO: BOOT NODES
		vec![
			"/dns/p2p-01.snakenet.hydradx.io/tcp/40444/p2p/12D3KooWAJ8t7rsWvV7d1CRCT7afwtmBQBrRT7mMNDVCWK7n9CrD"
				.parse()
				.unwrap(),
			"/dns/p2p-02.snakenet.hydradx.io/tcp/40444/p2p/12D3KooWErP8DjDoVFjsCCzvD9mFZBA6Y1VKMEBNH8vKCWDZDHz5"
				.parse()
				.unwrap(),
			"/dns/p2p-03.snakenet.hydradx.io/tcp/40444/p2p/12D3KooWH9rsDFq3wo13eKR5PWCvEDieK8uUKd1C1dLQNNxeU5AU"
				.parse()
				.unwrap(),
		],
		// Telemetry
		Some(
			TelemetryEndpoints::new(vec![
				(TELEMETRY_URLS[0].to_string(), 0),
				(TELEMETRY_URLS[1].to_string(), 0),
			])
			.expect("Telemetry url is valid"),
		),
		// Protocol ID
		Some(DEFAULT_PROTOCOL_ID),
		// Properties
		Some(properties),
		// Extensions
		None,
	))
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	let mut properties = Map::new();
	properties.insert("tokenDecimals".into(), 12.into());
	properties.insert("tokenSymbol".into(), "HDX".into());
	properties.insert("ss58Format".into(), 63.into());

	Ok(ChainSpec::from_genesis(
		// Name
		"HydraDX Local Testnet",
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

/// Configure initial storage state for FRAME modules.
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
) -> GenesisConfig {
	GenesisConfig {
		frame_system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: BalancesConfig {
			// Configure endowed accounts with initial balance of 1_000_000.
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1_000_000u128 * HDX))
				.collect(),
		},
		pallet_grandpa: Default::default(),
		pallet_sudo: SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		pallet_asset_registry: AssetRegistryConfig {
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
		orml_tokens: TokensConfig {
			endowed_accounts: endowed_accounts
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
		pallet_faucet: FaucetConfig {
			rampage: true,
			mint_limit: 5,
			mintable_currencies: vec![0, 1, 2],
		},
		pallet_babe: BabeConfig {
			authorities: vec![],
			epoch_config: Default::default(), //TODO: epoch config value ???
		},
		pallet_authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		pallet_im_online: Default::default(),
		pallet_treasury: Default::default(),
		pallet_session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		pallet_staking: StakingConfig {
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
		pallet_elections_phragmen: ElectionsConfig {
			members: vec![(get_account_id_from_seed::<sr25519::Public>("Alice"), STASH / 2)],
		},
		pallet_collective_Instance1: CouncilConfig {
			members: vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: TechnicalCommitteeConfig {
			members: vec![
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_account_id_from_seed::<sr25519::Public>("Eve"),
			],
			phantom: Default::default(),
		},
		pallet_claims: ClaimsConfig {
			claims: create_testnet_claims(),
		},
		pallet_genesis_history: GenesisHistoryConfig::default(),
	}
}

fn lerna_genesis(
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
) -> GenesisConfig {
	GenesisConfig {
		frame_system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: BalancesConfig {
			// Intergalactic initial supply
			balances: vec![
				(
					// Intergalactic HDX Tokens 15%
					hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into(),
					(1_500_000_000u128 * HDX) - (3 * STASH),
				),
				(
					// Treasury for rewards 3%
					hex!["84d0959b84b3b12013430ea136b0c26e83412ea3bc46a8620abb8c8db7e53d0c"].into(),
					300_000_000 * HDX,
				),
				(
					// Intergalactic Validator01
					hex!["5245cb1e9e810f66940ec82a23a485491347bdbdc2726f3e2d40d9650cbc4103"].into(),
					STASH,
				),
				(
					// Intergalactic Validator02
					hex!["be72e2daa41acfd97eed4c09a086dc84b99df8e8ddddb67e90b71c36e4826378"].into(),
					STASH,
				),
				(
					// Intergalactic Validator03
					hex!["fa431893b2d8196ab179793714d653ce840fcac1847c1cb32522496989c0e556"].into(),
					STASH,
				),
				(
					// Unsold tokens treasury
					hex!["6d6f646c70792f74727372790000000000000000000000000000000000000000"].into(),
					56873469471297884942_u128,
				),
			],
		},
		pallet_grandpa: GrandpaConfig { authorities: vec![] },
		pallet_sudo: SudoConfig {
			// Assign network admin rights.
			key: root_key,
		},
		pallet_asset_registry: AssetRegistryConfig {
			core_asset_id: CORE_ASSET_ID,
			asset_ids: vec![],
			next_asset_id: 1,
		},
		orml_tokens: TokensConfig {
			endowed_accounts: endowed_accounts.iter().flat_map(|_x| vec![]).collect(),
		},
		pallet_faucet: FaucetConfig {
			rampage: false,
			mint_limit: 5,
			mintable_currencies: vec![],
		},
		pallet_babe: BabeConfig {
			authorities: vec![],
			epoch_config: Default::default(),
		},
		pallet_authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		pallet_im_online: ImOnlineConfig { keys: vec![] },
		pallet_treasury: Default::default(),
		pallet_session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
					)
				})
				.collect::<Vec<_>>(),
		},
		pallet_staking: StakingConfig {
			validator_count: 3,
			minimum_validator_count: 3,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			force_era: Forcing::ForceNone,
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		},
		pallet_elections_phragmen: ElectionsConfig {
			// Intergalactic elections
			members: vec![(
				hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into(),
				STASH,
			)],
		},
		pallet_collective_Instance1: CouncilConfig {
			// Intergalactic council member
			members: vec![hex!["0abad795adcb5dee45d29528005b1f78d55fc170844babde88df84016c6cd14d"].into()],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: TechnicalCommitteeConfig {
			members: vec![
				hex!["d6cf8789dce651cb54a4036406f4aa0c771914d345c004ad0567b814c71fb637"].into(),
				hex!["bc96ec00952efa8f0e3e08b36bf5096bcb877acac536e478aecb72868db5db02"].into(),
				hex!["2875dd47bc1bcb70e23de79e7538c312be12c716033bbae425130e46f5f2b35e"].into(),
				hex!["644643bf953233d08c4c9bae0acd49f3baa7658d9b342b7e6879bb149ee6e44c"].into(),
				hex!["ccdb435892c9883656d0398b2b67023ba1e11bda0c7f213f70fdac54c6abab3f"].into(),
				hex!["f461c5ae6e80bf4af5b84452789c17b0b0a095a2d77c2a407978147de2d5b572"].into(),
			],
			phantom: Default::default(),
		},
		pallet_claims: ClaimsConfig { claims: vec![] },
		pallet_genesis_history: GenesisHistoryConfig {
			previous_chain: Chain {
				genesis_hash: hex!["0ed32bfcab4a83517fac88f2aa7cbc2f88d3ab93be9a12b6188a036bf8a943c2"]
					.to_vec()
					.into(),
				last_block_hash: hex!["f3c43294255f2d0cd8b3bc8787d18cc2adcec581f74d23df15ca75b8b77cd507"]
					.to_vec()
					.into(),
			},
		},
	}
}

fn create_testnet_claims() -> Vec<(EthereumAddress, Balance)> {
	let mut claims = Vec::<(EthereumAddress, Balance)>::new();

	// Alice's claim
	// Signature: 0xbcae7d4f96f71cf974c173ae936a1a79083af7f76232efbf8a568b7f990eceed73c2465bba769de959b7f6ac5690162b61eb90949901464d0fa158a83022a0741c
	// Message: "I hereby claim all my HDX tokens to wallet:d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
	let claim_address_1 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/0) : 0xdd75dd5f4a9e964d1c4cc929768947859a98ae2c08100744878a4b6b6d853cc0
		EthereumAddress(hex!["8202C0aF5962B750123CE1A9B12e1C30A4973557"]),
		HDX / 1_000,
	);

	// Bob's claim
	// Signature: 0x60f3d2541b0ff09982f70844a7f645f4681cbbad2f138fee18404c932bd02cb738d577d53ce94cf067bae87a0b6fa1ec532ceea78d71f4e81a9c27193649c6291b
	// Message: "I hereby claim all my HDX tokens to wallet:8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
	let claim_address_2 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/1) : 0x9b5ef380c0a59008df32ba71ab3c7645950f986fc3f43fd4f9dffc8b2b4e7a5d
		EthereumAddress(hex!["8aF7764663644989671A71Abe9738a3cF295f384"]),
		HDX,
	);

	// Charlie's claim
	// Signature: 0x52485aece74eb503fb998f0ca08bcc283fa731613db213af4e7fe153faed3de97ea0873d3889622b41d2d989a9e2a0bef160cff1ba8845875d4bc15431136a811c
	// Message: "I hereby claim all my HDX tokens to wallet:90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22"
	let claim_address_3 = (
		// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
		// private key (m/44'/60'/0'/0/2) : 0x653a29ac0c93de0e9f7d7ea2d60338e68f407b18d16d6ff84db996076424f8fa
		EthereumAddress(hex!["C19A2970A13ac19898c47d59Cbd0278D428EBC7c"]),
		1_000 * HDX,
	);

	claims.push(claim_address_1);
	claims.push(claim_address_2);
	claims.push(claim_address_3);
	claims
}
