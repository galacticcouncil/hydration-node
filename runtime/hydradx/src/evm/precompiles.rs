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

use crate::evm::precompile::multicurrency::MultiCurrencyPrecompile;
use codec::Decode;
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use hex_literal::hex;
use pallet_evm::{Precompile, PrecompileHandle, PrecompileResult, PrecompileSet};
use sp_core::H160;

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
const DISPATCH_ADDR: H160 = addr(1025);

impl<R> PrecompileSet for HydraDXPrecompiles<R>
where
	R: pallet_evm::Config + pallet_currencies::Config,
	R::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<R::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<R::AccountId>>,
	MultiCurrencyPrecompile<R>: Precompile,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let address = handle.code_address();

		if address == DISPATCH_ADDR {
			Some(pallet_evm_precompile_dispatch::Dispatch::<R>::execute(handle))
		} else if is_asset_address(address) {
			Some(MultiCurrencyPrecompile::<R>::execute(handle))
		} else {
			None
		}
	}

	fn is_precompile(&self, address: H160) -> bool {
		address == DISPATCH_ADDR || is_asset_address(address)
	}
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

fn is_asset_address(address: H160) -> bool {
	let asset_address_prefix = &(H160::from(hex!("0000000000000000000000000000000100000000"))[0..16]);

	&address.to_fixed_bytes()[0..16] == asset_address_prefix
}
