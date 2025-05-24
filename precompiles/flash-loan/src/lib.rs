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
use ethabi::ethereum_types::BigEndianHash;
use evm::ExitSucceed;
use fp_evm::{ExitReason, ExitRevert, PrecompileFailure, PrecompileHandle};
use frame_support::__private::RuntimeDebug;
use frame_support::traits::ConstU32;
use frame_support::traits::IsType;
use hydradx_traits::evm::{CallContext, InspectEvmAccounts, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use precompile_utils::evm::writer::EvmDataReader;
use precompile_utils::prelude::*;
use sp_core::crypto::AccountId32;
use sp_core::{H256, U256};
use sp_std::vec;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub const CALL_DATA_LIMIT: u32 = 2u32.pow(16);

pub const SUCCESS: [u8; 32] = keccak256!("ERC3156FlashBorrower.onFlashLoan");

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	Approve = "approve(address,uint256)",
}

pub struct FlashLoanReceiverPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> FlashLoanReceiverPrecompile<Runtime>
where
	Runtime: pallet_evm::Config + pallet_stableswap::Config + pallet_hsm::Config,
	<Runtime as frame_system::pallet::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	<Runtime as pallet_stableswap::pallet::Config>::AssetId: From<u32>,
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
		// Initiator is the one who called the flash loan
		// Caller of this callback is the flash minter contract.
		// "this" is the address that contains the flash loan amount.
		let caller = handle.context().caller;
		let this = handle.context().address;
		log::trace!(target: "flash", "this: {:?}", this);
		log::trace!(target: "flash", "caller: {:?}", caller);
		log::trace!(target: "flash", "initiator: {:?}", initiator);
		log::trace!(target: "flash", "amt: {:?}", amount);
		log::trace!(target: "flash", "fee: {:?}", fee);

		let mut reader = EvmDataReader::new(&data.as_bytes());
		let data_ident: u8 = reader.read()?;
		log::trace!(target: "flash", "data_ident: {:?}", data_ident);

		match data_ident {
			0 => {
				// We only allow the HSM account to use the flash loan for arbitrage opportunities.
				let hsm_account = pallet_hsm::Pallet::<Runtime>::account_id();
				let allowed_initiator = <Runtime as pallet_hsm::Config>::EvmAccounts::evm_address(&hsm_account);
				if initiator.0 != allowed_initiator {
					log::error!(target: "flash", "Caller is not the HSM account: {:?}", caller);
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: vec![],
					});
				}
				// Get the arb data
				// The first byte is the data identifier, the next bytes are the collateral asset id and pool id.
				let collateral_asset_id: u32 = reader.read()?;
				let pool_id: u32 = reader.read()?;

				log::trace!(target: "flash", "collateral_asset_id: {:?}", collateral_asset_id);
				log::trace!(target: "flash", "pool_id: {:?}", pool_id);
				let r = pallet_hsm::Pallet::<Runtime>::execute_arbitrage_with_flash_loan(
					this,
					pool_id.into(),
					collateral_asset_id.into(),
					amount.as_u128(),
				);
				if r.is_err() {
					log::error!(target: "flash", "execute_arbitrage_with_flash_loan failed: {:?}", r);
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: vec![],
					});
				}

				//TODO: remove fee mint - this is a workaround for now because we need to add the caller to list of borrowers first, so fee is 0
				let r = pallet_hsm::Pallet::<Runtime>::mint_hollar_to_evm(&this, fee.as_u128());

				// Approve the transfer of the loan
				let cc = CallContext::new_call(token.0, this);
				let mut data = Into::<u32>::into(Function::Approve).to_be_bytes().to_vec();
				data.extend_from_slice(H256::from(caller).as_bytes());
				data.extend_from_slice(H256::from_uint(&(amount + fee)).as_bytes());

				let (exit_reason, v) = <Runtime as pallet_hsm::Config>::Evm::call(cc, data, U256::zero(), 100_000);
				if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
					log::error!(target: "flash", "approve failed: {:?}, value {:?}", r, v);
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: vec![],
					});
				}

				Ok(SUCCESS.into())
			}
			_ => {
				log::error!(target: "flash", "data_ident {} not supported", data_ident);
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: vec![],
				})
			}
		}
	}
}
