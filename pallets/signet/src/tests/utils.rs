use crate::{tests::MaxChainIdLength, AffinePoint, BitcoinOutput, ErrorResponse, MaxScriptLength, Signature, UtxoInput};
use sp_core::ConstU32;
use sp_runtime::BoundedVec;

pub fn bounded_u8<const N: u32>(v: Vec<u8>) -> BoundedVec<u8, ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

pub fn bounded_array<const N: u32>(v: Vec<[u8; 32]>) -> BoundedVec<[u8; 32], ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

pub fn bounded_sig<const N: u32>(v: Vec<Signature>) -> BoundedVec<Signature, ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

pub fn bounded_err<const N: u32>(v: Vec<ErrorResponse>) -> BoundedVec<ErrorResponse, ConstU32<N>> {
	BoundedVec::try_from(v).unwrap()
}

pub fn bounded_chain_id(v: Vec<u8>) -> BoundedVec<u8, MaxChainIdLength> {
	BoundedVec::try_from(v).unwrap()
}

pub fn create_test_signature() -> Signature {
	Signature {
		big_r: AffinePoint {
			x: [1u8; 32],
			y: [2u8; 32],
		},
		s: [3u8; 32],
		recovery_id: 0,
	}
}

// Bitcoin helpers
pub fn create_test_utxo_input(txid: [u8; 32], vout: u32, value: u64) -> UtxoInput {
	UtxoInput {
		txid,
		vout,
		value,
		script_pubkey: BoundedVec::<u8, MaxScriptLength>::try_from(vec![0x00, 0x14]).unwrap(),
		sequence: 0xFFFFFFFF,
	}
}

pub fn create_test_bitcoin_output(value: u64) -> BitcoinOutput {
	BitcoinOutput {
		value,
		script_pubkey: BoundedVec::<u8, MaxScriptLength>::try_from(vec![0x00, 0x14]).unwrap(),
	}
}
