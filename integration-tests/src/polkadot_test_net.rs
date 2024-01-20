#![cfg(test)]
use frame_support::{
	assert_ok,
	sp_runtime::{
		traits::{AccountIdConversion, Block as BlockT, Dispatchable},
		BuildStorage, FixedU128, Permill,
	},
	traits::{GetCallMetadata, OnInitialize},
};
pub use hydradx_runtime::{
	evm::ExtendedAddressMapping, AccountId, Currencies, NativeExistentialDeposit, Treasury, VestingPalletId,
};
use pallet_transaction_multi_payment::Price;
pub use primitives::{constants::chain::CORE_ASSET_ID, AssetId, Balance, Moment};

use cumulus_primitives_core::ParaId;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
pub use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use hex_literal::hex;
use hydradx_runtime::{evm::WETH_ASSET_LOCATION, Referrals, RuntimeOrigin};
use pallet_evm::AddressMapping;
use pallet_referrals::{FeeDistribution, Level};
pub use polkadot_primitives::v5::{BlockNumber, MAX_CODE_SIZE, MAX_POV_SIZE};
use polkadot_runtime_parachains::configuration::HostConfiguration;
use sp_core::storage::Storage;
use sp_core::H160;
pub use xcm_emulator::Network;
use xcm_emulator::{decl_test_networks, decl_test_parachains, decl_test_relay_chains, DefaultMessageProcessor};

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];
pub const CHARLIE: [u8; 32] = [6u8; 32];
pub const DAVE: [u8; 32] = [7u8; 32];
pub const UNKNOWN: [u8; 32] = [8u8; 32];

pub fn evm_address() -> H160 {
	hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"].into()
}
pub fn evm_account() -> AccountId {
	ExtendedAddressMapping::into_account_id(evm_address())
}

pub fn evm_address2() -> H160 {
	hex!["222222ff7Be76052e023Ec1a306fCca8F9659D81"].into()
}
pub fn evm_account2() -> AccountId {
	ExtendedAddressMapping::into_account_id(evm_address2())
}
pub fn evm_signed_origin(address: H160) -> RuntimeOrigin {
	// account has to be truncated to spoof it as an origin
	let mut account_truncated: [u8; 32] = [0; 32];
	account_truncated[..address.clone().as_bytes().len()].copy_from_slice(address.as_bytes());
	RuntimeOrigin::signed(AccountId::from(account_truncated))
}
pub fn to_ether(b: Balance) -> Balance {
	b * 10_u128.pow(18)
}

pub const UNITS: Balance = 1_000_000_000_000;

pub const ACALA_PARA_ID: u32 = 2_000;
pub const HYDRA_PARA_ID: u32 = 2_034;
pub const MOONBEAM_PARA_ID: u32 = 2_004;
pub const INTERLAY_PARA_ID: u32 = 2_032;

pub const ALICE_INITIAL_NATIVE_BALANCE: Balance = 1_000 * UNITS;
pub const ALICE_INITIAL_DAI_BALANCE: Balance = 2_000 * UNITS;
pub const ALICE_INITIAL_LRNA_BALANCE: Balance = 200 * UNITS;
pub const ALICE_INITIAL_DOT_BALANCE: Balance = 2_000 * UNITS;
pub const BOB_INITIAL_NATIVE_BALANCE: Balance = 1_000 * UNITS;
pub const BOB_INITIAL_LRNA_BALANCE: Balance = 1_000 * UNITS;
pub const BOB_INITIAL_DAI_BALANCE: Balance = 1_000_000_000 * UNITS;
pub const CHARLIE_INITIAL_LRNA_BALANCE: Balance = 1_000 * UNITS;

pub fn parachain_reserve_account() -> AccountId {
	polkadot_parachain::primitives::Sibling::from(ACALA_PARA_ID).into_account_truncating()
}

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 3;
pub const ETH: AssetId = 4;
pub const BTC: AssetId = 5;
pub const ACA: AssetId = 6;
pub const WETH: AssetId = 20;
pub const PEPE: AssetId = 420;

pub const NOW: Moment = 1689844300000; // unix time in milliseconds

decl_test_relay_chains! {
	#[api_version(5)]
	pub struct PolkadotRelay {
		genesis = polkadot::genesis(),
		on_init = {
			polkadot_runtime::System::set_block_number(1);
		},
		runtime = polkadot_runtime,
		core = {
			MessageProcessor: DefaultMessageProcessor<PolkadotRelay>,
			SovereignAccountOf: polkadot_runtime::xcm_config::SovereignAccountOf,
		},
		pallets = {
			XcmPallet: polkadot_runtime::XcmPallet,
			Balances: polkadot_runtime::Balances,
			Hrmp: polkadot_runtime::Hrmp,
		}
	}
}

decl_test_parachains! {
	pub struct Hydra {
		genesis = hydra::genesis(),
		on_init = {
			hydradx_runtime::System::set_block_number(1);
			hydradx_runtime::Timestamp::set_timestamp(NOW);
			// Make sure the prices are up-to-date.
			hydradx_runtime::MultiTransactionPayment::on_initialize(1);
			hydradx_runtime::AssetRegistry::set_location(RuntimeOrigin::root(), WETH, WETH_ASSET_LOCATION).unwrap();
		},
		runtime = hydradx_runtime,
		core = {
			XcmpMessageHandler: hydradx_runtime::XcmpQueue,
			DmpMessageHandler: hydradx_runtime::DmpQueue,
			LocationToAccountId: hydradx_runtime::xcm::LocationToAccountId,
			ParachainInfo: hydradx_runtime::ParachainInfo,
		},
		pallets = {
			PolkadotXcm: hydradx_runtime::PolkadotXcm,
			Balances: hydradx_runtime::Balances,
		}
	},
	pub struct Acala {
		genesis = para::genesis(ACALA_PARA_ID),
		on_init = {
			hydradx_runtime::System::set_block_number(1);
		},
		runtime = hydradx_runtime,
		core = {
			XcmpMessageHandler: hydradx_runtime::XcmpQueue,
			DmpMessageHandler: hydradx_runtime::DmpQueue,
			LocationToAccountId: hydradx_runtime::xcm::LocationToAccountId,
			ParachainInfo: hydradx_runtime::ParachainInfo,
		},
		pallets = {
			PolkadotXcm: hydradx_runtime::PolkadotXcm,
			Balances: hydradx_runtime::Balances,
		}
	},
	pub struct Moonbeam {
		genesis = para::genesis(MOONBEAM_PARA_ID),
		on_init = {
			hydradx_runtime::System::set_block_number(1);
		},
		runtime = hydradx_runtime,
		core = {
			XcmpMessageHandler: hydradx_runtime::XcmpQueue,
			DmpMessageHandler: hydradx_runtime::DmpQueue,
			LocationToAccountId: hydradx_runtime::xcm::LocationToAccountId,
			ParachainInfo: hydradx_runtime::ParachainInfo,
		},
		pallets = {
			PolkadotXcm: hydradx_runtime::PolkadotXcm,
			Balances: hydradx_runtime::Balances,
		}
	},
	pub struct Interlay {
		genesis = para::genesis(INTERLAY_PARA_ID),
		on_init = {
			hydradx_runtime::System::set_block_number(1);
		},
		runtime = hydradx_runtime,
		core = {
			XcmpMessageHandler: hydradx_runtime::XcmpQueue,
			DmpMessageHandler: hydradx_runtime::DmpQueue,
			LocationToAccountId: hydradx_runtime::xcm::LocationToAccountId,
			ParachainInfo: hydradx_runtime::ParachainInfo,
		},
		pallets = {
			PolkadotXcm: hydradx_runtime::PolkadotXcm,
			Balances: hydradx_runtime::Balances,
		}
	}
}

decl_test_networks! {
	pub struct TestNet {
		relay_chain = PolkadotRelay,
		parachains = vec![
			Acala,
			Moonbeam,
			Interlay,
			Hydra,
		],
		bridge = ()
	},
}

pub mod polkadot {
	use super::*;

	fn get_host_configuration() -> HostConfiguration<BlockNumber> {
		HostConfiguration {
			minimum_validation_upgrade_delay: 5,
			validation_upgrade_cooldown: 5u32,
			validation_upgrade_delay: 5,
			code_retention_period: 1200,
			max_code_size: MAX_CODE_SIZE,
			max_pov_size: MAX_POV_SIZE,
			max_head_data_size: 32 * 1024,
			group_rotation_frequency: 20,
			paras_availability_period: 4,
			max_upward_queue_count: 8,
			max_upward_queue_size: 1024 * 1024,
			max_downward_message_size: 1024,
			max_upward_message_size: 50 * 1024,
			max_upward_message_num_per_candidate: 5,
			hrmp_sender_deposit: 0,
			hrmp_recipient_deposit: 0,
			hrmp_channel_max_capacity: 8,
			hrmp_channel_max_total_size: 8 * 1024,
			hrmp_max_parachain_inbound_channels: 4,
			hrmp_channel_max_message_size: 1024 * 1024,
			hrmp_max_parachain_outbound_channels: 4,
			hrmp_max_message_num_per_candidate: 5,
			dispute_period: 6,
			no_show_slots: 2,
			n_delay_tranches: 25,
			needed_approvals: 2,
			relay_vrf_modulo_samples: 2,
			zeroth_delay_tranche_width: 0,
			..Default::default()
		}
	}

	use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
	use polkadot_primitives::{AssignmentId, ValidatorId};
	use polkadot_service::chain_spec::get_authority_keys_from_seed_no_beefy;
	use sc_consensus_grandpa::AuthorityId as GrandpaId;
	use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
	use sp_consensus_babe::AuthorityId as BabeId;

	#[allow(clippy::type_complexity)]
	pub fn initial_authorities() -> Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ImOnlineId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> {
		vec![get_authority_keys_from_seed_no_beefy("Alice")]
	}

	fn session_keys(
		babe: BabeId,
		grandpa: GrandpaId,
		im_online: ImOnlineId,
		para_validator: ValidatorId,
		para_assignment: AssignmentId,
		authority_discovery: AuthorityDiscoveryId,
	) -> polkadot_runtime::SessionKeys {
		polkadot_runtime::SessionKeys {
			babe,
			grandpa,
			im_online,
			para_validator,
			para_assignment,
			authority_discovery,
		}
	}

	pub fn genesis() -> Storage {
		let genesis_config = polkadot_runtime::RuntimeGenesisConfig {
			balances: polkadot_runtime::BalancesConfig {
				balances: vec![
					(AccountId::from(ALICE), 2_002 * UNITS),
					(ParaId::from(HYDRA_PARA_ID).into_account_truncating(), 10 * UNITS),
				],
			},
			session: polkadot_runtime::SessionConfig {
				keys: initial_authorities()
					.iter()
					.map(|x| {
						(
							x.0.clone(),
							x.0.clone(),
							polkadot::session_keys(
								x.2.clone(),
								x.3.clone(),
								x.4.clone(),
								x.5.clone(),
								x.6.clone(),
								x.7.clone(),
							),
						)
					})
					.collect::<Vec<_>>(),
			},
			configuration: polkadot_runtime::ConfigurationConfig {
				config: get_host_configuration(),
			},
			xcm_pallet: polkadot_runtime::XcmPalletConfig {
				safe_xcm_version: Some(3),
				..Default::default()
			},
			babe: polkadot_runtime::BabeConfig {
				authorities: Default::default(),
				epoch_config: Some(polkadot_runtime::BABE_GENESIS_EPOCH_CONFIG),
				..Default::default()
			},
			..Default::default()
		};

		genesis_config.build_storage().unwrap()
	}
}

use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiSignature,
};
type AccountPublic = <MultiSignature as Verify>::Signer;

/// Helper function to generate a crypto pair from seed
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed.
fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
pub mod collators {
	use super::*;

	pub fn invulnerables() -> Vec<(AccountId, AuraId)> {
		vec![
			(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_from_seed::<AuraId>("Alice"),
			),
			(
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_from_seed::<AuraId>("Bob"),
			),
		]
	}
}

pub mod hydra {
	use super::*;

	pub fn genesis() -> Storage {
		let stable_amount = 50_000 * UNITS * 1_000_000;
		let native_amount = 936_329_588_000_000_000;
		let dot_amount = 87_719_298_250_000_u128;
		let eth_amount = 63_750_000_000_000_000_000u128;
		let btc_amount = 1_000_000_000u128;
		let omnipool_account = hydradx_runtime::Omnipool::protocol_account();
		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();

		let existential_deposit = NativeExistentialDeposit::get();

		let genesis_config = hydradx_runtime::RuntimeGenesisConfig {
			balances: hydradx_runtime::BalancesConfig {
				balances: vec![
					(AccountId::from(ALICE), ALICE_INITIAL_NATIVE_BALANCE),
					(AccountId::from(BOB), BOB_INITIAL_NATIVE_BALANCE),
					(AccountId::from(CHARLIE), 1_000 * UNITS),
					(AccountId::from(DAVE), 1_000 * UNITS),
					(omnipool_account.clone(), native_amount),
					(vesting_account(), 10_000 * UNITS),
					(staking_account, UNITS),
				],
			},
			collator_selection: hydradx_runtime::CollatorSelectionConfig {
				invulnerables: collators::invulnerables().iter().cloned().map(|(acc, _)| acc).collect(),
				candidacy_bond: 2 * UNITS,
				..Default::default()
			},
			session: hydradx_runtime::SessionConfig {
				keys: collators::invulnerables()
					.into_iter()
					.map(|(acc, aura)| {
						(
							acc.clone(),                                   // account id
							acc,                                           // validator id
							hydradx_runtime::opaque::SessionKeys { aura }, // session keys
						)
					})
					.collect(),
			},
			asset_registry: hydradx_runtime::AssetRegistryConfig {
				registered_assets: vec![
					(b"LRNA".to_vec(), 1_000u128, Some(LRNA)),
					(b"DAI".to_vec(), 1_000u128, Some(DAI)),
					(b"DOT".to_vec(), 1_000u128, Some(DOT)),
					(b"ETH".to_vec(), 1_000u128, Some(ETH)),
					(b"BTC".to_vec(), 1_000u128, Some(BTC)),
					(b"ACA".to_vec(), 1_000u128, Some(ACA)),
					(b"WETH".to_vec(), 1_000u128, Some(WETH)),
					(b"PEPE".to_vec(), 1_000u128, Some(PEPE)),
					// workaround for next_asset_id() to return correct values
					(b"DUMMY".to_vec(), 1_000u128, None),
				],
				native_asset_name: b"HDX".to_vec(),
				native_existential_deposit: existential_deposit,
			},
			parachain_info: hydradx_runtime::ParachainInfoConfig {
				parachain_id: HYDRA_PARA_ID.into(),
				..Default::default()
			},
			tokens: hydradx_runtime::TokensConfig {
				balances: vec![
					(AccountId::from(ALICE), LRNA, ALICE_INITIAL_LRNA_BALANCE),
					(AccountId::from(ALICE), DAI, ALICE_INITIAL_DAI_BALANCE),
					(AccountId::from(ALICE), DOT, ALICE_INITIAL_DOT_BALANCE),
					(AccountId::from(BOB), LRNA, BOB_INITIAL_LRNA_BALANCE),
					(AccountId::from(BOB), DAI, BOB_INITIAL_DAI_BALANCE),
					(AccountId::from(BOB), BTC, 1_000_000),
					(AccountId::from(CHARLIE), DAI, 80_000_000_000 * UNITS),
					(AccountId::from(BOB), PEPE, 1_000 * UNITS * 1_000_000),
					(AccountId::from(CHARLIE), LRNA, CHARLIE_INITIAL_LRNA_BALANCE),
					(AccountId::from(DAVE), LRNA, 1_000 * UNITS),
					(AccountId::from(DAVE), DAI, 1_000_000_000 * UNITS),
					(evm_account(), WETH, to_ether(1_000)),
					(omnipool_account.clone(), DAI, stable_amount),
					(omnipool_account.clone(), ETH, eth_amount),
					(omnipool_account.clone(), BTC, btc_amount),
					(omnipool_account, DOT, dot_amount),
				],
			},
			polkadot_xcm: hydradx_runtime::PolkadotXcmConfig {
				safe_xcm_version: Some(3),
				..Default::default()
			},
			multi_transaction_payment: hydradx_runtime::MultiTransactionPaymentConfig {
				currencies: vec![
					(LRNA, Price::from(1)),
					(DAI, Price::from(1)),
					(ACA, Price::from(1)),
					(BTC, Price::from_inner(134_000_000)),
					(WETH, Price::from_inner(3_666_754_716_981_130_000)),
				],
				account_currencies: vec![],
			},
			duster: hydradx_runtime::DusterConfig {
				account_blacklist: vec![Treasury::account_id()],
				reward_account: Some(Treasury::account_id()),
				dust_account: Some(Treasury::account_id()),
			},
			..Default::default()
		};
		genesis_config.build_storage().unwrap()
	}
}

pub mod para {
	use super::*;

	pub fn genesis(para_id: u32) -> Storage {
		let genesis_config = hydradx_runtime::RuntimeGenesisConfig {
			balances: hydradx_runtime::BalancesConfig {
				balances: vec![(AccountId::from(ALICE), ALICE_INITIAL_NATIVE_BALANCE)],
			},
			collator_selection: hydradx_runtime::CollatorSelectionConfig {
				invulnerables: collators::invulnerables().iter().cloned().map(|(acc, _)| acc).collect(),
				candidacy_bond: UNITS * 16,
				..Default::default()
			},
			session: hydradx_runtime::SessionConfig {
				keys: collators::invulnerables()
					.into_iter()
					.map(|(acc, aura)| {
						(
							acc.clone(),                                   // account id
							acc,                                           // validator id
							hydradx_runtime::opaque::SessionKeys { aura }, // session keys
						)
					})
					.collect(),
			},
			parachain_info: hydradx_runtime::ParachainInfoConfig {
				parachain_id: para_id.into(),
				..Default::default()
			},
			polkadot_xcm: hydradx_runtime::PolkadotXcmConfig {
				safe_xcm_version: Some(3),
				..Default::default()
			},
			duster: hydradx_runtime::DusterConfig {
				account_blacklist: vec![Treasury::account_id()],
				reward_account: Some(Treasury::account_id()),
				dust_account: Some(Treasury::account_id()),
			},
			..Default::default()
		};

		genesis_config.build_storage().unwrap()
	}
}

pub fn vesting_account() -> AccountId {
	VestingPalletId::get().into_account_truncating()
}

pub fn last_hydra_events(n: usize) -> Vec<hydradx_runtime::RuntimeEvent> {
	frame_system::Pallet::<hydradx_runtime::Runtime>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

pub fn expect_hydra_events(e: Vec<hydradx_runtime::RuntimeEvent>) {
	pretty_assertions::assert_eq!(last_hydra_events(e.len()), e);
}

pub fn set_relaychain_block_number(number: BlockNumber) {
	use hydradx_runtime::ParachainSystem;

	// We need to set block number this way as well because tarpaulin code coverage tool does not like the way
	// how we set the block number with `cumulus-test-relay-sproof-builder` package
	polkadot_run_to_block(number);

	ParachainSystem::on_initialize(number);

	let (relay_storage_root, proof) = RelayStateSproofBuilder::default().into_state_root_and_proof();

	assert_ok!(ParachainSystem::set_validation_data(
		RuntimeOrigin::none(),
		cumulus_primitives_parachain_inherent::ParachainInherentData {
			validation_data: cumulus_primitives_core::PersistedValidationData {
				parent_head: Default::default(),
				relay_parent_number: number,
				relay_parent_storage_root: relay_storage_root,
				max_pov_size: Default::default(),
			},
			relay_chain_state: proof,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		}
	));
}

pub fn hydradx_run_to_next_block() {
	use frame_support::traits::OnFinalize;

	let b = hydradx_runtime::System::block_number();

	hydradx_runtime::System::on_finalize(b);
	hydradx_runtime::EmaOracle::on_finalize(b);
	hydradx_runtime::MultiTransactionPayment::on_finalize(b);

	hydradx_runtime::System::on_initialize(b + 1);
	hydradx_runtime::EmaOracle::on_initialize(b + 1);
	hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);

	hydradx_runtime::System::set_block_number(b + 1);
}

pub fn hydradx_run_to_block(to: BlockNumber) {
	let b = hydradx_runtime::System::block_number();
	assert!(b <= to, "the current block number {:?} is higher than expected.", b);

	while hydradx_runtime::System::block_number() < to {
		hydradx_run_to_next_block();
	}
}

pub fn polkadot_run_to_block(to: BlockNumber) {
	use frame_support::traits::OnFinalize;

	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);
		hydradx_runtime::EmaOracle::on_finalize(b);
		hydradx_runtime::DCA::on_finalize(b);
		hydradx_runtime::CircuitBreaker::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);
		hydradx_runtime::EmaOracle::on_initialize(b + 1);
		hydradx_runtime::DCA::on_initialize(b + 1);
		hydradx_runtime::CircuitBreaker::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

pub fn hydra_live_ext(
	path_to_snapshot: &str,
) -> frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block> {
	let ext = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap()
		.block_on(async {
			use frame_remote_externalities::*;

			let snapshot_config = SnapshotConfig::from(String::from(path_to_snapshot));
			let offline_config = OfflineConfig {
				state_snapshot: snapshot_config,
			};
			let mode = Mode::Offline(offline_config);

			let builder = Builder::<hydradx_runtime::Block>::new().mode(mode);

			builder.build().await.unwrap()
		});
	ext
}

#[allow(dead_code)]
pub fn apply_blocks_from_file(pallet_whitelist: Vec<&str>) {
	let blocks =
		scraper::load_blocks_snapshot::<hydradx_runtime::Block>(&std::path::PathBuf::from("../scraper/SNAPSHOT"))
			.unwrap();

	for block in blocks.iter() {
		for tx in block.extrinsics() {
			let call = &tx.0.function;
			let call_p = call.get_call_metadata().pallet_name;

			if pallet_whitelist.contains(&call_p) {
				let acc = &tx.0.signature.as_ref().unwrap().0;
				assert_ok!(call
					.clone()
					.dispatch(hydradx_runtime::RuntimeOrigin::signed(acc.clone())));
			}
		}
	}
}

pub fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	let native_position_id = hydradx_runtime::Omnipool::next_position_id();

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		AccountId::from(ALICE),
	));

	let stable_position_id = hydradx_runtime::Omnipool::next_position_id();

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		native_position_id,
	));

	assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		stable_position_id,
	));

	set_zero_reward_for_referrals(DAI);
	set_zero_reward_for_referrals(HDX);
}

pub fn set_zero_reward_for_referrals(asset_id: AssetId) {
	assert_ok!(Referrals::set_reward_percentage(
		RawOrigin::Root.into(),
		asset_id,
		Level::None,
		FeeDistribution::default(),
	));
}
