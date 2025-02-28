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
use hydradx_traits::{evm::Erc20Encoding, RegisterAssetHook};
use primitive_types::{H160, H256};
use primitives::AssetId;

pub struct HydraErc20Mapping;

// impl<T: pallet_asset_registry::Config + BoundErc20> Erc20Mapping<AssetId> for HydraErc20Mapping {
// 	fn asset_address(asset_id: AssetId) -> EvmAddress {
// 		pallet_asset_registry::Pallet::<T>::contract_address(asset_id).unwrap_or_else(|| Self::encode_evm_address(asset_id))
// 	}
// }

/// The asset id (with type u32) is encoded in the last 4 bytes of EVM address
impl Erc20Encoding<AssetId> for HydraErc20Mapping {
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

fn set_code_metadata_for_erc20(asset_id: AssetId, code: &[u8]) {
	let size = code[..].len() as u64;
	let hash = H256::from(sp_io::hashing::keccak_256(code));
	let code_metadata = pallet_evm::CodeMetadata { size, hash };
	pallet_evm::AccountCodesMetadata::<Runtime>::insert(HydraErc20Mapping::encode_evm_address(asset_id), code_metadata);
}

pub struct SetCodeForErc20Precompile;
impl RegisterAssetHook<AssetId> for SetCodeForErc20Precompile {
	fn on_register_asset(asset_id: AssetId) {
		pallet_evm::AccountCodes::<Runtime>::insert(HydraErc20Mapping::encode_evm_address(asset_id), &hex!["00"][..]);

		let code = hex!["00"];
		set_code_metadata_for_erc20(asset_id, &code);
	}
}

pub struct SetCodeMetadataForErc20Precompile;
impl frame_support::traits::OnRuntimeUpgrade for SetCodeMetadataForErc20Precompile {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		log::info!("Running migration for SetCodeMetadataForErc20Precompile.",);

		let mut reads = 0;
		let mut writes = 0;

		let code = hex!["00"];

		pallet_asset_registry::Assets::<Runtime>::iter().for_each(|(asset_id, _)| {
			reads += 1;
			if !pallet_evm::AccountCodesMetadata::<Runtime>::contains_key(HydraErc20Mapping::encode_evm_address(
				asset_id,
			)) {
				set_code_metadata_for_erc20(asset_id, &code);

				writes += 1;
			}
		});
		<Runtime as frame_system::Config>::DbWeight::get().reads_writes(reads, writes)
	}
}
