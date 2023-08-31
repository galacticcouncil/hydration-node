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

use crate::TreasuryAccount;
pub use crate::{
	evm::accounts_conversion::{ExtendedAddressMapping, FindAuthorTruncated},
	AssetLocation, Aura,
};
use frame_support::traits::Defensive;
use frame_support::{
	parameter_types,
	traits::FindAuthor,
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
	ConsensusEngineId,
};
use hex_literal::hex;
use hydradx_traits::Registry;
use orml_tokens::CurrencyAdapter;
use pallet_evm::{AddressMapping, EnsureAddressTruncated, Error, OnChargeEVMTransaction};
use pallet_transaction_multi_payment::{DepositAll, DepositAllEvm, TransferEvmFees};
use polkadot_xcm::latest::MultiLocation;
use polkadot_xcm::prelude::{AccountKey20, Here, PalletInstance, Parachain, X3};
use primitive_types::H160;
use primitives::{AccountId, AssetId};
use sp_core::{Get, U256};
use sp_runtime::traits::Convert;
use sp_runtime::Permill;

mod accounts_conversion;
pub mod precompile;
pub mod precompiles;

// Centrifuge / Moonbeam:
// Current approximation of the gas per second consumption considering
// EVM execution over compiled WASM (on 4.4Ghz CPU).
// Given the 500ms Weight, from which 75% only are used for transactions,
// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~=
// 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;
// Approximate ratio of the amount of Weight per Gas.
const WEIGHT_PER_GAS: u64 = WEIGHT_REF_TIME_PER_SECOND / GAS_PER_SECOND;

pub struct BaseFeeThreshold;

// Increase fees if block is >50% full
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
	fn lower() -> Permill {
		Permill::zero()
	}

	fn ideal() -> Permill {
		Permill::from_parts(500_000)
	}

	fn upper() -> Permill {
		Permill::from_parts(1_000_000)
	}
}

pub const GAS_TO_WEIGHT_RATIO: u64 = 9000;

/// Convert weight to gas
pub struct WeightToGas;
impl Convert<Weight, u64> for WeightToGas {
	fn convert(weight: Weight) -> u64 {
		weight
			.ref_time()
			.checked_div(GAS_TO_WEIGHT_RATIO)
			.expect("Compile-time constant is not zero; qed;")
	}
}

parameter_types! {
	// evmTODO: set value
	pub BlockGasLimit: U256 = U256::from(u32::max_value());
	// Centrifuge uses max block weight limits within the runtime based on calculation of 0.5s for 6s block times;
	// it's interesting, check out: https://github.com/centrifuge/centrifuge-chain/blob/main/libs/primitives/src/lib.rs#L217:L228
	// pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);

	pub PrecompilesValue: precompiles::HydraDXPrecompiles<crate::Runtime> = precompiles::HydraDXPrecompiles::<_>::new();
	pub WeightPerGas: Weight = Weight::from_ref_time(WEIGHT_PER_GAS);
}

const MOONBEAM_PARA_ID: u32 = 2004;
pub const WETH_ASSET_LOCATION: AssetLocation = AssetLocation(MultiLocation {
	parents: 1,
	interior: X3(
		Parachain(MOONBEAM_PARA_ID),
		PalletInstance(110),
		AccountKey20 {
			network: None,
			key: hex!["ab3f0245b83feb11d15aaffefd7ad465a59817ed"],
		},
	),
});

pub struct WethAssetId;
impl Get<AssetId> for WethAssetId {
	fn get() -> AssetId {
		let invalid_id =
			pallet_asset_registry::Pallet::<crate::Runtime>::next_asset_id().defensive_unwrap_or(AssetId::MAX);

		match pallet_asset_registry::Pallet::<crate::Runtime>::location_to_asset(WETH_ASSET_LOCATION) {
			Some(asset_id) => asset_id,
			None => invalid_id,
		}
	}
}

type WethCurrency = CurrencyAdapter<crate::Runtime, WethAssetId>;

impl pallet_evm::Config for crate::Runtime {
	type AddressMapping = ExtendedAddressMapping;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressTruncated;
	type ChainId = crate::EVMChainId;
	type Currency = WethCurrency;
	type FeeCalculator = crate::BaseFee;
	type FindAuthor = FindAuthorTruncated<Aura>;
	type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
	type OnChargeTransaction =
		TransferEvmFees<crate::Currencies, TreasuryAccount, DepositAllEvm<crate::Runtime>, WethAssetId>;
	type OnCreate = ();
	type PrecompilesType = precompiles::HydraDXPrecompiles<Self>;
	type PrecompilesValue = PrecompilesValue;
	type Runner = pallet_evm::runner::stack::Runner<Self>;
	type RuntimeEvent = crate::RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressTruncated;
}

impl pallet_evm_chain_id::Config for crate::Runtime {}

parameter_types! {
	pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000);
	pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}

impl pallet_base_fee::Config for crate::Runtime {
	type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
	type DefaultElasticity = DefaultElasticity;
	type RuntimeEvent = crate::RuntimeEvent;
	type Threshold = BaseFeeThreshold;
}

impl pallet_ethereum::Config for crate::Runtime {
	type RuntimeEvent = crate::RuntimeEvent;
	type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
}
