// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate as dca;
use crate::{Config, Schedule};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::{Everything, GenesisBuild, Nothing};
use frame_support::weights::constants::ExtrinsicBaseWeight;
use frame_support::weights::IdentityFee;
use frame_support::weights::WeightToFeeCoefficient;
use frame_support::PalletId;

use frame_support::{assert_ok, parameter_types};
use frame_system as system;
use frame_system::pallet_prelude::OriginFor;
use frame_system::EnsureRoot;
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use hydradx_traits::Registry;
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrency;
use pallet_currencies::BasicCurrencyAdapter;
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_runtime::traits::Get;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::Perbill;
use sp_runtime::Permill;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, One},
	DispatchError,
};

use sp_runtime::{DispatchResult, FixedU128};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type BlockNumber = u64;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;
pub const REGISTERED_ASSET: AssetId = 1000;
pub const ONE_HUNDRED_BLOCKS: BlockNumber = 100;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const LP3: u64 = 3;

pub const ONE: Balance = 1_000_000_000_000;

pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

pub const DEFAULT_WEIGHT_CAP: u128 = 1_000_000_000_000_000_000;

pub const ALICE_INITIAL_NATIVE_BALANCE: u128 = 1000;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system,
		 DCA: dca,
		 Tokens: orml_tokens,
		 Omnipool: pallet_omnipool,
		 MultiTransactionPayment: pallet_transaction_multi_payment,
		 TransasctionPayment: pallet_transaction_payment,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 RelaychainInfo: pallet_relaychain_info,
	 }
);

lazy_static::lazy_static! {
	pub static ref OriginalStorageBondInNative: Balance = 2_000_000;
}

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static FEE_ASSET: RefCell<Vec<(u64,AssetId)>> = RefCell::new(vec![(ALICE,HDX)]);
	pub static STORAGE_BOND: RefCell<Balance> = RefCell::new(*OriginalStorageBondInNative);
	pub static EXECUTION_BOND: RefCell<Balance> = RefCell::new(1_000_000);
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		One::one()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
	type ReserveIdentifier = ();
	type MaxReserves = ();
}

parameter_types! {
		pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const DAIAssetId: AssetId = DAI;
	pub const PosiitionCollectionId: u32= 1000;

	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;
	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;

	pub const TransactionByteFee: Balance = 10 * ONE / 100_000;

	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_omnipool::Config for Test {
	type Event = Event;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type HubAssetId = LRNAAssetId;
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
	type StableCoinAssetId = DAIAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PosiitionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u32;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
}

pub struct SpotPriceProviderStub;

impl hydradx_traits::pools::SpotPriceProvider<AssetId> for SpotPriceProviderStub {
	type Price = FixedU128;

	fn pair_exists(_asset_a: AssetId, _asset_b: AssetId) -> bool {
		true
	}

	fn spot_price(_asset_a: AssetId, _asset_b: AssetId) -> Option<Self::Price> {
		Some(FixedU128::from_float(0.6))
	}
}

pub struct WeightToFee;

impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, MAXIMUM_BLOCK_WEIGHT]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT
		let p = ONE; // 1_000_000_000_000
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time()); // 7_919_840_000
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q, // 124
		}]
	}
}

impl pallet_transaction_multi_payment::Config for Test {
	type Event = Event;
	type AcceptedCurrencyOrigin = EnsureRoot<AccountId>;
	type Currencies = Tokens;
	type SpotPriceProvider = SpotPriceProviderStub;
	type WeightInfo = ();
	type WithdrawFeeForSetCurrency = ();
	type WeightToFee = WeightToFee;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
}

impl pallet_transaction_payment::Config for Test {
	type Event = Event;
	type OnChargeTransaction = TransferFees<Currencies, MultiTransactionPayment, DepositAll<Test>>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = ();
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

impl pallet_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub ExecutionBondInNativeCurrency: Balance= EXECUTION_BOND.with(|v| *v.borrow());
	pub StorageBondInNativeCurrency: Balance= STORAGE_BOND.with(|v| *v.borrow());
	pub MaxSchedulePerBlock: u32 = 20;
}

pub struct BlockNumberProviderMock {}

impl BlockNumberProvider for BlockNumberProviderMock {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		todo!()
	}
}

impl pallet_relaychain_info::Config for Test {
	type Event = Event;
	type RelaychainBlockNumberProvider = BlockNumberProviderMock;
}

impl Config for Test {
	type Event = Event;
	type Asset = AssetId;
	type AccountCurrencyAndPriceProvider = MultiTransactionPayment;
	type MultiReservableCurrency = Currencies;
	type SpotPriceProvider = Omnipool;
	type RandomnessProvider = DCA;
	type ExecutionBondInNativeCurrency = ExecutionBondInNativeCurrency;
	type StorageBondInNativeCurrency = StorageBondInNativeCurrency;
	type MaxSchedulePerBlock = MaxSchedulePerBlock;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
	type WeightToFee = IdentityFee<Balance>;
	type WeightInfo = ();
}
use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::weights::{ConstantMultiplier, WeightToFeeCoefficients, WeightToFeePolynomial};
use hydradx_traits::pools::SpotPriceProvider;
use pallet_relaychain_info::OnValidationDataHandler;
use pallet_transaction_multi_payment::{DepositAll, TransactionMultiPaymentDataProvider, TransferFees};
use smallvec::smallvec;
use test_utils::last_events;

pub struct DummyNFT;

impl<AccountId: From<u64>> Inspect<AccountId> for DummyNFT {
	type ItemId = u32;
	type CollectionId = u32;

	fn owner(_class: &Self::CollectionId, instance: &Self::ItemId) -> Option<AccountId> {
		let mut owner: Option<AccountId> = None;

		POSITIONS.with(|v| {
			if let Some(o) = v.borrow().get(instance) {
				owner = Some((*o).into());
			}
		});
		owner
	}
}

impl<AccountId: From<u64>> Create<AccountId> for DummyNFT {
	fn create_collection(_class: &Self::CollectionId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u64> + Into<u64> + Copy> Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::CollectionId, _instance: &Self::ItemId, _who: &AccountId) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(*_instance, (*_who).into());
		});
		Ok(())
	}

	fn burn(
		_class: &Self::CollectionId,
		instance: &Self::ItemId,
		_maybe_check_owner: Option<&AccountId>,
	) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.remove(instance);
		});
		Ok(())
	}
}

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Registry<T::AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	fn exists(asset_id: T::AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		Ok(T::AssetId::default())
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(T::AssetId::from(assigned))
	}
}

pub(crate) fn get_mock_minted_position(position_id: u32) -> Option<u64> {
	POSITIONS.with(|v| v.borrow().get(&position_id).copied())
}

pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	asset_fee: Permill,
	protocol_fee: Permill,
	asset_weight_cap: Permill,
	min_liquidity: u128,
	min_trade_limit: u128,
	register_stable_asset: bool,
	max_in_ratio: Balance,
	max_out_ratio: Balance,
	fee_asset_for_all_users: Vec<(u64, AssetId)>,
	init_pool: Option<(FixedU128, FixedU128)>,
	pool_tokens: Vec<(AssetId, FixedU128, AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		POSITIONS.with(|v| {
			v.borrow_mut().clear();
		});
		ASSET_WEIGHT_CAP.with(|v| {
			*v.borrow_mut() = Permill::from_percent(100);
		});
		ASSET_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});
		MIN_ADDED_LIQUDIITY.with(|v| {
			*v.borrow_mut() = 1000u128;
		});
		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = 1000u128;
		});
		MAX_IN_RATIO.with(|v| {
			*v.borrow_mut() = 1u128;
		});
		MAX_OUT_RATIO.with(|v| {
			*v.borrow_mut() = 1u128;
		});

		FEE_ASSET.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)],
			asset_fee: Permill::from_percent(0),
			protocol_fee: Permill::from_percent(0),
			asset_weight_cap: Permill::from_percent(100),
			min_liquidity: 0,
			registered_assets: vec![],
			min_trade_limit: 0,
			init_pool: None,
			register_stable_asset: true,
			pool_tokens: vec![],
			fee_asset_for_all_users: vec![],
			max_in_ratio: 1u128,
			max_out_ratio: 1u128,
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn add_endowed_accounts(mut self, account: (u64, AssetId, Balance)) -> Self {
		self.endowed_accounts.push(account);
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId) -> Self {
		self.registered_assets.push(asset);
		self
	}

	pub fn with_asset_weight_cap(mut self, cap: Permill) -> Self {
		self.asset_weight_cap = cap;
		self
	}

	pub fn with_asset_fee(mut self, fee: Permill) -> Self {
		self.asset_fee = fee;
		self
	}

	pub fn with_protocol_fee(mut self, fee: Permill) -> Self {
		self.protocol_fee = fee;
		self
	}
	pub fn with_min_added_liquidity(mut self, limit: Balance) -> Self {
		self.min_liquidity = limit;
		self
	}

	pub fn with_min_trade_amount(mut self, limit: Balance) -> Self {
		self.min_trade_limit = limit;
		self
	}

	pub fn with_initial_pool(mut self, stable_price: FixedU128, native_price: FixedU128) -> Self {
		self.init_pool = Some((stable_price, native_price));
		self
	}

	pub fn without_stable_asset_in_registry(mut self) -> Self {
		self.register_stable_asset = false;
		self
	}
	pub fn with_max_in_ratio(mut self, value: Balance) -> Self {
		self.max_in_ratio = value;
		self
	}
	pub fn with_max_out_ratio(mut self, value: Balance) -> Self {
		self.max_out_ratio = value;
		self
	}

	pub fn with_fee_asset(mut self, user_and_asset: Vec<(u64, AssetId)>) -> Self {
		self.fee_asset_for_all_users = user_and_asset;
		self
	}

	pub fn with_token(
		mut self,
		asset_id: AssetId,
		price: FixedU128,
		position_owner: AccountId,
		amount: Balance,
	) -> Self {
		self.pool_tokens.push((asset_id, price, position_owner, amount));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		// Add DAi and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			if self.register_stable_asset {
				v.borrow_mut().insert(DAI, DAI);
			}
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(REGISTERED_ASSET, REGISTERED_ASSET);
			self.registered_assets.iter().for_each(|asset| {
				v.borrow_mut().insert(*asset, *asset);
			});
		});

		ASSET_FEE.with(|v| {
			*v.borrow_mut() = self.asset_fee;
		});
		ASSET_WEIGHT_CAP.with(|v| {
			*v.borrow_mut() = self.asset_weight_cap;
		});

		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = self.protocol_fee;
		});

		MIN_ADDED_LIQUDIITY.with(|v| {
			*v.borrow_mut() = self.min_liquidity;
		});

		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = self.min_trade_limit;
		});
		MAX_IN_RATIO.with(|v| {
			*v.borrow_mut() = self.max_in_ratio;
		});
		MAX_OUT_RATIO.with(|v| {
			*v.borrow_mut() = self.max_out_ratio;
		});

		FEE_ASSET.with(|v| {
			*v.borrow_mut() = self.fee_asset_for_all_users;
		});

		pallet_balances::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.filter(|a| a.1 == HDX)
				.flat_map(|(x, asset, amount)| vec![(*x, *amount)])
				.collect(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.flat_map(|(x, asset, amount)| vec![(*x, *asset, *amount)])
				.collect(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		if let Some((stable_price, native_price)) = self.init_pool {
			r.execute_with(|| {
				Omnipool::set_tvl_cap(Origin::root(), u128::MAX);

				let stable_amount = Tokens::free_balance(DAI, &Omnipool::protocol_account());
				let native_amount = Tokens::free_balance(HDX, &Omnipool::protocol_account());

				assert_ok!(Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				));

				for (asset_id, price, owner, amount) in self.pool_tokens {
					assert_ok!(Tokens::transfer(
						Origin::signed(owner),
						Omnipool::protocol_account(),
						asset_id,
						amount
					));
					assert_ok!(Omnipool::add_token(
						Origin::root(),
						asset_id,
						price,
						self.asset_weight_cap,
						owner
					));
				}
			});
		}

		r.execute_with(|| {
			FEE_ASSET.borrow().with(|v| {
				let user_and_fee_assets = v.borrow().deref().clone();
				for fee_asset in user_and_fee_assets {
					MultiTransactionPayment::add_currency(Origin::root(), fee_asset.1, FixedU128::from_inner(1000000));
					MultiTransactionPayment::set_currency(Origin::signed(fee_asset.0), fee_asset.1);
				}
			});
		});

		r
	}
}

thread_local! {
	pub static DummyThreadLocal: RefCell<u128> = RefCell::new(100);
}

pub fn expect_suspended_events(e: Vec<Event>) {
	let last_events: Vec<Event> = get_last_suspended_events();
	assert_eq!(last_events, e);
}

pub fn get_last_suspended_events() -> Vec<Event> {
	let last_events: Vec<Event> = last_events::<Event, Test>(100);
	let mut suspended_events = vec![];

	for event in last_events {
		let e = event.clone();
		if let crate::tests::Event::DCA(dca::Event::Suspended { id, who }) = e {
			suspended_events.push(event.clone());
		}
	}

	suspended_events
}

pub fn expect_events(e: Vec<Event>) {
	test_utils::expect_events::<Event, Test>(e);
}
