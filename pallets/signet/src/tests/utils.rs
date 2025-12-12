use crate::{tests::MaxChainIdLength, AffinePoint, ErrorResponse, Signature};
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
