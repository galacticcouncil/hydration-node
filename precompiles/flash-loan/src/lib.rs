// Copyright 2019-2022 PureStake Inc.
// This file is part of Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

#![allow(clippy::all)]
#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use fp_evm::{Context, ExitReason, ExitRevert, PrecompileFailure, PrecompileHandle, Transfer};
use frame_support::{
	ensure,
	storage::types::{StorageMap, ValueQuery},
	traits::{ConstU32, Get, StorageInstance, Time},
	Blake2_128Concat,
};
use precompile_utils::evm::writer::EvmDataWriter;
use precompile_utils::{evm::costs::call_cost, prelude::*};
use sp_core::{H160, H256, U256};
use sp_io::hashing::keccak_256;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::vec;
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub const CALL_DATA_LIMIT: u32 = 2u32.pow(16);

pub const SUCCESS: [u8; 32] = keccak256!("ERC3156FlashBorrower.onFlashLoan");

pub struct FlashLoanReceiverPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> FlashLoanReceiverPrecompile<Runtime>
where
	Runtime: pallet_evm::Config,
{
	#[precompile::public("onFlashLoan(address,address,uint256,uint256,bytes)")]
	fn on_flash_loan(
		handle: &mut impl PrecompileHandle,
		initiator: Address,
		token: Address,
		amount: U256,
		fee: U256,
		data: BoundedBytes<ConstU32<CALL_DATA_LIMIT>>,
	) -> EvmResult<H256> {
		log::trace!(target: "flash", "flash loan received");
		let caller = handle.context().caller;
		let this = handle.context().address;
		log::trace!(target: "flash", "this: {:?}", this);
		log::trace!(target: "flash", "caller: {:?}", caller);
		log::trace!(target: "flash", "initiator: {:?}", initiator);
		log::trace!(target: "flash", "amt: {:?}", amount);
		log::trace!(target: "flash", "fee: {:?}", fee);
		Ok(SUCCESS.into())
	}
}
