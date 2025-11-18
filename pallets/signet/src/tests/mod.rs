mod tests;
pub mod utils;

use crate::{self as pallet_signet, *};
use frame_support::{
	parameter_types,
	traits::{ConstU16, ConstU64},
	PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::BuildStorage;

#[frame_support::pallet]
pub mod pallet_mock_caller {
	use crate::{self as pallet_signet, tests::utils::bounded_u8};
	use frame_support::{pallet_prelude::*, PalletId};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::AccountIdConversion;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_signet::Config {
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn call_signet(origin: OriginFor<T>) -> DispatchResult {
			// This pallet will call signet with ITS OWN account as the sender
			let _who = ensure_signed(origin)?;

			// Get this pallet's derived account (use fully-qualified syntax)
			let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

			// Call signet from this pallet's account
			pallet_signet::Pallet::<T>::sign(
				frame_system::RawOrigin::Signed(pallet_account).into(),
				[99u8; 32],
				1,
				bounded_u8::<256>(b"from_pallet".to_vec()),
				bounded_u8::<32>(b"ecdsa".to_vec()),
				bounded_u8::<64>(b"".to_vec()),
				bounded_u8::<1024>(b"{}".to_vec()),
			)?;

			Ok(())
		}
	}
}

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Signet: pallet_signet,
		MockCaller: pallet_mock_caller,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const MaxDataLength: u32 = 100_000;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
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
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
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
}

parameter_types! {
	pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
	pub const MaxChainIdLength: u32 = 128;
}

impl pallet_signet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = SignetPalletId;
	type MaxChainIdLength = MaxChainIdLength;
	type WeightInfo = ();
	type MaxDataLength = MaxDataLength;
}

parameter_types! {
	pub const MockCallerPalletId: PalletId = PalletId(*b"py/mockc");
}

impl pallet_mock_caller::Config for Test {
	type PalletId = MockCallerPalletId;
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
