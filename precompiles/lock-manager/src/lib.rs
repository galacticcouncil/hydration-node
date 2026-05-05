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
use precompile_utils::prelude::*;
use sp_core::U256;

/// Precompile at address 0x0806.
///
/// Reports a per-account "locked GIGAHDX" amount derived from
/// `pallet_gigahdx::Stakes[who].st_minted`. This is consumed by the
/// `LockableAToken.sol` contract's `freeBalance` check
/// (`free = balanceOf - locked`) to:
///
/// 1. Block user-initiated transfers of GIGAHDX (since `st_minted` equals
///    the user's aToken balance, `free = 0`).
/// 2. Allow legitimate `Pool.withdraw → aToken.burn` paths during
///    `pallet-gigahdx::giga_unstake`, which pre-decrements `st_minted` by
///    the amount being unstaked before invoking the MM.
pub struct LockManagerPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> LockManagerPrecompile<Runtime>
where
	Runtime: pallet_gigahdx::Config + pallet_evm::Config,
	Runtime::AddressMapping: pallet_evm::AddressMapping<<Runtime as frame_system::Config>::AccountId>,
{
	/// Returns the locked GIGAHDX balance for a given account.
	/// The `token` parameter is accepted for forward-compatibility but currently unused.
	#[precompile::public("getLockedBalance(address,address)")]
	#[precompile::view]
	fn get_locked_balance(handle: &mut impl PrecompileHandle, _token: Address, account: Address) -> EvmResult<U256> {
		// Blake2_128Concat key prefix (16) + AccountId (32) + StakeRecord (2 × u128 = 32) = 80 bytes
		handle.record_db_read::<Runtime>(80)?;

		let account_id = <Runtime::AddressMapping as pallet_evm::AddressMapping<
			<Runtime as frame_system::Config>::AccountId,
		>>::into_account_id(account.into());
		let locked = pallet_gigahdx::Stakes::<Runtime>::get(&account_id)
			.map(|s| s.st_minted)
			.unwrap_or(0);

		Ok(U256::from(locked))
	}
}
