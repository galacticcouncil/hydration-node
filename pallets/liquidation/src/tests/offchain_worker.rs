// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]

pub use crate::tests::mock::*;
use ethabi::ethereum_types::{H160, H256, U256};
use ethereum::{TransactionAction, TransactionSignature};
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use pallet_ethereum::Transaction;
use polkadot_primitives::EncodeAs;
use rlp::RlpStream;
use sp_core::crypto::AccountId32;
use sp_core::hashing::keccak_256;

pub const CHAIN_ID: u64 = 222_222;
fn create_unsigned_legacy_transaction() -> LegacyUnsignedTransaction {
	LegacyUnsignedTransaction {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
			"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
		)
		.encode_as(),
	}
}

fn create_transaction(account: &AccountInfo) -> Transaction {
	LegacyUnsignedTransaction {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!("18160ddd").encode_as(),
	}
	.sign(&account.private_key)
}

pub fn create_legacy_transaction(account: &AccountInfo) -> Transaction {
	create_unsigned_legacy_transaction().sign(&account.private_key)
}

pub struct AccountInfo {
	// pub address: H160,
	// pub account_id: AccountId32,
	pub private_key: H256,
}

fn alice_keys() -> AccountInfo {
	let private_key =
		H256::from_slice(hex!("e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a").as_slice());
	let secret_key = libsecp256k1::SecretKey::parse_slice(&private_key[..]).unwrap();
	let public_key = &libsecp256k1::PublicKey::from_secret_key(&secret_key).serialize()[1..65];
	println!("- - - - public key: {:?}", hex::encode(public_key));
	let address = H160::from(H256::from(keccak_256(public_key)));
	println!("- - - - address: {:?}", address);

	AccountInfo {
		private_key,
		// account_id: <Test as pallet_evm::Config>::AddressMapping::into_account_id(address),
		// address,
	}
}

pub struct LegacyUnsignedTransaction {
	pub nonce: U256,
	pub gas_price: U256,
	pub gas_limit: U256,
	pub action: TransactionAction,
	pub value: U256,
	pub input: Vec<u8>,
}

impl LegacyUnsignedTransaction {
	fn signing_rlp_append(&self, s: &mut RlpStream) {
		s.begin_list(9);
		s.append(&self.nonce);
		s.append(&self.gas_price);
		s.append(&self.gas_limit);
		s.append(&self.action);
		s.append(&self.value);
		s.append(&self.input);
		s.append(&CHAIN_ID);
		s.append(&0u8);
		s.append(&0u8);
	}

	fn signing_hash(&self) -> H256 {
		let mut stream = RlpStream::new();
		self.signing_rlp_append(&mut stream);
		H256::from(keccak_256(&stream.out()))
	}

	pub fn sign(&self, key: &H256) -> Transaction {
		self.sign_with_chain_id(key, CHAIN_ID)
	}

	pub fn sign_with_chain_id(&self, key: &H256, chain_id: u64) -> Transaction {
		let hash = self.signing_hash();
		let msg = libsecp256k1::Message::parse(hash.as_fixed_bytes());
		let s = libsecp256k1::sign(&msg, &libsecp256k1::SecretKey::parse_slice(&key[..]).unwrap());
		let sig = s.0.serialize();

		let sig = TransactionSignature::new(
			s.1.serialize() as u64 % 2 + chain_id * 2 + 35,
			H256::from_slice(&sig[0..32]),
			H256::from_slice(&sig[32..64]),
		)
		.unwrap();

		Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: self.nonce,
			gas_price: self.gas_price,
			gas_limit: self.gas_limit,
			action: self.action,
			value: self.value,
			input: self.input.clone(),
			signature: sig,
		})
	}
}

// -----------------------------------------------------------
use ethereum::EnvelopedEncodable;
use ethereum::TransactionV2 as EthereumTransaction;
use fc_rpc::{internal_err, EthSigner};
use fc_rpc_core::types::TransactionMessage;
use jsonrpsee::types::ErrorObjectOwned;

pub struct EthDevSigner {
	keys: Vec<libsecp256k1::SecretKey>,
}

impl EthDevSigner {
	pub fn new() -> Self {
		Self {
			keys: vec![
				libsecp256k1::SecretKey::parse(&[
					0xe5, 0xbe, 0x9a, 0x50, 0x92, 0xb8, 0x1b, 0xca, 0x64, 0xbe, 0x81, 0xd2, 0x12, 0xe7, 0xf2, 0xf9,
					0xeb, 0xa1, 0x83, 0xbb, 0x7a, 0x90, 0x95, 0x4f, 0x7b, 0x76, 0x36, 0x1f, 0x6e, 0xdb, 0x5c, 0xa,
				])
				.expect("Test key is valid; qed"),
				libsecp256k1::SecretKey::parse(&[
					0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
					0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				])
				.expect("Test key is valid; qed"),
			],
		}
	}
}

pub fn secret_key_address(secret: &libsecp256k1::SecretKey) -> H160 {
	let public = libsecp256k1::PublicKey::from_secret_key(secret);
	public_key_address(&public)
}

pub fn public_key_address(public: &libsecp256k1::PublicKey) -> H160 {
	let mut res = [0u8; 64];
	res.copy_from_slice(&public.serialize()[1..65]);
	H160::from(H256::from(keccak_256(&res)))
}

impl EthSigner for EthDevSigner {
	fn accounts(&self) -> Vec<H160> {
		self.keys.iter().map(secret_key_address).collect()
	}

	fn sign(&self, message: TransactionMessage, address: &H160) -> Result<EthereumTransaction, ErrorObjectOwned> {
		let mut transaction = None;

		for secret in &self.keys {
			let key_address = secret_key_address(secret);

			if &key_address == address {
				match message {
					TransactionMessage::Legacy(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let v = match m.chain_id {
							None => 27 + recid.serialize() as u64,
							Some(chain_id) => 2 * chain_id + 35 + recid.serialize() as u64,
						};
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction = Some(EthereumTransaction::Legacy(ethereum::LegacyTransaction {
							nonce: m.nonce,
							gas_price: m.gas_price,
							gas_limit: m.gas_limit,
							action: m.action,
							value: m.value,
							input: m.input,
							signature: ethereum::TransactionSignature::new(v, r, s)
								.ok_or_else(|| internal_err("signer generated invalid signature"))?,
						}));
					}
					TransactionMessage::EIP2930(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction = Some(EthereumTransaction::EIP2930(ethereum::EIP2930Transaction {
							chain_id: m.chain_id,
							nonce: m.nonce,
							gas_price: m.gas_price,
							gas_limit: m.gas_limit,
							action: m.action,
							value: m.value,
							input: m.input.clone(),
							access_list: m.access_list,
							odd_y_parity: recid.serialize() != 0,
							r,
							s,
						}));
					}
					TransactionMessage::EIP1559(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction = Some(EthereumTransaction::EIP1559(ethereum::EIP1559Transaction {
							chain_id: m.chain_id,
							nonce: m.nonce,
							max_priority_fee_per_gas: m.max_priority_fee_per_gas,
							max_fee_per_gas: m.max_fee_per_gas,
							gas_limit: m.gas_limit,
							action: m.action,
							value: m.value,
							input: m.input.clone(),
							access_list: m.access_list,
							odd_y_parity: recid.serialize() != 0,
							r,
							s,
						}));
					}
				}
				break;
			}
		}

		transaction.ok_or_else(|| internal_err("signer not available"))
	}
}

pub const NONCE: u32 = 0;
#[test]
fn eth_tx() {
	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: ethereum::TransactionAction::Call(H160::from_slice(
			hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
			"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
		)
		.encode_as(),
		chain_id: Some(CHAIN_ID),
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	println!("- - - - EVM address: {:?}", addr);
	let tx = signer
		.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr)
		.unwrap();
	println!("\n- - - - {:?}", tx);
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));
}

#[test]
fn eth_tx_second() {
	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE),

		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!("18160ddd").encode_as(),
		chain_id: Some(CHAIN_ID),
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	let tx = signer
		.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr)
		.unwrap();
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));

	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE + 1),

		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!("18160ddd").encode_as(),
		chain_id: Some(CHAIN_ID),
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	let tx = signer
		.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr)
		.unwrap();
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));
}

// EVM account 		7KATdGb5uUXrET6mzKwHK9U3BhTZ9tQQMthCCqr4enLwWsVE

// preimage 		0x350284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01e0ad1fc73521aa1b76e0a7ada0ee4c0590ac45970b1e7687018474a2c0b4b12dca039be6d59a0f906b6a691efc11682bf83c0b0f67a4f91c17b41a16f7d6218455000000000f008c67025da919ee5f9f0c3ef934f421ad4a05258dd53e1fdd1d6f6d5630c663188835a000
// propose external	0x490284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01b636d232c6a01d4f704235ef4e4db2224c5710fb2cc9060a13fbdc7eb974046c667790053bcc23d934e40d7924cf3ac17a226ff716212b767ec407aacbb74682b501040000170204130502979c60952137c5cfb58ec643b70928c8f0cc34a268bee06384155b724fd9b85523000000a4
// fast track		0x550284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01baa6a39e4d2cde4280b62abecb673f9dfbfb3ffe28d9ecc02011fe5108ef473fad1a3c64396e298e9a643fec891abaf1f3106f06c31596bd0c38969b9750bb8415020800001902041307979c60952137c5cfb58ec643b70928c8f0cc34a268bee06384155b724fd9b8550500000002000000ac
// vote				0xf50184d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d015a363a63a65943e280f79db3ec69775334bf455518449fc3b42d85a66410dc4f11625c5b965f1f3051e605265fb6c18ab2fbac5dd3155d9223b827f4c7e8058855020c00001302310300800070f4986991e00d0000000000000000
// enact authorized

// buy Alice WETH 	0x450284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01b2dc493c49f474b7d9e21e02e3480ae16ff75152a07829a282add65ae5d41e1c855581223ca0b43248c4742eaae26123fd78fcd251fc688f92d55f7e632f358a25000000003b0614000000000000000000f4448291634500000000000000000000f444829163450000000000000000
// send WETH 		0x590284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d010a1f51f3240e1c412004ddf54394ffd288ec5dc95a10dfd01275f3214c01ae1165c17c5dce07d198d68e93eb4a1bacce84fcdf9b9e873bb83fbb9add5e287a88a5000400004d00455448008097c3c354652cb1eeed3e5b65fba2576470678a000000000000000014000000130000c84e676dc11b
// transact 		0xf86b80840310db02830c4f54940000000000000000000000000000000100000000808418160ddd8306c83fa09d9346fa1a83c414dbf77c7dda1e159ac7e6d6931483f2efdcdef9df75fe4015a035b02e2627a861cac8df102e97008b1611dd2d6f36296eb6bdec12f603af3bf6

// curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "eth_sendRawTransaction", "params": ["0xf86b80840310db02830c4f54940000000000000000000000000000000100000000808418160ddd8306c83fa09d9346fa1a83c414dbf77c7dda1e159ac7e6d6931483f2efdcdef9df75fe4015a035b02e2627a861cac8df102e97008b1611dd2d6f36296eb6bdec12f603af3bf6"]}' http://localhost:9988 && curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_submitExtrinsic", "params": ["0x590284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01ec4320708109c661d46654241a6e72fe8a06a32f8becfd6a25059d90d828781e2ab05f4c7f2acb3afcf7fa2f74c035a3325b414a74fde9f396f350a2bca7ff8535011c00004f008eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a480000000013000064a7b3b6e00d"]}' http://localhost:9988
