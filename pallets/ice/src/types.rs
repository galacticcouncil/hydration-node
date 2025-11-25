use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use pallet_intent::types::Intent;
use sp_std::vec::Vec;

#[derive(Debug, Clone, PartialEq, Encode, Decode, TypeInfo)]
pub struct Solution {}

#[derive(Encode, Decode)]
pub struct SolverData {
	intents: Vec<Intent>,
}
