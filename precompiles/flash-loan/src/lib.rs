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
//              ^5@#.   7@#?.               Copyright (C) 2021-2025  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

#![allow(clippy::all)]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::decode_from_bytes;
use core::marker::PhantomData;
use ethabi::ethereum_types::BigEndianHash;
use evm::ExitSucceed;
use fp_evm::{ExitReason, ExitRevert, PrecompileFailure, PrecompileHandle};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::Get;
use frame_support::traits::ConstU32;
use frame_support::traits::IsType;
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use hydradx_traits::router::{Route, Trade};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use precompile_utils::evm::writer::EvmDataReader;
use precompile_utils::evm::Bytes;
use precompile_utils::prelude::*;
use sp_core::crypto::AccountId32;
use sp_core::{H256, U256};
use sp_std::vec;

pub const CALL_DATA_LIMIT: u32 = 2u32.pow(16);

pub const SUCCESS: [u8; 32] = keccak256!("ERC3156FlashBorrower.onFlashLoan");

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	Approve = "approve(address,uint256)",
}

pub struct FlashLoanReceiverPrecompile<Runtime, AllowedCallers>(PhantomData<(Runtime, AllowedCallers)>);

#[precompile_utils::precompile]
impl<Runtime, AllowedCallers> FlashLoanReceiverPrecompile<Runtime, AllowedCallers>
where
	Runtime: pallet_evm::Config + pallet_stableswap::Config + pallet_hsm::Config + pallet_liquidation::Config,
	<Runtime as frame_system::pallet::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	<Runtime as pallet_stableswap::pallet::Config>::AssetId: From<u32>,
	AllowedCallers: Get<sp_std::vec::Vec<EvmAddress>>,
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
		// Caller of this callback is usually the flash minter contract or one of the allowed callers.
		// "this" is the address that contains the flash loan amount.
		let caller = handle.context().caller;
		let this = handle.context().address;
		log::trace!(target: "flash", "this: {:?}", this);
		log::trace!(target: "flash", "caller: {:?}", caller);
		log::trace!(target: "flash", "initiator: {:?}", initiator);
		log::trace!(target: "flash", "amt: {:?}", amount);
		log::trace!(target: "flash", "fee: {:?}", fee);

		// ensure that the caller is one of the allowed callers
		let allowed_callers = AllowedCallers::get();
		if !allowed_callers.contains(&caller) {
			log::error!(target: "flash", "Caller is not allowed: {:?}", caller);
			return Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: vec![],
			});
		}

		// First byte of the data is the action identifier.
		let mut reader = EvmDataReader::new(&data.as_bytes());
		let action: u8 = reader.read()?;

		match action {
			0 => {
				// HSM arbitrage action
				// We only allow the HSM account to use the flash loan for arbitrage opportunities.
				Self::ensure_allowed_initiator(initiator.0, pallet_hsm::Pallet::<Runtime>::account_id())?;

				if let Err(r) = pallet_hsm::Pallet::<Runtime>::execute_arbitrage_with_flash_loan(
					this,
					amount.as_u128(),
					reader.read_till_end()?,
				) {
					log::error!(target: "flash", "execute_arbitrage_with_flash_loan failed: {:?}", r);
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: vec![],
					});
				}

				// Approve the loan repayment
				Self::approve(token.0, this, caller, amount + fee)?;

				Ok(SUCCESS.into())
			}
			1 => {
				// Liquidation action
				Self::ensure_allowed_initiator(initiator.0, pallet_liquidation::Pallet::<Runtime>::account_id())?;

				// Next bytes are:
				// - collateral asset id
				// - debt asset id
				// - user address
				// - route length
				// - route entry ( Trade type )
				let collateral_asset_id: u32 = reader.read()?;
				let debt_asset_id: u32 = reader.read()?;
				let user: EvmAddress = reader.read()?;
				let route_len: u32 = reader.read()?;

				let mut route = vec![];

				for _ in 0..route_len {
					let entry: Bytes = reader.read()?;
					let entry = entry.as_bytes().to_vec();
					let s = decode_from_bytes::<Trade<u32>>(entry.clone().into()).map_err(|_| {
						log::error!(target: "flash", "Failed to decode trade entry: {:?}", entry);
						PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: vec![],
						}
					})?;
					route.push(s);
				}

				Self::approve(
					token.0,
					this,
					pallet_liquidation::BorrowingContract::<Runtime>::get(),
					amount,
				)?;

				log::trace!(target: "flash", "action: {}, collateral_asset_id: {}, debt_asset_id: {}, user: {:?}, route_len: {}", action, collateral_asset_id, debt_asset_id, user, route_len);
				log::trace!(target: "flash", "route: {:?}", route);
				if let Err(r) = pallet_liquidation::Pallet::<Runtime>::liquidate_position(
					this,
					collateral_asset_id,
					debt_asset_id,
					amount.as_u128(),
					user,
					Route::truncate_from(route),
				) {
					log::error!(target: "flash", "liquidate_position failed: {:?}", r);
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: vec![],
					});
				}
				// Approve the loan repayment
				Self::approve(token.0, this, caller, amount + fee)?;
				Ok(SUCCESS.into())
			}
			_ => {
				log::error!(target: "flash", "flash loan action {} not supported", action);
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: vec![],
				})
			}
		}
	}

	fn approve(token: EvmAddress, from: EvmAddress, to: EvmAddress, amount: U256) -> Result<(), PrecompileFailure> {
		let cc = CallContext::new_call(token, from);
		let mut data = Into::<u32>::into(Function::Approve).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(to).as_bytes());
		data.extend_from_slice(H256::from_uint(&amount).as_bytes());

		let (exit_reason, v) = <Runtime as pallet_hsm::Config>::Evm::call(cc, data, U256::zero(), 100_000);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: "flash", "approve failed: {:?}, value {:?}", exit_reason, v);
			return Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: v,
			});
		}
		Ok(())
	}

	fn ensure_allowed_initiator(
		initiator: EvmAddress,
		expected: <Runtime as frame_system::Config>::AccountId,
	) -> Result<(), PrecompileFailure> {
		let allowed_initiator = <Runtime as pallet_hsm::Config>::EvmAccounts::evm_address(&expected);
		if initiator != allowed_initiator {
			log::error!(target: "flash", "Caller is not the expected initiator: {:?}", initiator);
			return Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: vec![],
			});
		}
		Ok(())
	}
}
