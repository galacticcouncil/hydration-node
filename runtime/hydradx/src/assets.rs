// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use super::*;
use crate::evm::Erc20Currency;
use crate::system::NativeAssetId;

use hydradx_adapters::{
	AssetFeeOraclePriceProvider, EmaOraclePriceAdapter, FreezableNFT, MultiCurrencyLockedBalance, OmnipoolHookAdapter,
	OracleAssetVolumeProvider, PriceAdjustmentAdapter, RelayChainBlockHashProvider, RelayChainBlockNumberProvider,
	StableswapHooksAdapter, VestingInfo,
};

pub use hydradx_traits::{
	registry::Inspect,
	router::{inverse_route, PoolType, Trade},
	AccountIdFor, AssetKind, AssetPairAccountIdFor, Liquidity, NativePriceOracle, OnTradeHandler, OraclePeriod, Source,
};
use pallet_currencies::BasicCurrencyAdapter;
use pallet_omnipool::{
	traits::{EnsurePriceWithin, OmnipoolHooks},
	weights::WeightInfo as OmnipoolWeights,
};
use pallet_otc::NamedReserveIdentifier;
use pallet_stableswap::weights::WeightInfo as StableswapWeights;
use pallet_transaction_multi_payment::{AddTxAssetOnAccount, RemoveTxAssetOnKilled};
use primitives::constants::{
	chain::{OMNIPOOL_SOURCE, XYK_SOURCE},
	currency::{NATIVE_EXISTENTIAL_DEPOSIT, UNITS},
	time::DAYS,
};
use sp_runtime::{traits::Zero, ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, Percent};

use crate::evm::precompiles::erc20_mapping::SetCodeForErc20Precompile;
use core::ops::RangeInclusive;
use frame_support::{
	parameter_types,
	sp_runtime::app_crypto::sp_core::crypto::UncheckedFrom,
	sp_runtime::traits::{One, PhantomData},
	sp_runtime::{FixedU128, Perbill, Permill},
	traits::{
		AsEnsureOriginWithArg, ConstU32, Contains, Currency, Defensive, EnsureOrigin, Imbalance, LockIdentifier,
		NeverEnsureOrigin, OnUnbalanced,
	},
	BoundedVec, PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use hydradx_traits::AMM;
use orml_traits::{
	currency::{MultiCurrency, MultiLockableCurrency, MutationHooks, OnDeposit, OnTransfer},
	GetByKey, Happened,
};
use pallet_dynamic_fees::types::FeeParams;
use pallet_lbp::weights::WeightInfo as LbpWeights;
use pallet_route_executor::{weights::WeightInfo as RouterWeights, AmmTradeWeights, MAX_NUMBER_OF_TRADES};
use pallet_staking::{
	types::{Action, Point},
	SigmoidPercentage,
};
use pallet_xyk::weights::WeightInfo as XykWeights;
use sp_std::num::NonZeroU16;
parameter_types! {
	pub const NativeExistentialDeposit: u128 = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

// pallet-treasury did not impl OnUnbalanced<Credit>, need an adapter to handle dust.
type CreditOf = frame_support::traits::fungible::Credit<<Runtime as frame_system::Config>::AccountId, Balances>;
type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;
pub struct DustRemovalAdapter;
impl OnUnbalanced<CreditOf> for DustRemovalAdapter {
	fn on_nonzero_unbalanced(amount: CreditOf) {
		let new_amount = NegativeImbalance::new(amount.peek());
		Treasury::on_nonzero_unbalanced(new_amount);
	}
}

parameter_types! {
	pub const MaxHolds: u32 = 0;
	pub const MaxFreezes: u32 = 0;
}

impl pallet_balances::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_balances::HydraWeight<Runtime>;
	type Balance = Balance;
	type DustRemoval = DustRemovalAdapter;
	type ExistentialDeposit = NativeExistentialDeposit;
	type AccountStore = System;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type MaxFreezes = MaxFreezes;
	type RuntimeFreezeReason = ();
}

pub struct CurrencyHooks;
impl MutationHooks<AccountId, AssetId, Balance> for CurrencyHooks {
	type OnDust = Duster;
	type OnSlash = ();
	type PreDeposit = SufficiencyCheck;
	type PostDeposit = ();
	type PreTransfer = SufficiencyCheck;
	type PostTransfer = ();
	type OnNewTokenAccount = AddTxAssetOnAccount<Runtime>;
	type OnKilledTokenAccount = (RemoveTxAssetOnKilled<Runtime>, OnKilledTokenAccount);
}

pub const SUFFICIENCY_LOCK: LockIdentifier = *b"insuffED";

parameter_types! {
	//NOTE: This should always be > 1 otherwise we will payout more than we collected as ED for
	//insufficient assets.
	pub InsufficientEDinHDX: Balance = FixedU128::from_rational(11, 10)
		.saturating_mul_int(<Runtime as pallet_balances::Config>::ExistentialDeposit::get());
}

pub struct SufficiencyCheck;
impl SufficiencyCheck {
	/// This function is used by `orml-toknes::MutationHooks` before a transaction is executed.
	/// It is called from `PreDeposit` and `PreTransfer`.
	/// If transferred asset is not sufficient asset, it calculates ED amount in user's fee asset
	/// and transfers it from user to treasury account.
	///
	/// If user's fee asset is not sufficient asset, it calculates ED amount in DOT and transfers it to treasury through a swap
	///
	/// Function also locks corresponding HDX amount in the treasury because returned ED to the users
	/// when the account is killed is in the HDX. We are collecting little bit more (currencty 10%)than
	/// we are paying back when account is killed.
	///
	/// We assume account already paid ED if it holds transferred insufficient asset so additional
	/// ED payment is not necessary.
	///
	/// NOTE: `OnNewTokenAccount` mutation hooks is not used because it can't fail so we would not
	/// be able to fail transactions e.g. if the user doesn't have enough funds to pay ED.
	///
	/// ED payment - transfer:
	/// - if both sender and dest. accounts are regular accounts, sender pays ED for dest. account.
	/// - if sender is whitelisted account, dest. accounts pays its own ED.
	///
	/// ED payment - deposit:
	/// - dest. accounts always pays its own ED no matter if it's whitelisted or not.
	///
	/// ED release:
	/// ED is always released on account kill to killed account, whitelisting doesn't matter.
	/// Released ED amount is calculated from locked HDX divided by number of accounts that paid
	/// ED.
	///
	/// WARN:
	/// `set_balance` - bypass `MutationHooks` so no one pays ED for these account but ED is still released
	/// when account is killed.
	///
	/// Emits `pallet_asset_registry::Event::ExistentialDepositPaid` when ED was paid.
	fn on_funds(asset: AssetId, paying_account: &AccountId, to: &AccountId) -> DispatchResult {
		if AssetRegistry::is_banned(asset) {
			return Err(DispatchError::Other("BannedAssetTransfer"));
		}

		//NOTE: To prevent duplicate ED collection we assume account already paid ED
		//if it has any amount of `asset`(exists in the storage).
		if !orml_tokens::Accounts::<Runtime>::contains_key(to, asset) && !AssetRegistry::is_sufficient(asset) {
			let fee_payment_asset = MultiTransactionPayment::account_currency(paying_account);

			let ed_in_fee_asset = if AssetRegistry::is_sufficient(fee_payment_asset) {
				let ed_in_fee_asset = MultiTransactionPayment::price(fee_payment_asset)
					.ok_or(pallet_transaction_multi_payment::Error::<Runtime>::UnsupportedCurrency)?
					.saturating_mul_int(InsufficientEDinHDX::get())
					.max(1);

				//NOTE: Account doesn't have enough funds to pay ED if this fail.
				<Currencies as MultiCurrency<AccountId>>::transfer(
					fee_payment_asset,
					paying_account,
					&TreasuryAccount::get(),
					ed_in_fee_asset,
				)
				.map_err(|_| orml_tokens::Error::<Runtime>::ExistentialDeposit)?;

				ed_in_fee_asset
			} else {
				let dot_asset_id = DotAssetId::get();

				let ed_in_dot = MultiTransactionPayment::price(dot_asset_id)
					.ok_or(pallet_transaction_multi_payment::Error::<Runtime>::UnsupportedCurrency)?
					.saturating_mul_int(InsufficientEDinHDX::get())
					.max(1);

				let amount_in_without_fee =
					XykPaymentAssetSupport::calculate_in_given_out(fee_payment_asset, dot_asset_id, ed_in_dot)?;
				let trade_fee = XykPaymentAssetSupport::calculate_fee_amount(amount_in_without_fee)?;
				let ed_in_fee_asset = amount_in_without_fee.saturating_add(trade_fee);

				//NOTE: Account doesn't have enough funds to pay ED if this fail.
				XykPaymentAssetSupport::buy(
					paying_account,
					fee_payment_asset,
					DotAssetId::get(),
					ed_in_dot,
					ed_in_fee_asset,
					&TreasuryAccount::get(),
				)
				.map_err(|_| orml_tokens::Error::<Runtime>::ExistentialDeposit)?;

				ed_in_fee_asset
			};

			//NOTE: we are locking little bit less than charging.
			let to_lock = pallet_balances::Locks::<Runtime>::get(TreasuryAccount::get())
				.iter()
				.find(|x| x.id == SUFFICIENCY_LOCK)
				.map(|p| p.amount)
				.unwrap_or_default()
				.saturating_add(<Runtime as pallet_balances::Config>::ExistentialDeposit::get());

			<Currencies as MultiLockableCurrency<AccountId>>::set_lock(
				SUFFICIENCY_LOCK,
				NativeAssetId::get(),
				&TreasuryAccount::get(),
				to_lock,
			)?;

			frame_system::Pallet::<Runtime>::inc_sufficients(to);

			pallet_asset_registry::ExistentialDepositCounter::<Runtime>::mutate(|v| *v = v.saturating_add(1));

			pallet_asset_registry::Pallet::<Runtime>::deposit_event(
				pallet_asset_registry::Event::<Runtime>::ExistentialDepositPaid {
					who: paying_account.clone(),
					fee_asset: fee_payment_asset,
					amount: ed_in_fee_asset,
				},
			);
		}

		Ok(())
	}
}

impl OnTransfer<AccountId, AssetId, Balance> for SufficiencyCheck {
	fn on_transfer(asset: AssetId, from: &AccountId, to: &AccountId, _amount: Balance) -> DispatchResult {
		if pallet_route_executor::Pallet::<Runtime>::skip_ed_lock() {
			return Ok(());
		}

		//NOTE: `to` is paying ED if `from` is whitelisted.
		//This can happen if pallet's account transfers insufficient tokens to another account.
		if <Runtime as orml_tokens::Config>::DustRemovalWhitelist::contains(from) {
			Self::on_funds(asset, to, to)
		} else {
			Self::on_funds(asset, from, to)
		}
	}
}

impl OnDeposit<AccountId, AssetId, Balance> for SufficiencyCheck {
	fn on_deposit(asset: AssetId, to: &AccountId, _amount: Balance) -> DispatchResult {
		Self::on_funds(asset, to, to)
	}
}

pub struct OnKilledTokenAccount;
impl Happened<(AccountId, AssetId)> for OnKilledTokenAccount {
	fn happened((who, asset): &(AccountId, AssetId)) {
		if pallet_route_executor::Pallet::<Runtime>::skip_ed_unlock() {
			return;
		}

		if AssetRegistry::is_sufficient(*asset) || frame_system::Pallet::<Runtime>::account(who).sufficients.is_zero() {
			return;
		}

		let (ed_to_refund, locked_ed) = RefundAndLockedEdCalculator::calculate();
		let paid_counts = pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get();

		let to_lock = locked_ed.saturating_sub(ed_to_refund);

		if to_lock.is_zero() {
			let _ = <Currencies as MultiLockableCurrency<AccountId>>::remove_lock(
				SUFFICIENCY_LOCK,
				NativeAssetId::get(),
				&TreasuryAccount::get(),
			)
			.defensive();
		} else {
			let _ = <Currencies as MultiLockableCurrency<AccountId>>::set_lock(
				SUFFICIENCY_LOCK,
				NativeAssetId::get(),
				&TreasuryAccount::get(),
				to_lock,
			)
			.defensive();
		}

		let _ = <Currencies as MultiCurrency<AccountId>>::transfer(
			NativeAssetId::get(),
			&TreasuryAccount::get(),
			who,
			ed_to_refund,
		);

		frame_system::Pallet::<Runtime>::dec_sufficients(who);
		pallet_asset_registry::ExistentialDepositCounter::<Runtime>::set(paid_counts.saturating_sub(1));
	}
}
pub struct RefundAndLockedEdCalculator;

impl RefundAndLockedEdCalculator {
	fn calculate() -> (Balance, Balance) {
		let locked_ed = pallet_balances::Locks::<Runtime>::get(TreasuryAccount::get())
			.iter()
			.find(|x| x.id == SUFFICIENCY_LOCK)
			.map(|p| p.amount)
			.unwrap_or_default();

		let paid_counts = pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get();
		let ed_to_refund = if paid_counts != 0 {
			locked_ed.saturating_div(paid_counts)
		} else {
			0
		};

		(ed_to_refund, locked_ed)
	}
}

impl RefundEdCalculator<Balance> for RefundAndLockedEdCalculator {
	fn calculate() -> Balance {
		let (ed_to_refund, _ed_to_lock) = Self::calculate();

		ed_to_refund
	}
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = weights::orml_tokens::HydraWeight<Runtime>;
	type ExistentialDeposits = AssetRegistry;
	type CurrencyHooks = CurrencyHooks;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = DustRemovalWhitelist;
}

// The latest versions of the orml-currencies pallet don't emit events.
// The infrastructure relies on the events from this pallet, so we use the latest version of
// the pallet that contains and emit events and was updated to the polkadot version we use.
impl pallet_currencies::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
	type Erc20Currency = Erc20Currency<Runtime>;
	type BoundErc20 = AssetRegistry;
	type GetNativeCurrencyId = NativeAssetId;
	type WeightInfo = weights::pallet_currencies::HydraWeight<Runtime>;
}

pub struct RootAsVestingPallet;
impl EnsureOrigin<RuntimeOrigin> for RootAsVestingPallet {
	type Success = AccountId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o).and_then(|o| match o {
			RawOrigin::Root => Ok(VestingPalletId::get().into_account_truncating()),
			r => Err(RuntimeOrigin::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		Ok(RuntimeOrigin::from(RawOrigin::Signed(zero_account_id)))
	}
}

parameter_types! {
	pub MinVestedTransfer: Balance = 100;
	pub const MaxVestingSchedules: u32 = 100;
	pub const VestingPalletId: PalletId = PalletId(*b"py/vstng");
}

impl orml_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MinVestedTransfer = MinVestedTransfer;
	type VestedTransferOrigin = RootAsVestingPallet;
	type WeightInfo = weights::orml_vesting::HydraWeight<Runtime>;
	type MaxVestingSchedules = MaxVestingSchedules;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
}

parameter_types! {
	pub ClaimMessagePrefix: &'static [u8] = b"I hereby claim all my HDX tokens to wallet:";
}

impl pallet_claims::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Prefix = ClaimMessagePrefix;
	type WeightInfo = weights::pallet_claims::HydraWeight<Runtime>;
	type Currency = Balances;
	type CurrencyBalance = Balance;
}

parameter_types! {
	#[derive(PartialEq, Debug)]
	pub const RegistryStrLimit: u32 = 32;
	#[derive(PartialEq, Debug)]
	pub const MinRegistryStrLimit: u32 = 3;
	pub const SequentialIdOffset: u32 = 1_000_000;
	pub const RegExternalWeightMultiplier: u64 = 10;
}

impl pallet_asset_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureRoot<AccountId>;
	type UpdateOrigin = SuperMajorityTechCommittee;
	type Currency = pallet_currencies::fungibles::FungibleCurrencies<Runtime>;
	type AssetId = AssetId;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStrLimit;
	type MinStringLimit = MinRegistryStrLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type RegExternalWeightMultiplier = RegExternalWeightMultiplier;
	type RegisterAssetHook = SetCodeForErc20Precompile;
	type WeightInfo = weights::pallet_asset_registry::HydraWeight<Runtime>;
}

parameter_types! {
	pub const CollectionDeposit: Balance = 0;
	pub const ItemDeposit: Balance = 0;
	pub const KeyLimit: u32 = 256;	// Max 256 bytes per key
	pub const ValueLimit: u32 = 1024;	// Max 1024 bytes per value
	pub const UniquesMetadataDepositBase: Balance = 1_000 * UNITS;
	pub const AttributeDepositBase: Balance = UNITS;
	pub const DepositPerByte: Balance = UNITS;
	pub const UniquesStringLimit: u32 = 72;
}

impl pallet_uniques::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Balances;
	type ForceOrigin = MajorityOfCouncil;
	// Standard collection creation is disallowed
	type CreateOrigin = AsEnsureOriginWithArg<NeverEnsureOrigin<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = UniquesStringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const LRNA: AssetId = 1;
	pub const MinTradingLimit : Balance = 1_000u128;
	pub const MinPoolLiquidity: Balance = 1_000_000u128;
	pub const MaxInRatio: Balance = 3u128;
	pub const MaxOutRatio: Balance = 3u128;
	pub const OmnipoolCollectionId: CollectionId = 1337u128;
	pub const EmaOracleSpotPriceLastBlock: OraclePeriod = OraclePeriod::LastBlock;
	pub const EmaOracleSpotPriceShort: OraclePeriod = OraclePeriod::Short;
	pub const OmnipoolMaxAllowedPriceDifference: Permill = Permill::from_percent(1);
	pub MinimumWithdrawalFee: Permill = Permill::from_rational(1u32,10000);
}

impl pallet_omnipool::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Currencies;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type AssetRegistry = AssetRegistry;
	type HdxAssetId = NativeAssetId;
	type HubAssetId = LRNA;
	type MinWithdrawalFee = MinimumWithdrawalFee;
	type MinimumTradingLimit = MinTradingLimit;
	type MinimumPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type PositionItemId = ItemId;
	type CollectionId = CollectionId;
	type NFTCollectionId = OmnipoolCollectionId;
	type NFTHandler = Uniques;
	type WeightInfo = weights::pallet_omnipool::HydraWeight<Runtime>;
	type OmnipoolHooks = OmnipoolHookAdapter<Self::RuntimeOrigin, NativeAssetId, LRNA, Runtime>;
	type PriceBarrier = (
		EnsurePriceWithin<
			AccountId,
			AssetId,
			EmaOraclePriceAdapter<EmaOracleSpotPriceLastBlock, Runtime>,
			OmnipoolMaxAllowedPriceDifference,
			CircuitBreakerWhitelist,
		>,
		EnsurePriceWithin<
			AccountId,
			AssetId,
			EmaOraclePriceAdapter<EmaOracleSpotPriceShort, Runtime>,
			OmnipoolMaxAllowedPriceDifference,
			CircuitBreakerWhitelist,
		>,
	);
	type ExternalPriceOracle = EmaOraclePriceAdapter<EmaOracleSpotPriceShort, Runtime>;
	type Fee = pallet_dynamic_fees::UpdateAndRetrieveFees<Runtime>;
}

pub struct CircuitBreakerWhitelist;

impl Contains<AccountId> for CircuitBreakerWhitelist {
	fn contains(a: &AccountId) -> bool {
		<PalletId as AccountIdConversion<AccountId>>::into_account_truncating(&TreasuryPalletId::get()) == *a
	}
}

parameter_types! {
	pub const DefaultMaxNetTradeVolumeLimitPerBlock: (u32, u32) = (5_000, 10_000);	// 50%
	pub const DefaultMaxLiquidityLimitPerBlock: Option<(u32, u32)> = Some((500, 10_000));	// 5%
}

impl pallet_circuit_breaker::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type WhitelistedAccounts = CircuitBreakerWhitelist;
	type DefaultMaxNetTradeVolumeLimitPerBlock = DefaultMaxNetTradeVolumeLimitPerBlock;
	type DefaultMaxAddLiquidityLimitPerBlock = DefaultMaxLiquidityLimitPerBlock;
	type DefaultMaxRemoveLiquidityLimitPerBlock = DefaultMaxLiquidityLimitPerBlock;
	type OmnipoolHubAsset = LRNA;
	type WeightInfo = weights::pallet_circuit_breaker::HydraWeight<Runtime>;
}

parameter_types! {
	pub SupportedPeriods: BoundedVec<OraclePeriod, ConstU32<{ pallet_ema_oracle::MAX_PERIODS }>> = BoundedVec::truncate_from(vec![
		OraclePeriod::LastBlock, OraclePeriod::Short, OraclePeriod::TenMinutes]);
}

pub struct OracleWhitelist<Runtime>(PhantomData<Runtime>);
impl Contains<(Source, AssetId, AssetId)> for OracleWhitelist<Runtime>
where
	Runtime: pallet_ema_oracle::Config + pallet_asset_registry::Config,
	AssetId: From<<Runtime as pallet_asset_registry::Config>::AssetId>,
{
	fn contains(t: &(Source, AssetId, AssetId)) -> bool {
		pallet_asset_registry::OracleWhitelist::<Runtime>::contains(t)
			|| pallet_ema_oracle::OracleWhitelist::<Runtime>::contains(t)
	}
}

impl pallet_ema_oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = SuperMajorityTechCommittee;
	/// The definition of the oracle time periods currently assumes a 6 second block time.
	/// We use the parachain blocks anyway, because we want certain guarantees over how many blocks correspond
	/// to which smoothing factor.
	type BlockNumberProvider = System;
	type SupportedPeriods = SupportedPeriods;
	type OracleWhitelist = OracleWhitelist<Runtime>;
	/// With every asset trading against LRNA we will only have as many pairs as there will be assets, so
	/// 40 seems a decent upper bound for the foreseeable future.
	type MaxUniqueEntries = ConstU32<40>;
	type WeightInfo = weights::pallet_ema_oracle::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	/// Should take care of the overhead introduced by `OracleWhitelist`.
	type BenchmarkHelper = RegisterAsset<Runtime>;
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a) || pallet_duster::DusterWhitelist::<Runtime>::contains(a)
	}
}

parameter_types! {
	pub const DustingReward: u128 = 0;
}

impl pallet_duster::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type MultiCurrency = Currencies;
	type MinCurrencyDeposits = AssetRegistry;
	type Reward = DustingReward;
	type NativeCurrencyId = NativeAssetId;
	type BlacklistUpdateOrigin = SuperMajorityTechCommittee;
	type TreasuryAccountId = TreasuryAccount;
	type WeightInfo = weights::pallet_duster::HydraWeight<Runtime>;
}

parameter_types! {
	pub const OmniWarehouseLMPalletId: PalletId = PalletId(*b"OmniWhLM");
	#[derive(PartialEq, Eq)]
	pub const MaxEntriesPerDeposit: u8 = 5; //NOTE: Rebenchmark when this change
	pub const MaxYieldFarmsPerGlobalFarm: u8 = 50; //NOTE: Includes deleted/destroyed farms
	pub const MinPlannedYieldingPeriods: BlockNumber = 14_440;  //1d with 6s blocks
	pub const MinTotalFarmRewards: Balance = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const OmnipoolLmOracle: [u8; 8] = OMNIPOOL_SOURCE;
}

type OmnipoolLiquidityMiningInstance = warehouse_liquidity_mining::Instance1;
impl warehouse_liquidity_mining::Config<OmnipoolLiquidityMiningInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Currencies;
	type PalletId = OmniWarehouseLMPalletId;
	type TreasuryAccountId = TreasuryAccount;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
	type AmmPoolId = AssetId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type AssetRegistry = AssetRegistry;
	type NonDustableWhitelistHandler = Duster;
	type PriceAdjustment = PriceAdjustmentAdapter<Runtime, OmnipoolLiquidityMiningInstance, OmnipoolLmOracle>;
}

parameter_types! {
	pub const OmniLMPalletId: PalletId = PalletId(*b"Omni//LM");
	pub const OmnipoolLMCollectionId: CollectionId = 2584_u128;
	pub const OmnipoolLMOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
	pub const OmnipoolLMOracleSource: Source = OMNIPOOL_SOURCE;
}

impl pallet_omnipool_liquidity_mining::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type CreateOrigin = AllTechnicalCommitteeMembers;
	type PalletId = OmniLMPalletId;
	type NFTCollectionId = OmnipoolLMCollectionId;
	type NFTHandler = Uniques;
	type LiquidityMiningHandler = OmnipoolWarehouseLM;
	type OracleSource = OmnipoolLMOracleSource;
	type OraclePeriod = OmnipoolLMOraclePeriod;
	type PriceOracle = EmaOracle;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type WeightInfo = weights::pallet_omnipool_liquidity_mining::HydraWeight<Runtime>;
}

parameter_types! {
	pub const XYKWarehouseLMPalletId: PalletId = PalletId(*b"xykLMpID");
	#[derive(PartialEq, Eq)]
	pub const XYKLmMaxEntriesPerDeposit: u8 = 5; //NOTE: Rebenchmark when this change
	pub const XYKLmMaxYieldFarmsPerGlobalFarm: u8 = 50; //NOTE: Includes deleted/destroyed farms
	pub const XYKLmMinPlannedYieldingPeriods: BlockNumber = 14_440;  //1d with 6s blocks
	pub const XYKLmMinTotalFarmRewards: Balance = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const XYKLmOracle: [u8; 8] = XYK_SOURCE;
}

type XYKLiquidityMiningInstance = warehouse_liquidity_mining::Instance2;
impl warehouse_liquidity_mining::Config<XYKLiquidityMiningInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Currencies;
	type PalletId = XYKWarehouseLMPalletId;
	type TreasuryAccountId = TreasuryAccount;
	type MinTotalFarmRewards = XYKLmMinTotalFarmRewards;
	type MinPlannedYieldingPeriods = XYKLmMinPlannedYieldingPeriods;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
	type AmmPoolId = AccountId;
	type MaxFarmEntriesPerDeposit = XYKLmMaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = XYKLmMaxYieldFarmsPerGlobalFarm;
	type AssetRegistry = AssetRegistry;
	type NonDustableWhitelistHandler = Duster;
	type PriceAdjustment = PriceAdjustmentAdapter<Runtime, XYKLiquidityMiningInstance, XYKLmOracle>;
}

parameter_types! {
	pub const XYKLmPalletId: PalletId = PalletId(*b"XYK///LM");
	pub const XYKLmCollectionId: CollectionId = 5389_u128;
}

impl pallet_xyk_liquidity_mining::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currencies = Currencies;
	type CreateOrigin = AllTechnicalCommitteeMembers;
	type PalletId = XYKLmPalletId;
	type NFTCollectionId = XYKLmCollectionId;
	type NFTHandler = Uniques;
	type LiquidityMiningHandler = XYKWarehouseLM;
	type NonDustableWhitelistHandler = Duster;
	type AMM = XYK;
	type AssetRegistry = AssetRegistry;
	type MaxFarmEntriesPerDeposit = XYKLmMaxEntriesPerDeposit;
	type WeightInfo = weights::pallet_xyk_liquidity_mining::HydraWeight<Runtime>;
}

// The reason why there is difference between PROD and benchmark is that it is not possible
// to set validation data in parachain system pallet in the benchmarks.
// So for benchmarking, we mock it out and return some hardcoded parent hash
pub struct RelayChainBlockHashProviderAdapter<Runtime>(sp_std::marker::PhantomData<Runtime>);

#[cfg(not(feature = "runtime-benchmarks"))]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		let validation_data = cumulus_pallet_parachain_system::ValidationData::<Runtime>::get();
		match validation_data {
			Some(data) => Some(data.parent_head.hash()),
			None => None,
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		None
	}
}

use hydradx_traits::pools::SpotPriceProvider;
#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::PriceOracle;

#[cfg(feature = "runtime-benchmarks")]
use hydra_dx_math::ema::EmaPrice;

#[cfg(feature = "runtime-benchmarks")]
pub struct DummyOraclePriceProvider;
#[cfg(feature = "runtime-benchmarks")]
impl PriceOracle<AssetId> for DummyOraclePriceProvider {
	type Price = EmaPrice;

	fn price(_route: &[Trade<AssetId>], _period: OraclePeriod) -> Option<Self::Price> {
		Some(EmaPrice::one())
	}
}

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::OraclePriceProvider;

#[cfg(feature = "runtime-benchmarks")]
pub struct DummySpotPriceProvider;
#[cfg(feature = "runtime-benchmarks")]
impl SpotPriceProvider<AssetId> for DummySpotPriceProvider {
	type Price = FixedU128;

	fn pair_exists(_asset_a: AssetId, _asset_b: AssetId) -> bool {
		true
	}

	fn spot_price(_asset_a: AssetId, _asset_b: AssetId) -> Option<Self::Price> {
		Some(FixedU128::one())
	}
}

pub const DOT_ASSET_LOCATION: AssetLocation = AssetLocation(polkadot_xcm::v3::MultiLocation::parent());

pub struct DotAssetId;
impl Get<AssetId> for DotAssetId {
	fn get() -> AssetId {
		let invalid_id =
			pallet_asset_registry::Pallet::<crate::Runtime>::next_asset_id().defensive_unwrap_or(AssetId::MAX);

		match pallet_asset_registry::Pallet::<crate::Runtime>::location_to_asset(DOT_ASSET_LOCATION) {
			Some(asset_id) => asset_id,
			None => invalid_id,
		}
	}
}

parameter_types! {
	pub MinBudgetInNativeCurrency: Balance = 1000 * UNITS;
	pub MaxSchedulesPerBlock: u32 = 20;
	pub MaxPriceDifference: Permill = Permill::from_rational(15u32, 1000u32);
	pub MaxConfigurablePriceDifference: Permill = Permill::from_percent(5);
	pub MinimalPeriod: u32 = 5;
	pub BumpChance: Percent = Percent::from_percent(17);
	pub NamedReserveId: NamedReserveIdentifier = *b"dcaorder";
	pub MaxNumberOfRetriesOnError: u8 = 3;
	pub DCAOraclePeriod: OraclePeriod = OraclePeriod::Short;

}

pub struct RetryOnErrorForDca;

impl Contains<DispatchError> for RetryOnErrorForDca {
	fn contains(t: &DispatchError) -> bool {
		let errors: Vec<DispatchError> = vec![
			pallet_omnipool::Error::<Runtime>::AssetNotFound.into(),
			pallet_omnipool::Error::<Runtime>::NotAllowed.into(),
		];
		errors.contains(t)
	}
}

impl pallet_dca::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type Currencies = Currencies;
	type RelayChainBlockHashProvider = RelayChainBlockHashProviderAdapter<Runtime>;
	type RandomnessProvider = DCA;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type OraclePriceProvider = OraclePriceProvider<AssetId, EmaOracle, LRNA>;
	#[cfg(feature = "runtime-benchmarks")]
	type OraclePriceProvider = DummyOraclePriceProvider;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type RouteExecutor = Router;
	#[cfg(feature = "runtime-benchmarks")]
	type RouteExecutor = pallet_route_executor::DummyRouter<Runtime>;
	type RouteProvider = Router;
	type MaxPriceDifferenceBetweenBlocks = MaxPriceDifference;
	type MaxConfigurablePriceDifferenceBetweenBlocks = MaxConfigurablePriceDifference;
	type MinimalPeriod = MinimalPeriod;
	type BumpChance = BumpChance;
	type MaxSchedulePerBlock = MaxSchedulesPerBlock;
	type MaxNumberOfRetriesOnError = MaxNumberOfRetriesOnError;
	type NativeAssetId = NativeAssetId;
	type MinBudgetInNativeCurrency = MinBudgetInNativeCurrency;
	type MinimumTradingLimit = MinTradingLimit;
	type FeeReceiver = TreasuryAccount;
	type NamedReserveId = NamedReserveId;
	type WeightToFee = WeightToFee;
	type AmmTradeWeights = RouterWeightInfo;
	type WeightInfo = weights::pallet_dca::HydraWeight<Runtime>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type NativePriceOracle = AssetFeeOraclePriceProvider<
		NativeAssetId,
		MultiTransactionPayment,
		Router,
		OraclePriceProvider<AssetId, EmaOracle, LRNA>,
		MultiTransactionPayment,
		DCAOraclePeriod,
	>;
	#[cfg(feature = "runtime-benchmarks")]
	type NativePriceOracle = AssetFeeOraclePriceProvider<
		NativeAssetId,
		MultiTransactionPayment,
		Router,
		DummyOraclePriceProvider,
		MultiTransactionPayment,
		DCAOraclePeriod,
	>;
	type RetryOnError = RetryOnErrorForDca;
	type PolkadotNativeAssetId = DotAssetId;
	type SwappablePaymentAssetSupport = XykPaymentAssetSupport;
}

// Provides weight info for the router. Router extrinsics can be executed with different AMMs, so we split the router weights into two parts:
// the router extrinsic overhead and the AMM weight.
pub struct RouterWeightInfo;
// Calculates the overhead of Router extrinsics. To do that, we benchmark Router::sell with single LBP trade and subtract the weight of LBP::sell.
// This allows us to calculate the weight of any route by adding the weight of AMM trades to the overhead of a router extrinsic.
impl RouterWeightInfo {
	pub fn sell_and_calculate_sell_trade_amounts_overhead_weight(
		num_of_calc_sell: u32,
		num_of_execute_sell: u32,
	) -> Weight {
		weights::pallet_route_executor::HydraWeight::<Runtime>::calculate_and_execute_sell_in_lbp(num_of_calc_sell)
			.saturating_sub(weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(
				num_of_calc_sell.saturating_add(num_of_execute_sell),
				num_of_execute_sell,
			))
	}

	pub fn buy_and_calculate_buy_trade_amounts_overhead_weight(
		num_of_calc_buy: u32,
		num_of_execute_buy: u32,
	) -> Weight {
		let router_weight = weights::pallet_route_executor::HydraWeight::<Runtime>::calculate_and_execute_buy_in_lbp(
			num_of_calc_buy,
			num_of_execute_buy,
		);
		// Handle this case separately. router_execution_buy provides incorrect weight for the case when only calculate_buy is executed.
		let lbp_weight = if (num_of_calc_buy, num_of_execute_buy) == (1, 0) {
			weights::pallet_lbp::HydraWeight::<Runtime>::calculate_buy()
		} else {
			weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_buy(
				num_of_calc_buy.saturating_add(num_of_execute_buy),
				num_of_execute_buy,
			)
		};
		router_weight.saturating_sub(lbp_weight)
	}

	pub fn set_route_overweight() -> Weight {
		let number_of_times_calculate_sell_amounts_executed = 5; //4 calculations + in the validation
		let number_of_times_execute_sell_amounts_executed = 0; //We do have it once executed in the validation of the route, but it is without writing to database (as rolled back), and since we pay back successful set_route, we just keep this overhead

		let set_route_overweight = weights::pallet_route_executor::HydraWeight::<Runtime>::set_route_for_xyk();

		// we substract weight of getting oracle price too as we add this back later based on the length of the route
		set_route_overweight
			.saturating_sub(weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_sell(
				number_of_times_calculate_sell_amounts_executed,
				number_of_times_execute_sell_amounts_executed,
			))
			.saturating_sub(weights::pallet_route_executor::HydraWeight::<Runtime>::get_oracle_price_for_xyk())
	}

	pub fn calculate_spot_price_overweight() -> Weight {
		Weight::from_parts(
			weights::pallet_route_executor::HydraWeight::<Runtime>::calculate_spot_price_with_fee_in_lbp()
				.ref_time()
				.saturating_sub(
					weights::pallet_lbp::HydraWeight::<Runtime>::calculate_spot_price_with_fee().ref_time(),
				),
			weights::pallet_route_executor::HydraWeight::<Runtime>::calculate_spot_price_with_fee_in_lbp().proof_size(),
		)
	}

	pub fn skip_ed_handling_overweight() -> Weight {
		weights::pallet_route_executor::HydraWeight::<Runtime>::skip_ed_handling_for_trade_with_insufficient_assets()
	}
}

impl AmmTradeWeights<Trade<AssetId>> for RouterWeightInfo {
	// Used in Router::sell extrinsic, which calls AMM::calculate_sell and AMM::execute_sell
	fn sell_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_sell is executed
		let e = 1; // number of times AMM::execute_sell is executed

		for trade in route {
			weight.saturating_accrue(Self::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_trade_weight(),
					)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_liquidity_changed_weight(),
					),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_sell(c, e)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		//We add the overweight for skipping ED handling if route has multiple trades and we have any insufficient asset
		if route.len() > 1
			&& route.iter().any(|trade| {
				!AssetRegistry::is_sufficient(trade.asset_in) || !AssetRegistry::is_sufficient(trade.asset_out)
			}) {
			weight.saturating_accrue(Self::skip_ed_handling_overweight());
		}

		weight
	}

	// Used in Router::buy extrinsic, which calls AMM::calculate_buy and AMM::execute_buy
	fn buy_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_buy is executed
		let e = 1; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_trade_weight(),
					)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_liquidity_changed_weight(),
					),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		//We add the overweight for skipping ED handling if we have any insufficient asset
		if route.len() > 1
			&& route.iter().any(|trade| {
				!AssetRegistry::is_sufficient(trade.asset_in) || !AssetRegistry::is_sufficient(trade.asset_out)
			}) {
			weight.saturating_accrue(Self::skip_ed_handling_overweight());
		}

		weight
	}

	// Used in DCA::schedule extrinsic, which calls Router::calculate_buy_trade_amounts
	fn calculate_buy_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_buy is executed
		let e = 0; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(1, 0));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in DCA::on_initialize for Order::Sell, which calls Router::calculate_sell_trade_amounts and Router::sell.
	fn sell_and_calculate_sell_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 2; // number of times AMM::calculate_sell is executed
		let e = 1; // number of times AMM::execute_sell is executed

		for trade in route {
			weight.saturating_accrue(Self::sell_and_calculate_sell_trade_amounts_overhead_weight(1, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_sell(c, e)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in DCA::on_initialize for Order::Buy, which calls 2 * Router::calculate_buy_trade_amounts and Router::buy.
	fn buy_and_calculate_buy_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 3; // number of times AMM::calculate_buy is executed
		let e = 1; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(2, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	fn set_route_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();

		//We ignore the calls for AMM:get_liquidty_depth, as the same logic happens in AMM calculation/execution

		//Overweight
		weight.saturating_accrue(Self::set_route_overweight());

		//Add a sell weight as we do a dry-run sell as validation
		weight.saturating_accrue(Self::sell_weight(route));

		//For the stored route we expect a worst case with max number of trades in the most expensive pool which is stableswap
		//We have have two sell calculation for that, normal and inverse
		weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_sell(2, 0)
			.checked_mul(MAX_NUMBER_OF_TRADES.into());

		//Calculate sell amounts for the new route
		for trade in route {
			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_sell(1, 0)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_sell(1, 0),
			};
			weight.saturating_accrue(amm_weight);
		}

		//Calculate sell amounts for the inversed new route
		for trade in inverse_route(route.to_vec()) {
			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::router_execution_sell(1, 0)
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::router_execution_sell(1, 0),
			};
			weight.saturating_accrue(amm_weight);
		}

		// Incorporate oracle price calculation
		// We use omnipool as reference as it is the worst case for calculating oracle price
		let weight_of_get_oracle_price_for_2_assets =
			weights::pallet_route_executor::HydraWeight::<Runtime>::get_oracle_price_for_omnipool();
		let weight_of_get_oracle_price_for_route =
			weight_of_get_oracle_price_for_2_assets.saturating_mul(route.len() as u64);
		weight.saturating_accrue(weight_of_get_oracle_price_for_route);

		weight
	}

	fn force_insert_route_weight() -> Weight {
		//Since we don't have any AMM specific thing in the extrinsic, we just return the plain weight
		weights::pallet_route_executor::HydraWeight::<Runtime>::force_insert_route()
	}

	// Used in OtcSettlements::settle_otc_order extrinsic
	fn calculate_spot_price_with_fee_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Self::calculate_spot_price_overweight();

		for trade in route {
			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::pallet_omnipool::HydraWeight::<Runtime>::calculate_spot_price_with_fee(),
				PoolType::LBP => weights::pallet_lbp::HydraWeight::<Runtime>::calculate_spot_price_with_fee(),
				PoolType::Stableswap(_) => {
					weights::pallet_stableswap::HydraWeight::<Runtime>::calculate_spot_price_with_fee()
				}
				PoolType::XYK => weights::pallet_xyk::HydraWeight::<Runtime>::calculate_spot_price_with_fee(),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	fn get_route_weight() -> Weight {
		weights::pallet_route_executor::HydraWeight::<Runtime>::get_route()
	}
}

parameter_types! {
	pub const DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
	pub const RouteValidationOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;

}

impl pallet_route_executor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type Currency = FungibleCurrencies<Runtime>;
	type WeightInfo = RouterWeightInfo;
	type AMM = (Omnipool, Stableswap, XYK, LBP);
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type NativeAssetId = NativeAssetId;
	type InspectRegistry = AssetRegistry;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type EdToRefundCalculator = RefundAndLockedEdCalculator;
	type OraclePriceProvider = hydradx_adapters::OraclePriceProvider<AssetId, EmaOracle, LRNA>;
	type BatchIdProvider = AmmSupport;
	type OraclePeriod = RouteValidationOraclePeriod;
	type OperationIdProvider = AmmSupport;
}

parameter_types! {
	pub const ExistentialDepositMultiplier: u8 = 5;
	pub const PricePrecision: FixedU128 = FixedU128::from_rational(1, 100);
	pub MinProfitPercentage: Perbill = Perbill::from_rational(1u32, 100_000_u32); // 0.001%
	pub OtcFee: Permill = Permill::from_rational(1u32, 1_000_u32); // 0.1%
}

impl pallet_otc::Config for Runtime {
	type AssetId = AssetId;
	type AssetRegistry = AssetRegistry;
	type Currency = Currencies;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposits = AssetRegistry;
	type ExistentialDepositMultiplier = ExistentialDepositMultiplier;
	type Fee = OtcFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = weights::pallet_otc::HydraWeight<Runtime>;
}

impl pallet_otc_settlements::Config for Runtime {
	type Currency = FungibleCurrencies<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type Router = Router;
	#[cfg(feature = "runtime-benchmarks")]
	type Router = pallet_route_executor::DummyRouter<Runtime>;
	type ProfitReceiver = TreasuryAccount;
	type MinProfitPercentage = MinProfitPercentage;
	type PricePrecision = PricePrecision;
	type MinTradingLimit = MinTradingLimit;
	type MaxIterations = ConstU32<40>;
	type WeightInfo = weights::pallet_otc_settlements::HydraWeight<Runtime>;
	type RouterWeightInfo = RouterWeightInfo;
}

// Dynamic fees
parameter_types! {
	pub AssetFeeParams: FeeParams<Permill> = FeeParams{
		min_fee: Permill::from_rational(25u32,10000u32), // 0.25%
		max_fee: Permill::from_rational(5u32,100u32),    // 5%
		decay: FixedU128::from_rational(1,100000),       // 0.001%
		amplification: FixedU128::from(2),               // 2
	};

	pub ProtocolFeeParams: FeeParams<Permill> = FeeParams{
		min_fee: Permill::from_rational(5u32,10000u32),  // 0.05%
		max_fee: Permill::from_rational(1u32,1000u32),   // 0.1%
		decay: FixedU128::from_rational(5,1000000),      // 0.0005%
		amplification: FixedU128::one(),                 // 1
	};

	pub const DynamicFeesOraclePeriod: OraclePeriod = OraclePeriod::Short;
}

impl pallet_dynamic_fees::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type Fee = Permill;
	type AssetId = AssetId;
	type Oracle = OracleAssetVolumeProvider<Runtime, LRNA, DynamicFeesOraclePeriod>;
	type AssetFeeParameters = AssetFeeParams;
	type ProtocolFeeParameters = ProtocolFeeParams;
}

// Stableswap
parameter_types! {
	pub StableswapAmplificationRange: RangeInclusive<NonZeroU16> = RangeInclusive::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(10_000).unwrap());
}

pub struct StableswapAccountIdConstructor<T: frame_system::Config>(PhantomData<T>);

impl<T: frame_system::Config> AccountIdFor<AssetId> for StableswapAccountIdConstructor<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	type AccountId = T::AccountId;

	fn from_assets(asset: &AssetId, identifier: Option<&[u8]>) -> Self::AccountId {
		let name = Self::name(asset, identifier);
		T::AccountId::unchecked_from(<T::Hashing as frame_support::sp_runtime::traits::Hash>::hash(&name[..]))
	}

	fn name(asset: &u32, identifier: Option<&[u8]>) -> Vec<u8> {
		let mut buf = identifier.map_or_else(Vec::new, |v| v.to_vec());
		buf.extend_from_slice(&(asset).to_le_bytes());
		buf
	}
}

use pallet_currencies::fungibles::FungibleCurrencies;

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::price::OraclePriceProviderUsingRoute;

#[cfg(feature = "runtime-benchmarks")]
use frame_support::storage::with_transaction;
use hydradx_traits::fee::{InspectTransactionFeeCurrency, SwappablePaymentAssetTrader};
#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::price::PriceProvider;
#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::registry::Create;
use hydradx_traits::router::RefundEdCalculator;
use pallet_referrals::traits::Convert;
use pallet_referrals::{FeeDistribution, Level};
#[cfg(feature = "runtime-benchmarks")]
use pallet_stableswap::BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
use sp_runtime::TransactionOutcome;

#[cfg(feature = "runtime-benchmarks")]
pub struct RegisterAsset<T>(PhantomData<T>);

#[cfg(feature = "runtime-benchmarks")]
impl<T: pallet_asset_registry::Config> BenchmarkHelper<AssetId> for RegisterAsset<T> {
	fn register_asset(asset_id: AssetId, decimals: u8) -> DispatchResult {
		let asset_name: BoundedVec<u8, RegistryStrLimit> = asset_id
			.to_le_bytes()
			.to_vec()
			.try_into()
			.map_err(|_| "BoundedConversionFailed")?;

		with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
				Some(asset_id),
				Some(asset_name.clone()),
				AssetKind::Token,
				1,
				Some(asset_name),
				Some(decimals),
				None,
				None,
			))
		})?;

		Ok(())
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: pallet_ema_oracle::Config> pallet_ema_oracle::BenchmarkHelper<AssetId> for RegisterAsset<T> {
	fn register_asset(asset_id: AssetId) -> DispatchResult {
		let result = with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
				Some(asset_id),
				None,
				AssetKind::Token,
				1,
				None,
				Some(12),
				None,
				None,
			))
		});

		// don't throw error if the asset is already registered
		if result.is_err_and(|e| e == pallet_asset_registry::Error::<Runtime>::AssetAlreadyRegistered.into()) {
			return Ok(());
		};

		let _ = result?;
		Ok(())
	}
}

impl pallet_stableswap::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type AssetId = AssetId;
	type Currency = Currencies;
	type ShareAccountId = StableswapAccountIdConstructor<Runtime>;
	type AssetInspection = AssetRegistry;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type DustAccountHandler = Duster;
	type Hooks = StableswapHooksAdapter<Runtime>;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MinTradingLimit = MinTradingLimit;
	type AmplificationRange = StableswapAmplificationRange;
	type WeightInfo = weights::pallet_stableswap::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = RegisterAsset<Runtime>;
}

// Bonds
parameter_types! {
	pub ProtocolFee: Permill = Permill::from_percent(2);
	pub const BondsPalletId: PalletId = PalletId(*b"pltbonds");
}

pub struct AssetTypeWhitelist;
impl Contains<AssetKind> for AssetTypeWhitelist {
	fn contains(t: &AssetKind) -> bool {
		matches!(t, AssetKind::Token | AssetKind::XYK | AssetKind::StableSwap)
	}
}

impl pallet_bonds::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Currency = Currencies;
	type AssetRegistry = AssetRegistry;
	type ExistentialDeposits = AssetRegistry;
	type TimestampProvider = Timestamp;
	type PalletId = BondsPalletId;
	type IssueOrigin = EnsureSigned<AccountId>;
	type AssetTypeWhitelist = AssetTypeWhitelist;
	type ProtocolFee = ProtocolFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = weights::pallet_bonds::HydraWeight<Runtime>;
}

// Staking
parameter_types! {
	pub const StakingPalletId: PalletId = PalletId(*b"staking#");
	pub const MinStake: Balance = 1_000 * UNITS;
	pub const PeriodLength: BlockNumber = DAYS;
	pub const TimePointsW:Permill =  Permill::from_percent(100);
	pub const ActionPointsW: Perbill = Perbill::from_percent(20);
	pub const TimePointsPerPeriod: u8 = 1;
	pub const CurrentStakeWeight: u8 = 2;
	pub const UnclaimablePeriods: BlockNumber = 1;
	pub const PointPercentage: FixedU128 = FixedU128::from_rational(2,100);
}

pub struct PointsPerAction;

impl GetByKey<Action, u32> for PointsPerAction {
	fn get(k: &Action) -> u32 {
		match k {
			Action::DemocracyVote => 100_u32,
		}
	}
}

pub struct StakingMinSlash;

impl GetByKey<FixedU128, Point> for StakingMinSlash {
	fn get(_k: &FixedU128) -> Point {
		50_u128
	}
}

impl pallet_staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = MajorityOfCouncil;
	type AssetId = AssetId;
	type Currency = Currencies;
	type PeriodLength = PeriodLength;
	type PalletId = StakingPalletId;
	type NativeAssetId = NativeAssetId;
	type MinStake = MinStake;
	type TimePointsWeight = TimePointsW;
	type ActionPointsWeight = ActionPointsW;
	type TimePointsPerPeriod = TimePointsPerPeriod;
	type UnclaimablePeriods = UnclaimablePeriods;
	type CurrentStakeWeight = CurrentStakeWeight;
	type PayablePercentage = SigmoidPercentage<PointPercentage, ConstU32<2_000>>;
	type BlockNumberProvider = System;
	type PositionItemId = u128;
	type CollectionId = u128;
	type NFTCollectionId = ConstU128<2222>;
	type Collections = FreezableNFT<Runtime, Self::RuntimeOrigin>;
	type NFTHandler = Uniques;
	type MaxVotes = MaxVotes;
	type ReferendumInfo = pallet_staking::integrations::democracy::ReferendumStatus<Runtime>;
	type MaxPointsPerAction = PointsPerAction;
	type Vesting = VestingInfo<Runtime>;
	type WeightInfo = weights::pallet_staking::HydraWeight<Runtime>;
	type MinSlash = StakingMinSlash;

	#[cfg(feature = "runtime-benchmarks")]
	type MaxLocks = MaxLocks;
}

// LBP
pub struct AssetPairAccountId<T: frame_system::Config>(PhantomData<T>);
impl<T: frame_system::Config> AssetPairAccountIdFor<AssetId, T::AccountId> for AssetPairAccountId<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	fn from_assets(asset_a: AssetId, asset_b: AssetId, identifier: &str) -> T::AccountId {
		let mut buf: Vec<u8> = identifier.as_bytes().to_vec();

		if asset_a < asset_b {
			buf.extend_from_slice(&asset_a.to_le_bytes());
			buf.extend_from_slice(&asset_b.to_le_bytes());
		} else {
			buf.extend_from_slice(&asset_b.to_le_bytes());
			buf.extend_from_slice(&asset_a.to_le_bytes());
		}
		T::AccountId::unchecked_from(<T::Hashing as frame_support::sp_runtime::traits::Hash>::hash(&buf[..]))
	}
}

impl pallet_lbp::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Currencies;
	type LockedBalance = MultiCurrencyLockedBalance<Runtime, NativeAssetId>;
	type CreatePoolOrigin = SuperMajorityTechCommittee;
	type LBPWeightFunction = pallet_lbp::LBPWeightFunction;
	type AssetPairAccountId = AssetPairAccountId<Self>;
	type WeightInfo = weights::pallet_lbp::HydraWeight<Runtime>;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
}

parameter_types! {
	pub XYKExchangeFee: (u32, u32) = (3, 1_000);
	pub const DiscountedFee: (u32, u32) = (7, 10_000);
	pub const XYKOracleSourceIdentifier: Source = XYK_SOURCE;
}

impl pallet_xyk::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetRegistry = AssetRegistry;
	type AssetPairAccountId = AssetPairAccountId<Self>;
	type Currency = Currencies;
	type NativeAssetId = NativeAssetId;
	type WeightInfo = weights::pallet_xyk::HydraWeight<Runtime>;
	type GetExchangeFee = XYKExchangeFee;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CanCreatePool = hydradx_adapters::xyk::AllowPoolCreation<Runtime, AssetRegistry>;
	type AMMHandler = pallet_ema_oracle::OnActivityHandler<Runtime>;
	type DiscountedFee = DiscountedFee;
	type NonDustableWhitelistHandler = Duster;
	type OracleSource = XYKOracleSourceIdentifier;
}

parameter_types! {
	pub const ReferralsPalletId: PalletId = PalletId(*b"referral");
	pub RegistrationFee: (AssetId,Balance, AccountId)= (NativeAssetId::get(), 222_000_000_000_000, TreasuryAccount::get());
	pub const MaxCodeLength: u32 = 10;
	pub const MinCodeLength: u32 = 4;
	pub const ReferralsOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
	pub const ReferralsSeedAmount: Balance = 10_000_000_000_000;
	pub ReferralsExternalRewardAccount: Option<AccountId> = Some(StakingPalletId::get().into_account_truncating());
}

impl pallet_referrals::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type AssetId = AssetId;
	type Currency = FungibleCurrencies<Runtime>;
	type Convert = ConvertViaOmnipool<Omnipool>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type PriceProvider =
		OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNA>, ReferralsOraclePeriod>;
	#[cfg(feature = "runtime-benchmarks")]
	type PriceProvider = ReferralsDummyPriceProvider;
	type RewardAsset = NativeAssetId;
	type PalletId = ReferralsPalletId;
	type RegistrationFee = RegistrationFee;
	type CodeLength = MaxCodeLength;
	type MinCodeLength = MinCodeLength;
	type LevelVolumeAndRewardPercentages = ReferralsLevelVolumeAndRewards;
	type ExternalAccount = ReferralsExternalRewardAccount;
	type SeedNativeAmount = ReferralsSeedAmount;
	type WeightInfo = weights::pallet_referrals::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ReferralsBenchmarkHelper;
}

impl pallet_amm_support::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

pub struct ConvertViaOmnipool<SP>(PhantomData<SP>);
impl<SP> Convert<AccountId, AssetId, Balance> for ConvertViaOmnipool<SP>
where
	SP: SpotPriceProvider<AssetId, Price = FixedU128>,
{
	type Error = DispatchError;

	fn convert(
		who: AccountId,
		asset_from: AssetId,
		asset_to: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error> {
		if amount < <Runtime as pallet_omnipool::Config>::MinimumTradingLimit::get() {
			return Err(pallet_referrals::Error::<Runtime>::ConversionMinTradingAmountNotReached.into());
		}
		let price = SP::spot_price(asset_to, asset_from).ok_or(pallet_referrals::Error::<Runtime>::PriceNotFound)?;
		let amount_to_receive = price.saturating_mul_int(amount);
		let min_expected = amount_to_receive
			.saturating_sub(Permill::from_percent(1).mul_floor(amount_to_receive))
			.max(1);
		let balance = Currencies::free_balance(asset_to, &who);
		let r = Omnipool::sell(
			RuntimeOrigin::signed(who.clone()),
			asset_from,
			asset_to,
			amount,
			min_expected,
		);
		if let Err(error) = r {
			if error == pallet_omnipool::Error::<Runtime>::ZeroAmountOut.into() {
				return Err(pallet_referrals::Error::<Runtime>::ConversionZeroAmountReceived.into());
			}
			return Err(error);
		}
		let balance_after = Currencies::free_balance(asset_to, &who);
		let received = balance_after.saturating_sub(balance);
		Ok(received)
	}
}

pub struct ReferralsLevelVolumeAndRewards;

impl GetByKey<Level, (Balance, FeeDistribution)> for ReferralsLevelVolumeAndRewards {
	fn get(k: &Level) -> (Balance, FeeDistribution) {
		let volume = match k {
			Level::Tier0 | Level::None => 0,
			Level::Tier1 => 305 * UNITS,
			Level::Tier2 => 4_583 * UNITS,
			Level::Tier3 => 61_111 * UNITS,
			Level::Tier4 => 763_888 * UNITS,
		};
		let rewards = match k {
			Level::None => FeeDistribution {
				referrer: Permill::zero(),
				trader: Permill::zero(),
				external: Permill::from_percent(50),
			},
			Level::Tier0 => FeeDistribution {
				referrer: Permill::from_percent(5),
				trader: Permill::from_percent(10),
				external: Permill::from_percent(35),
			},
			Level::Tier1 => FeeDistribution {
				referrer: Permill::from_percent(10),
				trader: Permill::from_percent(11),
				external: Permill::from_percent(29),
			},
			Level::Tier2 => FeeDistribution {
				referrer: Permill::from_percent(15),
				trader: Permill::from_percent(12),
				external: Permill::from_percent(23),
			},
			Level::Tier3 => FeeDistribution {
				referrer: Permill::from_percent(20),
				trader: Permill::from_percent(13),
				external: Permill::from_percent(17),
			},
			Level::Tier4 => FeeDistribution {
				referrer: Permill::from_percent(25),
				trader: Permill::from_percent(15),
				external: Permill::from_percent(10),
			},
		};
		(volume, rewards)
	}
}

#[cfg(feature = "runtime-benchmarks")]
use pallet_referrals::BenchmarkHelper as RefBenchmarkHelper;
use pallet_xyk::types::AssetPair;
use primitives::constants::chain::CORE_ASSET_ID;

#[cfg(feature = "runtime-benchmarks")]
pub struct ReferralsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl RefBenchmarkHelper<AssetId, Balance> for ReferralsBenchmarkHelper {
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance) {
		let asset_id: u32 = 1234u32;
		let asset_name: BoundedVec<u8, RegistryStrLimit> = asset_id.to_le_bytes().to_vec().try_into().unwrap();

		with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_asset(
				Some(asset_id),
				Some(asset_name.clone()),
				AssetKind::Token,
				Some(1_000_000),
				Some(asset_name),
				Some(18),
				None,
				None,
				true,
			))
		})
		.unwrap();

		let native_price = FixedU128::from_inner(1201500000000000);
		let asset_price = FixedU128::from_inner(45_000_000_000);

		Currencies::update_balance(
			RuntimeOrigin::root(),
			Omnipool::protocol_account(),
			NativeAssetId::get(),
			1_000_000_000_000_000_000,
		)
		.unwrap();

		Currencies::update_balance(
			RuntimeOrigin::root(),
			Omnipool::protocol_account(),
			asset_id,
			1_000_000_000_000_000_000_000_000,
		)
		.unwrap();

		Omnipool::add_token(
			RuntimeOrigin::root(),
			NativeAssetId::get(),
			native_price,
			Permill::from_percent(10),
			TreasuryAccount::get(),
		)
		.unwrap();

		Omnipool::add_token(
			RuntimeOrigin::root(),
			asset_id,
			asset_price,
			Permill::from_percent(10),
			TreasuryAccount::get(),
		)
		.unwrap();
		(1234, 1_000_000_000_000_000_000)
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ReferralsDummyPriceProvider;

#[cfg(feature = "runtime-benchmarks")]
impl PriceProvider<AssetId> for ReferralsDummyPriceProvider {
	type Price = EmaPrice;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(EmaPrice::one());
		}
		Some(EmaPrice::new(1_000_000_000_000, 2_000_000_000_000_000_000))
	}
}

pub struct XykPaymentAssetSupport;

impl InspectTransactionFeeCurrency<AssetId> for XykPaymentAssetSupport {
	fn is_transaction_fee_currency(asset: AssetId) -> bool {
		asset == CORE_ASSET_ID || MultiTransactionPayment::contains(&asset)
	}
}

impl SwappablePaymentAssetTrader<AccountId, AssetId, Balance> for XykPaymentAssetSupport {
	fn is_trade_supported(from: AssetId, into: AssetId) -> bool {
		XYK::exists(pallet_xyk::types::AssetPair::new(from, into))
	}

	fn calculate_fee_amount(swap_amount: Balance) -> Result<Balance, DispatchError> {
		let xyk_exchange_rate = XYKExchangeFee::get();

		hydra_dx_math::fee::calculate_pool_trade_fee(swap_amount, xyk_exchange_rate)
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn calculate_in_given_out(
		insuff_asset_id: AssetId,
		asset_out: AssetId,
		asset_out_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let asset_pair_account = XYK::get_pair_id(AssetPair::new(insuff_asset_id, asset_out));
		let out_reserve = Currencies::free_balance(asset_out, &asset_pair_account);
		let in_reserve = Currencies::free_balance(insuff_asset_id, &asset_pair_account.clone());

		hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, asset_out_amount)
			.map_err(|_err| ArithmeticError::Overflow.into())
	}

	fn buy(
		origin: &AccountId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		max_limit: Balance,
		dest: &AccountId,
	) -> DispatchResult {
		XYK::buy_for(
			origin,
			pallet_xyk::types::AssetPair { asset_in, asset_out },
			amount,
			max_limit,
			false,
			dest,
		)
	}
}
