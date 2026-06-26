// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

use core::cell::RefCell;
use evm::{ExitReason, ExitRevert, ExitSucceed};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{fungible, ConstU128, ConstU32, ConstU64, Everything, Imbalance, Nothing, OnUnbalanced},
	weights::{Weight, WeightToFee as WeightToFeeT},
};
use hydradx_traits::evm::{CallContext, CallResult, Erc20Encoding, Erc20Mapping, InspectEvmAccounts, EVM};
use orml_traits::parameter_type_with_key;
use pallet_transaction_payment::FungibleAdapter;
use primitives::{AssetId, Balance, EvmAddress};
use sp_core::{H160, H256, U256};
use sp_runtime::{
	traits::{BlakeTwo256, Convert, IdentityLookup},
	AccountId32, BuildStorage, DispatchError, SaturatedConversion,
};

use crate::{self as pallet_lazy_executor, pallet, Function};

type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type NamedReserveIdentifier = [u8; 8];

pub type LazyExecutorCall = pallet::Call<Test>;

pub const UNIT: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId32::new([3u8; 32]);
/// Funded with neither native nor multi-currency balance.
pub const ACC_ZERO_BALANCE: AccountId = AccountId32::new([9u8; 32]);

pub const HDX: AssetId = 0;
pub const DOT: AssetId = 3;

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		LazyExecutor: pallet_lazy_executor,
		TransactionPayment: pallet_transaction_payment,
	}
);

/// The receiver contract that the forward is pushed to.
pub fn contract_address() -> EvmAddress {
	H160::repeat_byte(0xAA)
}

/// Substrate account that ends up holding the pushed funds (mirrors `EvmAccountsMock::account_id`).
pub fn contract_account() -> AccountId {
	EvmAccountsMock::account_id(contract_address())
}

/// The four ack bytes the receiver must return for the push to commit.
pub fn execute_selector() -> [u8; 4] {
	Into::<u32>::into(Function::Execute).to_be_bytes()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EvmOutcome {
	SucceedCorrectAck,
	SucceedWrongAck,
	Revert,
}

thread_local! {
	static EVM_OUTCOME: RefCell<EvmOutcome> = const { RefCell::new(EvmOutcome::SucceedCorrectAck) };
}

pub fn set_evm_outcome(outcome: EvmOutcome) {
	EVM_OUTCOME.with(|v| *v.borrow_mut() = outcome);
}

pub struct EvmMock;
impl EVM<CallResult> for EvmMock {
	fn call(context: CallContext, _data: Vec<u8>, _value: U256, _gas: u64) -> CallResult {
		let (exit_reason, value) = match EVM_OUTCOME.with(|v| *v.borrow()) {
			EvmOutcome::SucceedCorrectAck => {
				let mut ack = vec![0u8; 32];
				ack[..4].copy_from_slice(&execute_selector());
				(ExitReason::Succeed(ExitSucceed::Returned), ack)
			}
			EvmOutcome::SucceedWrongAck => (ExitReason::Succeed(ExitSucceed::Returned), vec![0u8; 32]),
			EvmOutcome::Revert => (ExitReason::Revert(ExitRevert::Reverted), vec![]),
		};

		CallResult {
			exit_reason,
			value,
			contract: context.contract,
			gas_used: U256::zero(),
			gas_limit: U256::zero(),
		}
	}

	fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> CallResult {
		unimplemented!()
	}
}

pub struct EvmAccountsMock;
impl InspectEvmAccounts<AccountId> for EvmAccountsMock {
	fn is_evm_account(_account_id: AccountId) -> bool {
		false
	}

	fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress {
		EvmAddress::from_slice(&account_id.as_ref()[..20])
	}

	fn truncated_account_id(evm_address: EvmAddress) -> AccountId {
		Self::account_id(evm_address)
	}

	fn bound_account_id(_evm_address: EvmAddress) -> Option<AccountId> {
		None
	}

	fn account_id(evm_address: EvmAddress) -> AccountId {
		let mut bytes = [0u8; 32];
		bytes[..20].copy_from_slice(evm_address.as_bytes());
		AccountId32::new(bytes)
	}

	fn can_deploy_contracts(_evm_address: EvmAddress) -> bool {
		false
	}

	fn is_approved_contract(_address: EvmAddress) -> bool {
		false
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
		let mut asset_id: u32 = 0;
		for byte in evm_address.as_bytes() {
			asset_id = (asset_id << 8) | (*byte as u32);
		}
		Some(asset_id)
	}
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

pub struct EvmErrorDecodeMock;
impl Convert<CallResult, DispatchError> for EvmErrorDecodeMock {
	fn convert(_call_result: CallResult) -> DispatchError {
		DispatchError::Other("Call failed")
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
	pub const GasLimit: u64 = 1_000_000;
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
	type MaxConsumers = ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
	type ExtensionsWeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
}

impl orml_tokens::Config for Test {
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

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> hydradx_traits::CreateBare<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: Self::RuntimeCall) -> Extrinsic {
		Extrinsic::new_bare(call)
	}
}

impl pallet_lazy_executor::Config for Test {
	type RuntimeCall = RuntimeCall;
	type Currency = Tokens;
	type Evm = EvmMock;
	type EvmAccounts = EvmAccountsMock;
	type Erc20Mapping = HydraErc20Mapping;
	type GasWeightMapping = DummyGasWeightMapping;
	type GasLimit = GasLimit;
	type EvmErrorDecoder = EvmErrorDecodeMock;
	type UnsignedPriority = ConstU64<100>;
	type UnsignedLongevity = ConstU64<3>;
	type WeightInfo = ();
}

parameter_types! {
	pub static WeightToFee: u128 = 1;
	pub static TransactionByteFee: u128 = 1;
	pub static OperationalFeeMultiplier: u8 = 5;
}

impl WeightToFeeT for WeightToFee {
	type Balance = u128;

	fn weight_to_fee(weight: &Weight) -> Self::Balance {
		Self::Balance::saturated_from(weight.ref_time()).saturating_mul(WEIGHT_TO_FEE.with(|v| *v.borrow()))
	}
}

impl WeightToFeeT for TransactionByteFee {
	type Balance = u128;

	fn weight_to_fee(weight: &Weight) -> Self::Balance {
		Self::Balance::saturated_from(weight.ref_time()).saturating_mul(TRANSACTION_BYTE_FEE.with(|v| *v.borrow()))
	}
}

parameter_types! {
	pub(crate) static TipUnbalancedAmount: u128 = 0;
	pub(crate) static FeeUnbalancedAmount: u128 = 0;
}

pub struct DealWithFees;
impl OnUnbalanced<fungible::Credit<<Test as frame_system::Config>::AccountId, Balances>> for DealWithFees {
	fn on_unbalanceds(
		mut fees_then_tips: impl Iterator<Item = fungible::Credit<<Test as frame_system::Config>::AccountId, Balances>>,
	) {
		if let Some(fees) = fees_then_tips.next() {
			FeeUnbalancedAmount::mutate(|a| *a += fees.peek());
			if let Some(tips) = fees_then_tips.next() {
				TipUnbalancedAmount::mutate(|a| *a += tips.peek());
			}
		}
	}
}

impl pallet_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, DealWithFees>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = TransactionByteFee;
	type FeeMultiplierUpdate = ();
	type WeightInfo = ();
}

#[derive(Default)]
pub struct ExtBuilder {
	native_balances: Vec<(AccountId, Balance)>,
	token_balances: Vec<(AccountId, AssetId, Balance)>,
}

impl ExtBuilder {
	pub fn new() -> Self {
		Self {
			native_balances: vec![(ALICE, 200_000 * UNIT), (BOB, 150_000 * UNIT), (CHARLIE, 15_000 * UNIT)],
			token_balances: vec![],
		}
	}

	pub fn with_tokens(mut self, balances: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.token_balances = balances;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self.native_balances,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.token_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();
		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}
