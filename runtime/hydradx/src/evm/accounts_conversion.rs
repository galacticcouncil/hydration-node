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
#![allow(unused_imports)]
use crate::{
	evm::{ConsensusEngineId, FindAuthor},
	AccountId, Aura,
};
use codec::{Decode, Encode};
use core::marker::PhantomData;
use hex_literal::hex;
use pallet_evm::AddressMapping;
use sp_core::{crypto::ByteArray, H160};
use sp_runtime::traits::AccountIdConversion;

#[derive(Encode, Decode, Default)]
struct EthereumAccount(H160);

impl sp_runtime::TypeId for EthereumAccount {
	const TYPE_ID: [u8; 4] = *b"ETH\0";
}

pub struct ExtendedAddressMapping;

impl AddressMapping<AccountId> for ExtendedAddressMapping {
	fn into_account_id(address: H160) -> AccountId {
		EthereumAccount(address).into_account_truncating()
	}
}

// Ethereum-compatible blocks author (20 bytes)
// Converted by truncating from Substrate author (32 bytes)
pub struct FindAuthorTruncated<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorTruncated<F> {
	fn find_author<'a, I>(digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		if let Some(author_index) = F::find_author(digests) {
			let authority_id = Aura::authorities()[author_index as usize].clone();
			return Some(H160::from_slice(&authority_id.to_raw_vec()[4..24]));
		}
		None
	}
}

#[cfg(test)]
#[test]
fn eth_address_should_convert_to_account_id() {
	// Private key: 42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14
	// Address: 	0x222222ff7Be76052e023Ec1a306fCca8F9659D80
	// Account Id: 	45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000
	// SS58(63): 	7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb
	assert_eq!(
		ExtendedAddressMapping::into_account_id(H160::from(hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"])),
		AccountId::from(hex!["45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000"])
	);
}
