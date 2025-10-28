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
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	AccountId32, BuildStorage, MultiSignature,
};

extern crate alloc;

pub type NamedReserveIdentifier = [u8; 8];
pub type Amount = i128;
pub const HDX: AssetId = 0;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Signature = MultiSignature;

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
	type AccountId = u64;
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

  pub const TreasuryAccount: u64 = 99;
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

	pub const SigEthFaucetMinRequest: u64 = 0;

	pub const SigEthFaucetFeeAssetId: AssetId = 1;
	pub const SigEthFaucetFaucetAssetId: AssetId = 2;


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
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		let _ = Currencies::deposit(1, &1, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, &2, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(1, &3, 1_000_000_000_000_000_000_000);

		let _ = Currencies::deposit(2, &1, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, &2, 1_000_000_000_000_000_000_000);
		let _ = Currencies::deposit(2, &3, 1_000_000_000_000_000_000_000);

		let _ = pallet_signet::Pallet::<Test>::initialize(
			RuntimeOrigin::signed(1),
			1,
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

		assert_ok!(Dispenser::initialize(RuntimeOrigin::signed(1)));

		assert_noop!(
			Dispenser::initialize(RuntimeOrigin::signed(2)),
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
fn test_deposit_erc20_success() {
	new_test_ext().execute_with(|| {
		assert_ok!(Dispenser::initialize(RuntimeOrigin::signed(1)));

		let requester = 1u64;
		let receiver_address = create_test_receiver_address();
		let amount = 1_000_000u128;
		let tx_params = create_test_tx_params();

		let request_id = compute_request_id(requester, receiver_address, amount, &tx_params);
		let hdx_balance_before = Currencies::free_balance(1, &requester);
		let eth_balance_before = Currencies::free_balance(2, &requester);

		assert_ok!(Dispenser::request_fund(
			RuntimeOrigin::signed(requester),
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

fn compute_request_id(requester: u64, to: [u8; 20], amount_wei: u128, tx_params: &EvmTransactionParams) -> [u8; 32] {
	use alloy_sol_types::SolValue;
	use sp_core::crypto::Ss58Codec;

	let call = crate::IGasFaucet::fundCall {
		to: Address::from_slice(&to),
		amount: U256::from(amount_wei),
	};

	let faucet_addr = <Test as crate::Config>::FaucetAddress::get();
	let rlp_encoded = pallet_build_evm_tx::Pallet::<Test>::build_evm_tx(
		frame_system::RawOrigin::Signed(requester).into(),
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
