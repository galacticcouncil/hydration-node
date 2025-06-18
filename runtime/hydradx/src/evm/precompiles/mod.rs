//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

use core::marker::PhantomData;

use crate::evm::precompiles::{chainlink_adapter::ChainlinkOraclePrecompile, multicurrency::MultiCurrencyPrecompile};
use frame_support::dispatch::{GetDispatchInfo, PostDispatchInfo};
use pallet_evm::{ExitError, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileOutput};
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_simple::{ECRecover, Identity, Ripemd160, Sha256};

use crate::evm::precompiles::dynamic::DynamicPrecompileWrapper;
use crate::evm::precompiles::erc20_mapping::is_asset_address;
use codec::{alloc, Decode};
use ethabi::Token;
use frame_support::pallet_prelude::{Get, IsType};
use hex_literal::hex;
use pallet_evm_precompile_call_permit::CallPermitPrecompile;
use pallet_evm_precompile_dispatch::Dispatch;
use pallet_evm_precompile_flash_loan::FlashLoanReceiverPrecompile;
use precompile_utils::precompile_set::{
	AcceptDelegateCall, CallableByContract, CallableByPrecompile, PrecompileAt, PrecompileSetBuilder,
	SubcallWithMaxNesting,
};
use primitive_types::{H160, U256};
use sp_core::crypto::AccountId32;
use sp_std::{borrow::ToOwned, vec::Vec};

pub mod chainlink_adapter;
pub mod costs;
pub mod dynamic;
pub mod erc20_mapping;
pub mod handle;
pub mod multicurrency;
pub mod substrate;

pub type EvmResult<T = ()> = Result<T, PrecompileFailure>;

#[cfg(test)]
mod tests;

/// The `address` type of Solidity.
/// H160 could represent 2 types of data (bytes20 and address) that are not encoded the same way.
/// To avoid issues writing H160 is thus not supported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Address(pub H160);

impl From<H160> for Address {
	fn from(a: H160) -> Address {
		Address(a)
	}
}

impl From<Address> for H160 {
	fn from(a: Address) -> H160 {
		a.0
	}
}

macro_rules! def_address_getter {
	($name:ident, $addr:expr) => {
		pub struct $name;
		impl Get<H160> for $name {
			fn get() -> H160 {
				$addr
			}
		}
	};
}

def_address_getter!(ECRecoverAddress, H160(hex!("0000000000000000000000000000000000000001")));
def_address_getter!(SHA256Address, H160(hex!("0000000000000000000000000000000000000002")));
def_address_getter!(RipemdAddress, H160(hex!("0000000000000000000000000000000000000003")));
def_address_getter!(IdentityAddress, H160(hex!("0000000000000000000000000000000000000004")));
def_address_getter!(ModexpAddress, H160(hex!("0000000000000000000000000000000000000005")));
def_address_getter!(BnAddAddress, H160(hex!("0000000000000000000000000000000000000006")));
def_address_getter!(BnMulAddress, H160(hex!("0000000000000000000000000000000000000007")));
def_address_getter!(BnPairingAddress, H160(hex!("0000000000000000000000000000000000000008")));
def_address_getter!(Blake2FAddress, H160(hex!("0000000000000000000000000000000000000009")));
def_address_getter!(
	CallPermitAddress,
	H160(hex!("000000000000000000000000000000000000080a"))
);
def_address_getter!(
	FlashLoanReceiverAddress,
	H160(hex!("000000000000000000000000000000000000090a"))
);
// Same as Moonbean and Centrifuge, should benefit interoperability
// See also
// https://docs.moonbeam.network/builders/pallets-precompiles/precompiles/overview/#precompiled-contract-addresses
def_address_getter!(DispatchAddress, addr(1025));

pub struct AllowedFlashLoanCallers;
impl Get<sp_std::vec::Vec<H160>> for AllowedFlashLoanCallers {
	fn get() -> sp_std::vec::Vec<H160> {
		let Some(flash_minter) = pallet_hsm::Pallet::<crate::Runtime>::flash_minter() else {
			log::warn!(target: "precompiles", "No flash minter configured, no flash loan precompile will be available");
			return sp_std::vec![];
		};
		sp_std::vec![flash_minter]
	}
}

type StandardPrecompilesChecks = (AcceptDelegateCall, CallableByContract, CallableByPrecompile);
type CustomPrecompilesCheck = (CallableByContract, CallableByPrecompile);

/// The main precompile set for the HydraDX runtime.
pub type HydraDXPrecompiles<R> = PrecompileSetBuilder<
	R,
	(
		// Standard Ethereum precompiles
		PrecompileAt<ECRecoverAddress, ECRecover, StandardPrecompilesChecks>,
		PrecompileAt<SHA256Address, Sha256, StandardPrecompilesChecks>,
		PrecompileAt<RipemdAddress, Ripemd160, StandardPrecompilesChecks>,
		PrecompileAt<IdentityAddress, Identity, StandardPrecompilesChecks>,
		PrecompileAt<ModexpAddress, Modexp, StandardPrecompilesChecks>,
		PrecompileAt<BnAddAddress, Bn128Add, StandardPrecompilesChecks>,
		PrecompileAt<BnMulAddress, Bn128Mul, StandardPrecompilesChecks>,
		PrecompileAt<BnPairingAddress, Bn128Pairing, StandardPrecompilesChecks>,
		PrecompileAt<Blake2FAddress, Blake2F, StandardPrecompilesChecks>,
		// HydraDX specific precompiles
		PrecompileAt<CallPermitAddress, CallPermitPrecompile<R>, CustomPrecompilesCheck>,
		PrecompileAt<
			FlashLoanReceiverAddress,
			FlashLoanReceiverPrecompile<R, AllowedFlashLoanCallers>,
			CustomPrecompilesCheck,
		>,
		//For security reasons, we dont allow dispatch to be called by contract
		//as Dispatch is mainly just for users to be able to interact with any substrate stuff
		//We also set recursion limit to 0, forbidding any recursion so we protect against reentrancy
		PrecompileAt<DispatchAddress, Dispatch<R>, (SubcallWithMaxNesting<0>,)>,
		DynamicPrecompileWrapper<MultiCurrencyPrecompile<R>>,
		DynamicPrecompileWrapper<ChainlinkOraclePrecompile<R>>,
	),
>;

pub type DispatchPrecompile<R> = PrecompileAt<DispatchAddress, Dispatch<R>, (SubcallWithMaxNesting<0>,)>;

pub fn is_precompile(address: H160) -> bool {
	address == DispatchAddress::get() || is_asset_address(address) || is_standard_precompile(address)
}

pub fn is_standard_precompile(address: H160) -> bool {
	let eth_precompile_end = Blake2FAddress::get();

	!address.is_zero() && address <= eth_precompile_end
}

// This is a reimplementation of the upstream u64->H160 conversion
// function, made `const` to make our precompile address `const`s a
// bit cleaner. It can be removed when upstream has a const conversion
// function.
pub const fn addr(a: u64) -> H160 {
	let b = a.to_be_bytes();
	H160([
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	])
}

#[must_use]
pub fn error<T: Into<alloc::borrow::Cow<'static, str>>>(text: T) -> PrecompileFailure {
	PrecompileFailure::Error {
		exit_status: ExitError::Other(text.into()),
	}
}

#[must_use]
pub fn revert(output: impl AsRef<[u8]>) -> PrecompileFailure {
	PrecompileFailure::Revert {
		exit_status: ExitRevert::Reverted,
		output: output.as_ref().to_owned(),
	}
}

#[must_use]
pub fn succeed(output: impl AsRef<[u8]>) -> PrecompileOutput {
	PrecompileOutput {
		exit_status: ExitSucceed::Returned,
		output: output.as_ref().to_owned(),
	}
}

pub struct Output;

impl Output {
	pub fn encode_bool(b: bool) -> Vec<u8> {
		ethabi::encode(&[Token::Bool(b)])
	}

	pub fn encode_uint<T>(b: T) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Uint(U256::from(b))])
	}

	pub fn encode_uint_tuple<T>(b: Vec<T>) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Tuple(b.into_iter().map(U256::from).map(Token::Uint).collect())])
	}

	pub fn encode_uint_array<T>(b: Vec<T>) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Array(b.into_iter().map(U256::from).map(Token::Uint).collect())])
	}

	pub fn encode_bytes(b: &[u8]) -> Vec<u8> {
		ethabi::encode(&[Token::Bytes(b.to_vec())])
	}

	pub fn encode_fixed_bytes(b: &[u8]) -> Vec<u8> {
		ethabi::encode(&[Token::FixedBytes(b.to_vec())])
	}

	pub fn encode_address(b: H160) -> Vec<u8> {
		ethabi::encode(&[Token::Address(b)])
	}

	pub fn encode_address_tuple(b: Vec<H160>) -> Vec<u8> {
		ethabi::encode(&[Token::Tuple(b.into_iter().map(Token::Address).collect())])
	}

	pub fn encode_address_array(b: Vec<H160>) -> Vec<u8> {
		ethabi::encode(&[Token::Array(b.into_iter().map(Token::Address).collect())])
	}
}

/// The `bytes`/`string` type of Solidity.
/// It is different from `Vec<u8>` which will be serialized with padding for each `u8` element
/// of the array, while `Bytes` is tightly packed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
	/// Interpret as `bytes`.
	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}

	/// Interpret as `string`.
	/// Can fail if the string is not valid UTF8.
	pub fn as_str(&self) -> Result<&str, sp_std::str::Utf8Error> {
		sp_std::str::from_utf8(&self.0)
	}
}

impl From<&[u8]> for Bytes {
	fn from(a: &[u8]) -> Self {
		Self(a.to_owned())
	}
}

impl From<&str> for Bytes {
	fn from(a: &str) -> Self {
		a.as_bytes().into()
	}
}

impl From<Bytes> for Vec<u8> {
	fn from(val: Bytes) -> Self {
		val.0
	}
}
