#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{convert::TryFrom, vec::Vec};

use sp_std::prelude::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	HDT = 0,
	DOT = 1,
}

impl Into<Vec<u8>> for CurrencyId {
	fn into(self) -> Vec<u8> {
		use CurrencyId::*;
		match self {
			HDT => b"HDT".to_vec(),
			DOT => b"DOT".to_vec(),
		}
	}
}

impl TryFrom<Vec<u8>> for CurrencyId {
	type Error = ();
	fn try_from(v: Vec<u8>) -> Result<CurrencyId, ()> {
		match v.as_slice() {
			b"HDT" => Ok(CurrencyId::HDT),
			b"DOT" => Ok(CurrencyId::DOT),
			_ => Err(()),
		}
	}
}
