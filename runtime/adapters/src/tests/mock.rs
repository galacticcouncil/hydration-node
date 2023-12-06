// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for Assets pallet.

use primitives::Amount;

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::traits::{ConstU128, Contains, Everything};
use frame_support::weights::Weight;
use frame_support::{
	assert_ok, construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::EnsureRoot;
use hydra_dx_math::ema::EmaPrice;
use hydra_dx_math::support::rational::Rounding;
use hydra_dx_math::to_u128_wrapper;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use hydradx_traits::{router::PoolType, AssetKind, AssetPairAccountIdFor, CanCreatePool, Registry, ShareTokenRegistry};
use orml_traits::{parameter_type_with_key, GetByKey};
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_currencies::BasicCurrencyAdapter;
use pallet_omnipool;
use pallet_omnipool::traits::EnsurePriceWithin;
use pallet_omnipool::traits::ExternalPriceProvider;
use primitive_types::{U128, U256};
use sp_core::H256;
use sp_runtime::traits::Zero;
use sp_runtime::Permill;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError, DispatchResult, FixedU128,
};
use std::cell::RefCell;
use std::collections::HashMap;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;

pub const DAVE: AccountId = 2;
pub const CHARLIE: AccountId = 3;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 3;

pub const REGISTERED_ASSET: AssetId = 1000;

pub const ONE: Balance = 1_000_000_000_000;

pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

thread_local! {
	pub static DUSTER_WHITELIST: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_PRICE_DIFF: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static EXT_PRICE_ADJUSTMENT: RefCell<(u32,u32, bool)> = RefCell::new((0u32,0u32, false));
	pub static WITHDRAWAL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static WITHDRAWAL_ADJUSTMENT: RefCell<(u32,u32, bool)> = RefCell::new((0u32,0u32, false));
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Omnipool: pallet_omnipool,
		Tokens: orml_tokens,
		RouteExecutor: pallet_route_executor,
		Currencies: pallet_currencies,
		XYK: pallet_xyk,
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const PosiitionCollectionId: u32= 1000;

	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;
	pub MaxPriceDiff: Permill = MAX_PRICE_DIFF.with(|v| *v.borrow());
	pub FourPercentDiff: Permill = Permill::from_percent(4);
	pub MinWithdrawFee: Permill = WITHDRAWAL_FEE.with(|v| *v.borrow());
}

impl pallet_omnipool::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type Fee = FeeProvider;
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
	type OmnipoolHooks = ();
	type PriceBarrier = (
		EnsurePriceWithin<AccountId, AssetId, MockOracle, FourPercentDiff, ()>,
		EnsurePriceWithin<AccountId, AssetId, MockOracle, MaxPriceDiff, ()>,
	);
	type MinWithdrawalFee = MinWithdrawFee;
	type ExternalPriceOracle = WithdrawFeePriceOracle;
}

pub struct FeeProvider;

impl GetByKey<AssetId, (Permill, Permill)> for FeeProvider {
	fn get(_: &AssetId) -> (Permill, Permill) {
		(ASSET_FEE.with(|v| *v.borrow()), PROTOCOL_FEE.with(|v| *v.borrow()))
	}
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}
parameter_types! {
	pub const StableAssetId: AssetId = 2;
	pub const MinTradingLimit : Balance = 1_000u128;
	pub const MinPoolLiquidity: Balance = 1_000_000u128;

	pub MinimumWithdrawalFee: Permill = Permill::from_rational(1u32,10000);
	pub XYKExchangeFee: (u32, u32) = (3, 1_000);
	pub const DiscountedFee: (u32, u32) = (7, 10_000);
}

impl pallet_xyk::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetRegistry = DummyRegistry<Test>;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currencies;
	type NativeAssetId = HDXAssetId;
	type WeightInfo = ();
	type GetExchangeFee = XYKExchangeFee;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type OracleSource = ();
	type CanCreatePool = DummyCanCreatePool;
	type AMMHandler = ();
	type DiscountedFee = DiscountedFee;
	type NonDustableWhitelistHandler = DummyDuster;
}

pub struct Whitelist;

impl Contains<AccountId> for Whitelist {
	fn contains(account: &AccountId) -> bool {
		DUSTER_WHITELIST.with(|v| v.borrow().contains(account))
	}
}

pub struct DummyDuster;

impl DustRemovalAccountWhitelist<AccountId> for DummyDuster {
	type Error = DispatchError;

	fn add_account(account: &AccountId) -> Result<(), Self::Error> {
		if Whitelist::contains(account) {
			return Err(sp_runtime::DispatchError::Other("Account is already in the whitelist"));
		}

		DUSTER_WHITELIST.with(|v| v.borrow_mut().push(*account));

		Ok(())
	}

	fn remove_account(account: &AccountId) -> Result<(), Self::Error> {
		DUSTER_WHITELIST.with(|v| {
			let mut v = v.borrow_mut();

			let idx = v.iter().position(|x| *x == *account).unwrap();
			v.remove(idx);

			Ok(())
		})
	}
}

pub struct DummyCanCreatePool;

impl CanCreatePool<AssetId> for DummyCanCreatePool {
	fn can_create(_: AssetId, _: AssetId) -> bool {
		true
	}
}

pub struct AssetPairAccountIdTest();
impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b)
		}
		(a * 1000 + b) as u64
	}
}
pub const ASSET_PAIR_ACCOUNT: AccountId = 12;
//pub const ASSET_PAIR_ACCOUNT: [u8; 32] = [4u8; 32];

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
}

type Pools = (Omnipool, XYK);

impl pallet_route_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = NativeCurrencyId;
	type Currency = FungibleCurrencies<Test>;
	type AMM = Pools;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type WeightInfo = ();
}

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
		MAX_PRICE_DIFF.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});
		EXT_PRICE_ADJUSTMENT.with(|v| {
			*v.borrow_mut() = (0, 0, false);
		});
		WITHDRAWAL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});
		WITHDRAWAL_ADJUSTMENT.with(|v| {
			*v.borrow_mut() = (0, 0, false);
		});

		Self {
			endowed_accounts: vec![
				(Omnipool::protocol_account(), DAI, 1000 * ONE),
				(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			],
			asset_fee: Permill::from_percent(0),
			protocol_fee: Permill::from_percent(0),
			asset_weight_cap: Permill::from_percent(100),
			min_liquidity: 0,
			registered_assets: vec![],
			min_trade_limit: 0,
			init_pool: None,
			register_stable_asset: true,
			pool_tokens: vec![],
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
	pub fn with_initial_pool(mut self, stable_price: FixedU128, native_price: FixedU128) -> Self {
		self.init_pool = Some((stable_price, native_price));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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

		let mut initial_native_accounts: Vec<(AccountId, Balance)> = vec![(ASSET_PAIR_ACCOUNT, 10000 * ONE)];
		let additional_accounts: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|a| a.1 == HDX)
			.flat_map(|(x, _, amount)| vec![(*x, *amount)])
			.collect::<_>();

		initial_native_accounts.extend(additional_accounts);

		pallet_balances::GenesisConfig::<Test> {
			balances: initial_native_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut initial_accounts = vec![
			(ASSET_PAIR_ACCOUNT, LRNA, 10000 * ONE),
			(ASSET_PAIR_ACCOUNT, DAI, 10000 * ONE),
		];

		initial_accounts.extend(self.endowed_accounts);

		orml_tokens::GenesisConfig::<Test> {
			balances: initial_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		if let Some((stable_price, native_price)) = self.init_pool {
			r.execute_with(|| {
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					HDXAssetId::get(),
					native_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					DAI,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
				for (asset_id, price, owner, amount) in self.pool_tokens {
					assert_ok!(Tokens::transfer(
						RuntimeOrigin::signed(owner),
						Omnipool::protocol_account(),
						asset_id,
						amount
					));
					assert_ok!(Omnipool::add_token(
						RuntimeOrigin::root(),
						asset_id,
						price,
						self.asset_weight_cap,
						owner
					));
				}
			});
		}

		r
	}
}

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

impl<T: pallet_omnipool::Config> Registry<T::AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
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

	fn retrieve_asset_type(_asset_id: T::AssetId) -> Result<AssetKind, DispatchError> {
		unimplemented!()
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

impl<T: pallet_omnipool::Config> ShareTokenRegistry<T::AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	u32: From<<T as pallet_omnipool::Config>::AssetId>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	fn retrieve_shared_asset(_: &Vec<u8>, _: &[T::AssetId]) -> Result<T::AssetId, DispatchError> {
		Ok(T::AssetId::default())
	}

	fn create_shared_asset(_: &Vec<u8>, _: &[T::AssetId], _: Balance) -> Result<T::AssetId, DispatchError> {
		unimplemented!("not implemented method: create_shared_asset")
	}
}

pub struct MockOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for MockOracle {
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		assert_eq!(asset_a, LRNA);
		let asset_state = Omnipool::load_asset_state(asset_b)?;
		let price = EmaPrice::new(asset_state.hub_reserve, asset_state.reserve);
		let adjusted_price = EXT_PRICE_ADJUSTMENT.with(|v| {
			let (n, d, neg) = *v.borrow();
			let adjustment = EmaPrice::new(price.n * n as u128, price.d * d as u128);
			if neg {
				saturating_sub(price, adjustment)
			} else {
				saturating_add(price, adjustment)
			}
		});

		Ok(adjusted_price)
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
}

pub struct WithdrawFeePriceOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for WithdrawFeePriceOracle {
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		assert_eq!(asset_a, LRNA);
		let asset_state = Omnipool::load_asset_state(asset_b)?;
		let price = EmaPrice::new(asset_state.hub_reserve, asset_state.reserve);

		let adjusted_price = WITHDRAWAL_ADJUSTMENT.with(|v| {
			let (n, d, neg) = *v.borrow();
			let adjustment = EmaPrice::new(price.n * n as u128, price.d * d as u128);
			if neg {
				saturating_sub(price, adjustment)
			} else {
				saturating_add(price, adjustment)
			}
		});

		Ok(adjusted_price)
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
}

// Helper methods to work with Ema Price
pub(super) fn round_to_rational((n, d): (U256, U256), rounding: Rounding) -> EmaPrice {
	let shift = n.bits().max(d.bits()).saturating_sub(128);
	let (n, d) = if shift > 0 {
		let min_n = u128::from(!n.is_zero());
		let (bias_n, bias_d) = rounding.to_bias(1);
		let shifted_n = (n >> shift).low_u128();
		let shifted_d = (d >> shift).low_u128();
		(
			shifted_n.saturating_add(bias_n).max(min_n),
			shifted_d.saturating_add(bias_d).max(1),
		)
	} else {
		(n.low_u128(), d.low_u128())
	};
	EmaPrice::new(n, d)
}

pub(super) fn saturating_add(l: EmaPrice, r: EmaPrice) -> EmaPrice {
	if l.n.is_zero() || r.n.is_zero() {
		return EmaPrice::new(l.n, l.d);
	}
	let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(l.n, l.d, r.n, r.d);
	// n = l.n * r.d - r.n * l.d
	let n = l_n.full_mul(r_d).saturating_add(r_n.full_mul(l_d));
	// d = l.d * r.d
	let d = l_d.full_mul(r_d);
	round_to_rational((n, d), Rounding::Nearest)
}

pub(super) fn saturating_sub(l: EmaPrice, r: EmaPrice) -> EmaPrice {
	if l.n.is_zero() || r.n.is_zero() {
		return EmaPrice::new(l.n, l.d);
	}
	let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(l.n, l.d, r.n, r.d);
	// n = l.n * r.d - r.n * l.d
	let n = l_n.full_mul(r_d).saturating_sub(r_n.full_mul(l_d));
	// d = l.d * r.d
	let d = l_d.full_mul(r_d);
	round_to_rational((n, d), Rounding::Nearest)
}
