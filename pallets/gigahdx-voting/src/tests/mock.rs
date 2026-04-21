use crate as pallet_gigahdx_voting;
use crate::*;
pub use frame_support::{
	assert_noop, assert_ok, parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage, Permill,
	},
	traits::{Everything, Nothing},
	PalletId,
};
use hydradx_traits::gigahdx::{
	ForceRemoveVote, GetReferendumOutcome, GetTrackId, ReferendumOutcome, TrackRewardConfig,
};
use orml_traits::parameter_type_with_key;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::H256;
use sp_runtime::DispatchResult;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const ST_HDX: AssetId = 100;
pub const GIGAHDX: AssetId = 101;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: pallet_currencies,
		GigaHdx: pallet_gigahdx,
		GigaHdxVoting: pallet_gigahdx_voting,
	}
);

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
	pub const ExistentialDeposit: u128 = 1;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

impl pallet_balances::Config for Test {
	type MaxLocks = frame_support::traits::ConstU32<50>;
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

impl orml_tokens::Config for Test {
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = frame_support::traits::ConstU32<50>;
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_currencies::Config for Test {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = HDXAssetId;
	type RegistryInspect = MockBoundErc20<Test>;
	type EgressHandler = pallet_currencies::MockEgressHandler<Test>;
	type WeightInfo = ();
}

// --- pallet-gigahdx config ---

parameter_types! {
	pub const GigaHdxPalletId: PalletId = PalletId(*b"gigahdx!");
	pub const HdxAssetId: AssetId = HDX;
	pub const StHdxAssetId: AssetId = ST_HDX;
	pub const GigaHdxAssetId: AssetId = GIGAHDX;
	pub const CooldownPeriod: u64 = 100;
	pub const MinStake: Balance = ONE;
	pub const MaxUnstakePositions: u32 = 10;
}

impl pallet_gigahdx::Config for Test {
	type Currency = FungibleCurrencies<Test>;
	type LockableCurrency = Currencies;
	type MoneyMarket = ();
	type Hooks = GigaHdxVoting;
	type PalletId = GigaHdxPalletId;
	type HdxAssetId = HdxAssetId;
	type StHdxAssetId = StHdxAssetId;
	type GigaHdxAssetId = GigaHdxAssetId;
	type CooldownPeriod = CooldownPeriod;
	type MinStake = MinStake;
	type MaxUnstakePositions = MaxUnstakePositions;
	type WeightInfo = ();
}

// --- pallet-gigahdx-voting config ---

// Thread-local mocks for referenda queries.
thread_local! {
	static REFERENDUM_OUTCOMES: sp_std::cell::RefCell<sp_std::collections::btree_map::BTreeMap<u32, ReferendumOutcome>> =
		sp_std::cell::RefCell::new(sp_std::collections::btree_map::BTreeMap::new());
	static TRACK_IDS: sp_std::cell::RefCell<sp_std::collections::btree_map::BTreeMap<u32, u16>> =
		sp_std::cell::RefCell::new(sp_std::collections::btree_map::BTreeMap::new());
	static FORCE_REMOVE_VOTE_CALLS: sp_std::cell::RefCell<sp_std::vec::Vec<(AccountId, Option<u16>, u32)>> =
		sp_std::cell::RefCell::new(sp_std::vec::Vec::new());
}

pub struct MockReferenda;

impl GetReferendumOutcome<u32> for MockReferenda {
	fn is_referendum_finished(index: u32) -> bool {
		REFERENDUM_OUTCOMES.with(|outcomes| {
			outcomes
				.borrow()
				.get(&index)
				.map_or(false, |o| *o != ReferendumOutcome::Ongoing)
		})
	}

	fn referendum_outcome(index: u32) -> ReferendumOutcome {
		REFERENDUM_OUTCOMES.with(|outcomes| {
			outcomes
				.borrow()
				.get(&index)
				.copied()
				.unwrap_or(ReferendumOutcome::Ongoing)
		})
	}
}

impl GetTrackId<u32> for MockReferenda {
	type TrackId = u16;

	fn track_id(index: u32) -> Option<u16> {
		TRACK_IDS.with(|tracks| tracks.borrow().get(&index).copied())
	}
}

pub struct MockTrackRewards;

impl TrackRewardConfig for MockTrackRewards {
	fn reward_percentage(track_id: u16) -> Permill {
		match track_id {
			0 => Permill::from_percent(10), // Root track: 10%
			1 => Permill::from_percent(5),  // Whitelisted: 5%
			_ => Permill::from_percent(2),  // Others: 2%
		}
	}
}

pub struct MockForceRemoveVote;

impl ForceRemoveVote<AccountId> for MockForceRemoveVote {
	fn remove_vote(who: &AccountId, class: Option<u16>, index: u32) -> DispatchResult {
		FORCE_REMOVE_VOTE_CALLS.with(|calls| {
			calls.borrow_mut().push((*who, class, index));
		});
		Ok(())
	}
}

parameter_types! {
	pub const GigaRewardPotId: PalletId = PalletId(*b"gigarwd!");
	pub const VoteLockingPeriod: u64 = 10; // 10 blocks per lock period
	pub const MaxVotes: u32 = 20;
}

impl pallet_gigahdx_voting::Config for Test {
	type NativeCurrency = Balances;
	type Referenda = MockReferenda;
	type TrackRewards = MockTrackRewards;
	type ForceRemoveVote = MockForceRemoveVote;
	type GigaRewardPotId = GigaRewardPotId;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MaxVotes = MaxVotes;
	type VotingWeightInfo = ();
}

// --- Test helpers ---

pub fn set_referendum_outcome(ref_index: u32, outcome: ReferendumOutcome) {
	REFERENDUM_OUTCOMES.with(|outcomes| {
		outcomes.borrow_mut().insert(ref_index, outcome);
	});
}

pub fn set_track_id(ref_index: u32, track_id: u16) {
	TRACK_IDS.with(|tracks| {
		tracks.borrow_mut().insert(ref_index, track_id);
	});
}

pub fn get_force_remove_calls() -> sp_std::vec::Vec<(AccountId, Option<u16>, u32)> {
	FORCE_REMOVE_VOTE_CALLS.with(|calls| calls.borrow().clone())
}

pub fn clear_force_remove_calls() {
	FORCE_REMOVE_VOTE_CALLS.with(|calls| calls.borrow_mut().clear());
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1_000 * ONE),
				(BOB, HDX, 1_000 * ONE),
				(CHARLIE, HDX, 1_000 * ONE),
				(ALICE, GIGAHDX, 500 * ONE),
				(BOB, GIGAHDX, 300 * ONE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Native (HDX) goes through pallet_balances.
		let native_balances: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, id, _)| *id == HDX)
			.map(|(acc, _, bal)| (*acc, *bal))
			.collect();

		pallet_balances::GenesisConfig::<Test> {
			balances: native_balances,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		// Non-native tokens go through orml_tokens (exclude native HDX).
		let non_native_balances: Vec<(AccountId, AssetId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|(_, id, _)| *id != HDX)
			.cloned()
			.collect();

		orml_tokens::GenesisConfig::<Test> {
			balances: non_native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
			// Clear thread-local state.
			REFERENDUM_OUTCOMES.with(|o| o.borrow_mut().clear());
			TRACK_IDS.with(|t| t.borrow_mut().clear());
			FORCE_REMOVE_VOTE_CALLS.with(|c| c.borrow_mut().clear());
		});

		ext
	}
}

pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		System::set_block_number(System::block_number() + 1);
	}
}
