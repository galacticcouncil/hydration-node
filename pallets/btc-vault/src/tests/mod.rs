pub mod test_cases;

use crate as pallet_btc_vault;
use frame_support::{
	parameter_types,
	traits::{Currency, Everything},
	PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Signet: pallet_signet,
		BtcVault: pallet_btc_vault,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const ExistentialDeposit: u128 = 1;
	pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
	pub const BtcVaultPalletId: PalletId = PalletId(*b"btcvault");
	pub const MaxChainIdLength: u32 = 128;
	pub const MaxDataLength: u32 = 100_000;
	pub const MaxSignatureDeposit: u32 = 10_000_000;
	pub const MaxInputs: u32 = 10;
	pub const MaxOutputs: u32 = 10;
	pub const BitcoinCaip2: &'static str = "bip122:000000000019d6689c085ae165831e93";
	pub const MpcRootSignerAddress: [u8; 20] = [1u8; 20];
	pub const VaultPubkeyHash: [u8; 20] = [0xAA; 20];
	pub const KeyVersion: u32 = 0;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

impl pallet_signet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = SignetPalletId;
	type MaxChainIdLength = MaxChainIdLength;
	type WeightInfo = pallet_signet::weights::WeightInfo<Test>;
	type MaxDataLength = MaxDataLength;
	type UpdateOrigin = frame_system::EnsureRoot<u64>;
	type MaxSignatureDeposit = MaxSignatureDeposit;
	type MaxInputs = MaxInputs;
	type MaxOutputs = MaxOutputs;
}

impl pallet_btc_vault::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = frame_system::EnsureRoot<u64>;
	type PalletId = BtcVaultPalletId;
	type BitcoinCaip2 = BitcoinCaip2;
	type MpcRootSignerAddress = MpcRootSignerAddress;
	type VaultPubkeyHash = VaultPubkeyHash;
	type KeyVersion = KeyVersion;
	type WeightInfo = crate::weights::SubstrateWeight<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		let _ = Balances::deposit_creating(&1, 1_000_000);
		let _ = Balances::deposit_creating(&2, 1_000_000);
		let _ = Balances::deposit_creating(&3, 100);
	});
	ext
}
