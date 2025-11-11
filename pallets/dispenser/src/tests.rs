use crate::{self as pallet_dispenser, *};
use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use codec::Encode;
use frame_support::{
	assert_noop, assert_ok, parameter_types,
	traits::{
		fungible::conformance_tests::regular::balanced::deposit,
		tokens::nonfungibles::{Create, Inspect, Mutate},
		Currency as CurrencyTrait, Everything, Nothing,
	},
	PalletId,
};
use frame_system as system;
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrency;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sp_core::offchain::{
	testing::{PoolState, TestOffchainExt, TestTransactionPoolExt},
	OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
};
use sp_std::sync::Arc;
use sp_std::sync::RwLock;

use sp_core::sr25519::{Public as Sr25519Public, Signature as Sr25519Signature};
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionSource};
use sp_runtime::{traits::Verify, MultiSignature};
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	AccountId32, BuildStorage,
};

extern crate alloc;

pub type NamedReserveIdentifier = [u8; 8];
pub type Amount = i128;
pub const HDX: AssetId = 0;

pub const MIN_WEI_BALANCE: u128 = 1_000_000_000_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Currencies: pallet_currencies,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Signet: pallet_signet,
		BuildEvmTx: pallet_build_evm_tx,
		Dispenser: pallet_dispenser,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;

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
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = frame_system::mocking::MockBlock<Test>;
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
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
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
	pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
	pub const MaxChainIdLength: u32 = 128;

	pub const MaxReserves: u32 = 50;

	pub const ExistentialDeposit: u128 = 1;

	pub const HDXAssetId: AssetId = HDX;

   pub const TreasuryPalletId: PalletId = PalletId(*b"py/treas");
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

parameter_types! {
	pub TreasuryAccount: AccountId32 = TreasuryPalletId::get().into_account_truncating();
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

impl frame_system::offchain::SigningTypes for Test {
	type Public = <MultiSignature as Verify>::Signer;
	type Signature = MultiSignature;
}

impl pallet_signet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = SignetPalletId;
	type MaxChainIdLength = MaxChainIdLength;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxDataLength: u32 = 1024;
}

impl pallet_build_evm_tx::Config for Test {
	type MaxDataLength = MaxDataLength;
}

parameter_types! {
	pub const DispenserPalletId: PalletId = PalletId(*b"py/erc20");
	pub const SigEthFaucetDispenserFee: u128 = 500;

	pub const SigEthFaucetMaxDispense: u128 = 1_000_000_000;

	pub const SigEthFaucetMinRequest: u128 = 100;

	pub const SigEthFaucetFeeAssetId: AssetId = 1;
	pub const SigEthFaucetFaucetAssetId: AssetId = 2;

	pub const SigEthMinFaucetThreshold: u128 = 1;

}

// MPC “root signer” (Ethereum address expected to sign Signet responses)
pub struct SigEthFaucetMpcRoot;
impl frame_support::traits::Get<[u8; 20]> for SigEthFaucetMpcRoot {
	fn get() -> [u8; 20] {
		[
			0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58, 0x5d, 0xd2, 0x99, 0xe0, 0x3d, 0x12, 0xfa, 0x42,
			0x93, 0xbc,
		]
	}
}

impl frame_system::offchain::SendTransactionTypes<RuntimeCall> for Test {
	type OverarchingCall = RuntimeCall;
	type Extrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
}

parameter_types! {
	pub const EthRpcUrl: &'static str = "https://rpc.ankr.com/eth"; // placeholder for tests
}

impl pallet_dispenser::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type VaultPalletId = DispenserPalletId;

	type Currency = FungibleCurrencies<Test>;

	type MinimumRequestAmount = SigEthFaucetMinRequest;

	type MaxDispenseAmount = SigEthFaucetMaxDispense;

	type DispenserFee = SigEthFaucetDispenserFee;

	type FeeAsset = SigEthFaucetFeeAssetId;

	type FaucetAsset = SigEthFaucetFaucetAssetId;

	type TreasuryAddress = TreasuryAccount;

	type FaucetAddress = SigEthFaucetMpcRoot;

	type MPCRootSigner = SigEthFaucetMpcRoot;

	type UpdateOrigin = frame_system::EnsureRoot<AccountId32>;

	type MinFaucetEthThreshold = SigEthMinFaucetThreshold;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let alice = &acct(1);
	let bob = &acct(2);
	let charlie = &acct(3);
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		let _ = Currencies::deposit(1, alice, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, bob, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, charlie, 1_000_000_000_000_000_000_000);

		let _ = Currencies::deposit(2, alice, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, bob, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, charlie, 1_000_000_000_000_000_000_000);
		let requester = acct(1);
		let _ = pallet_signet::Pallet::<Test>::initialize(
			RuntimeOrigin::signed(requester.clone()),
			requester,
			100,
			bounded_chain_id(b"test-chain".to_vec()),
		);
		let pallet_account = Dispenser::account_id();
		let _ = <Balances as CurrencyTrait<_>>::deposit_creating(&pallet_account, 10_000);
	});
	ext
}

#[test]
fn test_cannot_initialize_twice() {
	new_test_ext().execute_with(|| {
		let mpc_address = create_test_mpc_address();

		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		assert_noop!(
			Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE),
			Error::<Test>::AlreadyInitialized
		);

		assert_eq!(
			Dispenser::dispenser_config(),
			Some(DispenserConfigData {
				init: true,
				paused: false,
			})
		);
	});
}

#[test]
fn test_request_rejected_when_paused() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 1_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let hdx_before = Currencies::free_balance(1, &requester);
		let eth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester.clone()), receiver, amount, req_id, tx),
			Error::<Test>::Paused
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), eth_before);
	});
}

#[test]
fn test_invalid_request_id_reverts_balances() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 123_456u128;
		let tx = create_test_tx_params();

		let bad_req_id = [9u8; 32];
		let hdx_before = Currencies::free_balance(1, &requester);
		let eth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(
				RuntimeOrigin::signed(requester.clone()),
				receiver,
				amount,
				bad_req_id,
				tx
			),
			Error::<Test>::InvalidRequestId
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), eth_before);
	});
}

#[test]
fn test_fee_and_asset_routing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let amount = 10_000u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let fee = <Test as crate::Config>::DispenserFee::get();
		let treasury = <Test as crate::Config>::TreasuryAddress::get();
		let pallet_account = Dispenser::account_id();

		let hdx_req_before = Currencies::free_balance(1, &requester);
		let hdx_treas_before = Currencies::free_balance(1, &treasury);
		let weth_treas_before = Currencies::free_balance(2, &treasury);
		let eth_req_before = Currencies::free_balance(2, &requester);
		let eth_pallet_before = Currencies::free_balance(2, &pallet_account);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver,
			amount,
			req_id,
			tx
		));

		assert_eq!(Currencies::free_balance(1, &requester), hdx_req_before - fee);
		assert_eq!(Currencies::free_balance(1, &treasury), hdx_treas_before + fee);
		assert_eq!(Currencies::free_balance(2, &treasury), weth_treas_before + amount);
		assert_eq!(Currencies::free_balance(2, &requester), eth_req_before - amount);
		assert_eq!(Currencies::free_balance(2, &pallet_account), eth_pallet_before + 0);
	});
}

#[test]
fn test_pause_unpause_state() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		assert_ok!(Dispenser::pause(RuntimeOrigin::root()));
		assert_eq!(Dispenser::dispenser_config().unwrap().paused, true);

		assert_ok!(Dispenser::unpause(RuntimeOrigin::root()));
		assert_eq!(Dispenser::dispenser_config().unwrap().paused, false);
	});
}

#[test]
fn test_amount_too_small_and_too_large() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();

		let amt_small = (<Test as crate::Config>::MinimumRequestAmount::get() - 1) as u128;
		let rid_small = compute_request_id(requester.clone(), receiver, amt_small, &tx);
		assert_noop!(
			Dispenser::request_fund(
				RuntimeOrigin::signed(requester.clone()),
				receiver,
				amt_small,
				rid_small,
				tx.clone()
			),
			Error::<Test>::AmountTooSmall
		);

		let amt_big = <Test as crate::Config>::MaxDispenseAmount::get() + 1;
		let rid_big = compute_request_id(requester.clone(), receiver, amt_big, &tx);
		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester), receiver, amt_big, rid_big, tx),
			Error::<Test>::AmountTooLarge
		);
	});
}

#[test]
fn test_deposit_erc20_success() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let requester = acct(1);
		let receiver_address = create_test_receiver_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		let request_id = compute_request_id(requester.clone(), receiver_address, amount, &tx_params);
		let hdx_balance_before = Currencies::free_balance(1, &requester);
		let eth_balance_before = Currencies::free_balance(2, &requester);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester.clone()),
			receiver_address,
			amount,
			request_id,
			tx_params,
		));

		let events = System::events();
		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Dispenser(Event::FundRequested {
					request_id: rid,
					requester: req,
					to,
					amount_wei: amt,
				}) if rid == &request_id
					&& req == &requester
					&& to == &receiver_address
					&& amount == amount
			)
		}));

		assert!(events.iter().any(|e| {
			matches!(
				&e.event,
				RuntimeEvent::Signet(pallet_signet::Event::SignRespondRequested { .. })
			)
		}));

		assert_eq!(
			Currencies::free_balance(1, &requester),
			hdx_balance_before - <Test as crate::Config>::DispenserFee::get()
		);

		assert_eq!(Currencies::free_balance(2, &requester), eth_balance_before - amount);
	});
}

#[test]
fn governance_sets_faucet_balance_and_emits_event() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let old = Dispenser::current_faucet_balance_wei();
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), 42u128));
		assert_eq!(Dispenser::current_faucet_balance_wei(), 42u128);

		let ev = System::events().into_iter().any(|rec| {
			matches!(rec.event,
				RuntimeEvent::Dispenser(Event::FaucetBalanceUpdated {
					old_balance_wei, new_balance_wei
				}) if old_balance_wei == old && new_balance_wei == 42u128
			)
		});
		assert!(ev, "FaucetBalanceUpdated event not found");
	});
}

#[test]
fn non_governance_cannot_set_faucet_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let alice = acct(1);
		assert_noop!(
			Dispenser::set_faucet_balance(RuntimeOrigin::signed(alice), 7u128),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_eq!(Dispenser::current_faucet_balance_wei(), MIN_WEI_BALANCE);
	});
}

#[test]
fn request_rejected_when_balance_below_threshold() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));
		let alice = acct(1);
		let requester = acct(1);
		let receiver = create_test_receiver_address();
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), 10u128));

		let amount = 100u128;
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		let hdx_before = Currencies::free_balance(1, &requester);
		let weth_before = Currencies::free_balance(2, &requester);

		assert_noop!(
			Dispenser::request_fund(RuntimeOrigin::signed(requester.clone()), receiver, amount, req_id, tx),
			Error::<Test>::FaucetBalanceBelowThreshold
		);

		assert_eq!(Currencies::free_balance(1, &requester), hdx_before);
		assert_eq!(Currencies::free_balance(2, &requester), weth_before);
	});
}

#[test]
fn request_allowed_at_or_above_threshold() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::root(), MIN_WEI_BALANCE));

		let amount = 101u128;
		let needed = <Test as crate::Config>::MinFaucetEthThreshold::get() + amount;
		assert_ok!(Dispenser::set_faucet_balance(RuntimeOrigin::root(), needed));

		let requester = acct(1);
		let receiver = create_test_receiver_address();
		let tx = create_test_tx_params();
		let req_id = compute_request_id(requester.clone(), receiver, amount, &tx);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester),
			receiver,
			amount,
			req_id,
			tx
		));
	});
}

fn get_test_secret_key() -> SecretKey {
	SecretKey::from_slice(&[42u8; 32]).expect("Valid secret key")
}

fn bounded_u8<const N: u32>(v: Vec<u8>) -> BoundedVec<u8, ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

fn bounded_chain_id(v: Vec<u8>) -> BoundedVec<u8, MaxChainIdLength> {
	BoundedVec::try_from(v).unwrap()
}

// Get public key from secret key
fn get_test_public_key() -> PublicKey {
	let secp = Secp256k1::new();
	let secret_key = get_test_secret_key();
	PublicKey::from_secret_key(&secp, &secret_key)
}

fn public_key_to_eth_address(public_key: &PublicKey) -> [u8; 20] {
	// Get uncompressed public key (65 bytes: 0x04 + x + y)
	let uncompressed = public_key.serialize_uncompressed();
	// Skip the 0x04 prefix byte and hash the remaining 64 bytes
	let hash = keccak_256(&uncompressed[1..]);
	// Take the last 20 bytes as Ethereum address
	let mut address = [0u8; 20];
	address.copy_from_slice(&hash[12..]);
	address
}

fn create_valid_signature(message_hash: &[u8; 32]) -> pallet_signet::Signature {
	let secp = Secp256k1::new();
	let secret_key = get_test_secret_key();
	let message = Message::from_slice(message_hash).expect("Valid message hash");

	let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);
	let (recovery_id, sig_bytes) = sig.serialize_compact();

	let mut r = [0u8; 32];
	let mut s = [0u8; 32];
	r.copy_from_slice(&sig_bytes[0..32]);
	s.copy_from_slice(&sig_bytes[32..64]);

	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint { x: r, y: [0u8; 32] },
		s,
		recovery_id: recovery_id.to_i32() as u8,
	}
}

fn create_test_signature() -> pallet_signet::Signature {
	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint {
			x: [1u8; 32],
			y: [2u8; 32],
		},
		s: [3u8; 32],
		recovery_id: 0,
	}
}

fn create_test_tx_params() -> EvmTransactionParams {
	EvmTransactionParams {
		value: 0,
		gas_limit: 100_000,
		max_fee_per_gas: 30_000_000_000,
		max_priority_fee_per_gas: 1_000_000_000,
		nonce: 0,
		chain_id: 1,
	}
}

fn create_test_receiver_address() -> [u8; 20] {
	[1u8; 20]
}

fn create_test_mpc_address() -> [u8; 20] {
	let public_key = get_test_public_key();
	public_key_to_eth_address(&public_key)
}

fn compute_request_id(
	requester: AccountId32,
	to: [u8; 20],
	amount_wei: u128,
	tx_params: &EvmTransactionParams,
) -> [u8; 32] {
	use alloy_sol_types::SolValue;
	use sp_core::crypto::Ss58Codec;

	let call = crate::IGasFaucet::fundCall {
		to: Address::from_slice(&to),
		amount: U256::from(amount_wei),
	};

	let faucet_addr = <Test as crate::Config>::FaucetAddress::get();
	let rlp_encoded = pallet_build_evm_tx::Pallet::<Test>::build_evm_tx(
		frame_system::RawOrigin::Signed(requester.clone()).into(),
		Some(H160::from(faucet_addr)),
		0u128,
		call.abi_encode(),
		tx_params.nonce,
		tx_params.gas_limit,
		tx_params.max_fee_per_gas,
		tx_params.max_priority_fee_per_gas,
		vec![],
		tx_params.chain_id,
	)
	.expect("build_evm_tx should succeed");

	let pallet_account = Dispenser::account_id();
	let encoded_sender = pallet_account.encode();

	let mut account_bytes = [0u8; 32];
	let len = core::cmp::min(encoded_sender.len(), 32);
	account_bytes[..len].copy_from_slice(&encoded_sender[..len]);

	let account_id32 = sp_runtime::AccountId32::from(account_bytes);
	let sender_ss58 = account_id32.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(0));
	let path = {
		let req_scale = requester.encode();
		let mut s = String::from("0x");
		s.push_str(&hex::encode(req_scale));
		s
	};

	let packed = (
		sender_ss58.as_str(),
		rlp_encoded.as_slice(),
		60u32,
		0u32,
		path.as_str(),
		"ecdsa",
		"ethereum",
		"",
	)
		.abi_encode_packed();

	keccak_256(&packed)
}

fn acct(n: u8) -> AccountId32 {
	AccountId32::new([n; 32])
}
