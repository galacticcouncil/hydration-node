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

use crate::evm::precompiles::{erc20_mapping::is_asset_address, multicurrency::MultiCurrencyPrecompile};
use codec::Decode;
use frame_support::dispatch::{GetDispatchInfo, PostDispatchInfo};
use pallet_evm::{
	ExitError, ExitRevert, ExitSucceed, IsPrecompileResult, Precompile, PrecompileFailure, PrecompileHandle,
	PrecompileOutput, PrecompileResult, PrecompileSet,
};
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_simple::{ECRecover, Identity, Ripemd160, Sha256};
use sp_runtime::traits::Dispatchable;

use codec::alloc;
use ethabi::Token;
use hex_literal::hex;
use primitive_types::{H160, U256};
use sp_std::{borrow::ToOwned, vec::Vec};

pub mod costs;
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

pub struct HydraDXPrecompiles<R>(PhantomData<R>);

impl<R> HydraDXPrecompiles<R> {
	#[allow(clippy::new_without_default)] // We'll never use Default and can't derive it.
	pub fn new() -> Self {
		Self(Default::default())
	}
}

// Same as Moonbean and Centrifuge, should benefit interoperability
// See also
// https://docs.moonbeam.network/builders/pallets-precompiles/precompiles/overview/#precompiled-contract-addresses
pub const DISPATCH_ADDR: H160 = addr(1025);

pub const ECRECOVER: H160 = H160(hex!("0000000000000000000000000000000000000001"));
pub const SHA256: H160 = H160(hex!("0000000000000000000000000000000000000002"));
pub const RIPEMD: H160 = H160(hex!("0000000000000000000000000000000000000003"));
pub const IDENTITY: H160 = H160(hex!("0000000000000000000000000000000000000004"));
pub const MODEXP: H160 = H160(hex!("0000000000000000000000000000000000000005"));
pub const BN_ADD: H160 = H160(hex!("0000000000000000000000000000000000000006"));
pub const BN_MUL: H160 = H160(hex!("0000000000000000000000000000000000000007"));
pub const BN_PAIRING: H160 = H160(hex!("0000000000000000000000000000000000000008"));
pub const BLAKE2F: H160 = H160(hex!("0000000000000000000000000000000000000009"));
pub const CALLPERMIT: H160 = H160(hex!("000000000000000000000000000000000000080a"));

pub const ETH_PRECOMPILE_END: H160 = BLAKE2F;

pub fn is_standard_precompile(address: H160) -> bool {
	!address.is_zero() && address <= ETH_PRECOMPILE_END
}

impl<R> PrecompileSet for HydraDXPrecompiles<R>
where
	R: pallet_evm::Config + pallet_currencies::Config + pallet_evm_accounts::Config,
	R::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<R::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<R::AccountId>>,
	MultiCurrencyPrecompile<R>: Precompile,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let context = handle.context();
		let address = handle.code_address();

		// Disallow calling custom precompiles with DELEGATECALL or CALLCODE
		if context.address != address && is_precompile(address) && !is_standard_precompile(address) {
			return Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "precompile cannot be called with DELEGATECALL or CALLCODE".into(),
			}));
		}

		if address == ECRECOVER {
			Some(ECRecover::execute(handle))
		} else if address == SHA256 {
			Some(Sha256::execute(handle))
		} else if address == RIPEMD {
			Some(Ripemd160::execute(handle))
		} else if address == IDENTITY {
			Some(Identity::execute(handle))
		} else if address == MODEXP {
			Some(Modexp::execute(handle))
		} else if address == BN_ADD {
			Some(Bn128Add::execute(handle))
		} else if address == BN_MUL {
			Some(Bn128Mul::execute(handle))
		} else if address == BN_PAIRING {
			Some(Bn128Pairing::execute(handle))
		} else if address == BLAKE2F {
			Some(Blake2F::execute(handle))
		} else if address == CALLPERMIT {
			Some(pallet_evm_precompile_call_permit::CallPermitPrecompile::<R>::execute(
				handle,
			))
		} else if address == DISPATCH_ADDR {
			Some(pallet_evm_precompile_dispatch::Dispatch::<R>::execute(handle))
		} else if is_asset_address(address) {
			Some(MultiCurrencyPrecompile::<R>::execute(handle))
		} else {
			None
		}
	}

	fn is_precompile(&self, address: H160, _remaining_gas: u64) -> IsPrecompileResult {
		IsPrecompileResult::Answer {
			is_precompile: is_precompile(address),
			extra_cost: 0,
		}
	}
}

pub fn is_precompile(address: H160) -> bool {
	address == DISPATCH_ADDR || is_asset_address(address) || is_standard_precompile(address)
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
