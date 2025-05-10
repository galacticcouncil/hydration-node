use crate as pallet_liquidation;
use crate::types::money_market::Function;
use crate::*;
use ethabi::ethereum_types::H160;
use evm::{ExitError, ExitSucceed};
use frame_support::sp_runtime::traits::CheckedConversion;
use frame_support::{
	assert_ok, parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		BuildStorage, FixedU128, MultiSignature, Permill,
	},
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate},
		Everything, Nothing,
	},
};
use frame_system::{EnsureRoot, EnsureSigned};
use hex_literal::hex;
use hydra_dx_math::{ema::EmaPrice, ratio::Ratio};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::fee::GetDynamicFee;
use hydradx_traits::{router::PoolType, OraclePeriod, PriceOracle};
use orml_traits::parameter_type_with_key;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use pallet_omnipool::traits::ExternalPriceProvider;
use sp_core::H256;

type Block = frame_system::mocking::MockBlock<Test>;

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 3;

pub const ONE: Balance = 1_000_000_000_000;
pub const ALICE_HDX_INITIAL_BALANCE: Balance = 1_000_000_000_000 * ONE;
pub const ALICE_DOT_INITIAL_BALANCE: Balance = 1_000_000_000_000 * ONE;

pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const MONEY_MARKET: AccountId = AccountId::new([9; 32]);

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Balances: pallet_balances,
		 Tokens: orml_tokens,
		 Currencies: pallet_currencies,
		 AssetRegistry: pallet_asset_registry,
		 Omnipool: pallet_omnipool,
		 Router: pallet_route_executor,
		 EvmAccounts: pallet_evm_accounts,
		 Liquidation: pallet_liquidation,
		 Broadcast: pallet_broadcast,
	 }
);

parameter_types! {
	pub const LiquidationGasLimit: u64 = 1_000_000;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
}

fn decode_liquidation_call_data(data: Vec<u8>) -> Option<(EvmAddress, EvmAddress, EvmAddress, crate::Balance, bool)> {
	if data.len() != 164 {
		return None;
	}
	let data = data.clone();

	let function_u32: u32 = u32::from_be_bytes(data[0..4].try_into().ok()?);
	let function = Function::try_from(function_u32).ok()?;
	if function == Function::LiquidationCall {
		let collateral_asset = EvmAddress::from(H256::from_slice(&data[4..36]));
		let debt_asset = EvmAddress::from(H256::from_slice(&data[36..68]));
		let user = EvmAddress::from(H256::from_slice(&data[68..100]));
		let debt_to_cover = Balance::try_from(U256::checked_from(&data[100..132])?).ok()?;
		let receive_atoken = !H256(data[132..164].try_into().unwrap()).is_zero();

		Some((collateral_asset, debt_asset, user, debt_to_cover, receive_atoken))
	} else {
		None
	}
}

pub struct EvmMock;
impl EVM<CallResult> for EvmMock {
	fn call(context: CallContext, data: Vec<u8>, _value: U256, _gas: u64) -> CallResult {
		let maybe_data = decode_liquidation_call_data(data);
		match maybe_data {
			Some(data) => {
				let collateral_asset = HydraErc20Mapping::decode_evm_address(data.0);
				let debt_asset = HydraErc20Mapping::decode_evm_address(data.1);

				if collateral_asset.is_none() || debt_asset.is_none() {
					return (ExitReason::Error(ExitError::DesignatedInvalid), vec![]);
				};

				let collateral_asset = collateral_asset.unwrap();
				let debt_asset = debt_asset.unwrap();

				let caller = EvmAccounts::account_id(context.sender);
				let contract_addr = EvmAccounts::account_id(context.contract);
				let amount = data.3;

				let first_transfer_result = Currencies::transfer(
					RuntimeOrigin::signed(caller.clone()),
					contract_addr.clone(),
					debt_asset,
					amount,
				);
				let second_transfer_result = Currencies::transfer(
					RuntimeOrigin::signed(contract_addr),
					caller,
					collateral_asset,
					2 * amount,
				);

				if first_transfer_result.is_err() || second_transfer_result.is_err() {
					return (ExitReason::Error(ExitError::DesignatedInvalid), vec![]);
				}
			}
			None => return (ExitReason::Error(ExitError::DesignatedInvalid), vec![]),
		}

		(ExitReason::Succeed(ExitSucceed::Returned), vec![])
	}

	fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> CallResult {
		unimplemented!()
	}
}

pub struct HydraErc20Mapping;
impl Erc20Mapping<AssetId> for HydraErc20Mapping {
	fn asset_address(asset_id: AssetId) -> EvmAddress {
		Self::encode_evm_address(asset_id)
	}
	fn address_to_asset(address: EvmAddress) -> Option<AssetId> {
		Self::decode_evm_address(address)
	}
}
impl Erc20Encoding<AssetId> for HydraErc20Mapping {
	fn encode_evm_address(asset_id: AssetId) -> EvmAddress {
		let asset_id_bytes: [u8; 4] = asset_id.to_le_bytes();

		let mut evm_address_bytes = [0u8; 20];

		evm_address_bytes[15] = 1;

		for i in 0..4 {
			evm_address_bytes[16 + i] = asset_id_bytes[3 - i];
		}

		EvmAddress::from(evm_address_bytes)
	}

	fn decode_evm_address(evm_address: EvmAddress) -> Option<AssetId> {
		if !is_asset_address(evm_address) {
			return None;
		}

		let mut asset_id: u32 = 0;
		for byte in evm_address.as_bytes() {
			asset_id = (asset_id << 8) | (*byte as u32);
		}

		Some(asset_id)
	}
}

pub fn is_asset_address(address: H160) -> bool {
	let asset_address_prefix = &(H160::from(hex!("0000000000000000000000000000000100000000"))[0..16]);

	&address.to_fixed_bytes()[0..16] == asset_address_prefix
}

pub struct DummyGasWeightMapping;
impl pallet_evm::GasWeightMapping for DummyGasWeightMapping {
	fn gas_to_weight(_gas: u64, _without_base_weight: bool) -> Weight {
		Weight::zero()
	}
	fn weight_to_gas(_weight: Weight) -> u64 {
		0
	}
}
impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = FungibleCurrencies<Test>;
	type Evm = EvmMock;
	type Router = Router;
	type EvmAccounts = EvmAccounts;
	type Erc20Mapping = HydraErc20Mapping;
	type GasWeightMapping = DummyGasWeightMapping;
	type GasLimit = LiquidationGasLimit;
	type ProfitReceiver = TreasuryAccount;
	type RouterWeightInfo = ();
	type WeightInfo = ();
}

parameter_types! {
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
	pub const RouteValidationOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
}

pub struct PriceProviderMock {}

impl PriceOracle<AssetId> for PriceProviderMock {
	type Price = Ratio;

	fn price(route: &[Trade<AssetId>], _: OraclePeriod) -> Option<Ratio> {
		let has_insufficient_asset = route.iter().any(|t| t.asset_in > 2000 || t.asset_out > 2000);
		if has_insufficient_asset {
			return None;
		}
		Some(Ratio::new(88, 100))
	}
}

impl pallet_route_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = HDXAssetId;
	type Currency = FungibleCurrencies<Test>;
	type AMM = Omnipool;
	type OraclePriceProvider = PriceProviderMock;
	type OraclePeriod = RouteValidationOraclePeriod;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type ForceInsertOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

impl pallet_broadcast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
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
	type AccountData = pallet_balances::AccountData<u128>;
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
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const PositionCollectionId: u32 = 1000;

	pub const ExistentialDeposit: u128 = 500;
	pub ProtocolFee: Permill = Permill::from_percent(0);
	pub AssetFee: Permill = Permill::from_percent(0);
	pub BurnFee: Permill = Permill::from_percent(0);
	pub AssetWeightCap: Permill = Permill::from_percent(100);
	pub MinAddedLiquidity: Balance = 1000u128;
	pub MinTradeAmount: Balance = 1000u128;
	pub MaxInRatio: Balance = 1u128;
	pub MaxOutRatio: Balance = 1u128;
	pub const TVLCap: Balance = Balance::MAX;

	pub const TransactionByteFee: Balance = 10 * ONE / 100_000;

	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
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
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = HDXAssetId;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinTradingLimit: Balance = 1_000;
	pub const MinPoolLiquidity: Balance = 1_000;
	pub const DiscountedFee: (u32, u32) = (7, 10_000);
}

parameter_types! {
	#[derive(PartialEq, Debug)]
	pub RegistryStringLimit: u32 = 100;
	#[derive(PartialEq, Debug)]
	pub MinRegistryStringLimit: u32 = 2;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

type AssetLocation = u8;

impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureRoot<AccountId>;
	type Currency = Tokens;
	type UpdateOrigin = EnsureSigned<AccountId>;
	type AssetId = AssetId;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStringLimit;
	type MinStringLimit = MinRegistryStringLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type RegExternalWeightMultiplier = frame_support::traits::ConstU64<1>;
	type RegisterAssetHook = ();
	type WeightInfo = ();
}

impl pallet_omnipool::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type HubAssetId = LRNAAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PositionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = AssetRegistry;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type UpdateTradabilityOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u32;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type OmnipoolHooks = ();
	type PriceBarrier = ();
	type MinWithdrawalFee = ();
	type ExternalPriceOracle = WithdrawFeePriceOracle;
	type Fee = FeeProvider;
	type BurnProtocolFee = BurnFee;
}

pub struct DummyNFT;

impl Inspect<AccountId> for DummyNFT {
	type ItemId = u32;
	type CollectionId = u32;

	fn owner(_class: &Self::CollectionId, _instance: &Self::ItemId) -> Option<AccountId> {
		unimplemented!()
	}
}

impl Create<AccountId> for DummyNFT {
	fn create_collection(_class: &Self::CollectionId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::CollectionId, _instance: &Self::ItemId, _who: &AccountId) -> DispatchResult {
		Ok(())
	}

	fn burn(
		_class: &Self::CollectionId,
		_instance: &Self::ItemId,
		_maybe_check_owner: Option<&AccountId>,
	) -> DispatchResult {
		Ok(())
	}
}

pub struct WithdrawFeePriceOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for WithdrawFeePriceOracle {
	type Error = DispatchError;

	fn get_price(_asset_a: AssetId, _asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		unimplemented!()
	}

	fn get_price_weight() -> Weight {
		unimplemented!()
	}
}

pub struct FeeProvider;

impl GetDynamicFee<(AssetId, Balance)> for FeeProvider {
	type Fee = (Permill, Permill);
	fn get(_: (AssetId, Balance)) -> Self::Fee {
		(Permill::from_percent(0), Permill::from_percent(0))
	}

	fn get_and_store(key: (AssetId, Balance)) -> Self::Fee {
		Self::get(key)
	}
}

pub struct EvmNonceProviderMock;
impl pallet_evm_accounts::EvmNonceProvider for EvmNonceProviderMock {
	fn get_nonce(_evm_address: H160) -> U256 {
		U256::zero()
	}
}

impl pallet_evm_accounts::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type FeeMultiplier = ConstU32<10>;
	type EvmNonceProvider = EvmNonceProviderMock;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	init_pool: Option<(FixedU128, FixedU128)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, ALICE_HDX_INITIAL_BALANCE),
				(MONEY_MARKET, HDX, 1_000_000_000_000 * ONE),
				(MONEY_MARKET, DOT, 1_000_000_000_000 * ONE),
				(ALICE, DAI, 1_000_000_000_000_000_000 * ONE),
				(ALICE, DOT, ALICE_DOT_INITIAL_BALANCE),
				(BOB, HDX, 1_000_000_000 * ONE),
				(BOB, DOT, 1_000_000_000 * ONE),
				(Omnipool::protocol_account(), HDX, 1_000_000 * ONE),
				(Omnipool::protocol_account(), LRNA, 1_000_000 * ONE),
				(Omnipool::protocol_account(), DAI, 1_000_000 * ONE),
				(Omnipool::protocol_account(), DOT, 1_000_000 * ONE),
			],
			init_pool: Some((FixedU128::from_float(0.5), FixedU128::from(1))),
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let registered_assets = vec![
			(
				Some(LRNA),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"LRNA".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"LRNA".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(DAI),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DAI".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DAI".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(DOT),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
		];

		let mut initial_native_accounts: Vec<(AccountId, Balance)> = vec![];
		let additional_accounts: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|a| a.1 == HDX)
			.flat_map(|(x, _, amount)| vec![((*x).clone(), *amount)])
			.collect::<_>();

		initial_native_accounts.extend(additional_accounts);

		pallet_asset_registry::GenesisConfig::<Test> {
			registered_assets,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: initial_native_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();

		ext.execute_with(|| {
			System::set_block_number(1);
		});

		if let Some((stable_price, native_price)) = self.init_pool {
			ext.execute_with(|| {
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
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					DOT,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
			});
		}

		ext
	}
}
