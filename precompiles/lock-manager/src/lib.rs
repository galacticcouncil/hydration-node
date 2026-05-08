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

#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use frame_support::traits::Get;
use pallet_evm::GasWeightMapping;
use precompile_utils::prelude::*;
use sp_core::{H160, U256};

/// Precompile at address 0x0806.
///
/// Reports a per-account "locked GIGAHDX" amount derived from
/// `pallet_gigahdx::Stakes[who].gigahdx`. This is consumed by the
/// `LockableAToken.sol` contract's `freeBalance` check
/// (`free = balanceOf - locked`) to:
///
/// 1. Block user-initiated transfers of GIGAHDX (since `gigahdx` equals
///    the user's aToken balance, `free = 0`).
/// 2. Allow legitimate `Pool.withdraw → aToken.burn` paths during
///    `pallet-gigahdx::giga_unstake`, which pre-decrements `gigahdx` by
///    the amount being unstaked before invoking the MM.
///
/// `ExpectedToken` pins the EVM address of the GIGAHDX aToken contract
/// the precompile is willing to answer for. Calls from any other token
/// address return zero — defense against an unrelated aToken pointing
/// its `freeBalance` check at `0x0806` and accidentally over-locking
/// holders based on their gigahdx-stake state.
pub struct LockManagerPrecompile<Runtime, ExpectedToken>(PhantomData<(Runtime, ExpectedToken)>);

#[precompile_utils::precompile]
impl<Runtime, ExpectedToken> LockManagerPrecompile<Runtime, ExpectedToken>
where
	Runtime: pallet_gigahdx::Config + pallet_evm::Config,
	Runtime::AddressMapping: pallet_evm::AddressMapping<<Runtime as frame_system::Config>::AccountId>,
	ExpectedToken: Get<H160>,
{
	/// Returns the locked GIGAHDX balance for `account`. Returns zero when
	/// `token` is not the configured GIGAHDX aToken address.
	#[precompile::public("getLockedBalance(address,address)")]
	#[precompile::view]
	fn get_locked_balance(handle: &mut impl PrecompileHandle, token: Address, account: Address) -> EvmResult<U256> {
		// Charge for the `Stakes` StorageMap read via DbWeight (proof-size
		// aware) — more accurate than `record_db_read`'s byte heuristic.
		let read_weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);
		let read_gas = <Runtime as pallet_evm::Config>::GasWeightMapping::weight_to_gas(read_weight);
		handle.record_cost(read_gas)?;

		if H160::from(token) != ExpectedToken::get() {
			return Ok(U256::zero());
		}

		let account_id = <Runtime::AddressMapping as pallet_evm::AddressMapping<
			<Runtime as frame_system::Config>::AccountId,
		>>::into_account_id(account.into());
		let locked = pallet_gigahdx::Pallet::<Runtime>::locked_gigahdx(&account_id);

		Ok(U256::from(locked))
	}
}
