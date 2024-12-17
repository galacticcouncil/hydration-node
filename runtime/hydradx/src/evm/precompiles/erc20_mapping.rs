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

use crate::evm::EvmAddress;
use crate::Runtime;
use hex_literal::hex;
use hydradx_traits::{evm::Erc20Mapping, RegisterAssetHook};
use primitive_types::H160;
use primitives::AssetId;

pub struct HydraErc20Mapping;

/// Erc20Mapping logic for HydraDX
/// The asset id (with type u32) is encoded in the last 4 bytes of EVM address
impl Erc20Mapping<AssetId> for HydraErc20Mapping {
	fn encode_evm_address(asset_id: AssetId) -> EvmAddress {
		let asset_id_bytes: [u8; 4] = asset_id.to_le_bytes();

		let mut evm_address_bytes = [0u8; 20];

		evm_address_bytes[15] = 1;

		for i in 0..4 {
			evm_address_bytes[16 + i] = asset_id_bytes[3 - i];
		}

		EvmAddress::from(evm_address_bytes)
	}

	fn decode_evm_address(evm_address: EvmAddress) -> Option<AssetId> {
		if !is_asset_address(evm_address) {
			return None;
		}

		let mut asset_id: u32 = 0;
		for byte in evm_address.as_bytes() {
			asset_id = (asset_id << 8) | (*byte as u32);
		}

		Some(asset_id)
	}
}

pub fn is_asset_address(address: H160) -> bool {
	let asset_address_prefix = &(H160::from(hex!("0000000000000000000000000000000100000000"))[0..16]);

	&address.to_fixed_bytes()[0..16] == asset_address_prefix
}

pub struct SetCodeForErc20Precompile;

impl RegisterAssetHook<AssetId> for SetCodeForErc20Precompile {
	fn on_register_asset(asset_id: AssetId) {
		pallet_evm::AccountCodes::<Runtime>::insert(HydraErc20Mapping::encode_evm_address(asset_id), &hex!["00"][..]);
	}
}

impl frame_support::traits::OnRuntimeUpgrade for SetCodeForErc20Precompile {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		let mut reads = 0;
		let mut writes = 0;
		pallet_asset_registry::Assets::<Runtime>::iter().for_each(|(asset_id, _)| {
			reads += 1;
			if !pallet_evm::AccountCodes::<Runtime>::contains_key(HydraErc20Mapping::encode_evm_address(asset_id)) {
				Self::on_register_asset(asset_id);
				writes += 1;
			}
		});
		<Runtime as frame_system::Config>::DbWeight::get().reads_writes(reads, writes)
	}
}
