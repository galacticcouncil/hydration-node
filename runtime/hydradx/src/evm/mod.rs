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
	AssetLocation, Aura, NORMAL_DISPATCH_RATIO,
};
use frame_support::{
	parameter_types,
	traits::{Defensive, FindAuthor, Imbalance, OnUnbalanced},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
	ConsensusEngineId,
};
use hex_literal::hex;
use orml_tokens::CurrencyAdapter;
use pallet_evm::{EnsureAddressTruncated, FeeCalculator};
use pallet_transaction_multi_payment::{DepositAll, DepositFee, TransferEvmFees};
use polkadot_xcm::{
	latest::MultiLocation,
	prelude::{AccountKey20, PalletInstance, Parachain, X3},
};
use primitives::{constants::chain::MAXIMUM_BLOCK_WEIGHT, AccountId, AssetId};
use sp_core::{Get, U256};

mod accounts_conversion;
pub mod precompiles;

// Current approximation of the gas per second consumption considering
// EVM execution over compiled WASM (on 4.4Ghz CPU).
// Given the 500ms Weight, from which 75% only are used for transactions,
// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~=
// 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;
// Approximate ratio of the amount of Weight per Gas.
const WEIGHT_PER_GAS: u64 = WEIGHT_REF_TIME_PER_SECOND / GAS_PER_SECOND;

// Fixed gas price of 0.08 gwei per gas
// pallet-base-fee to be implemented after migration to polkadot-v1.1.0
const DEFAULT_BASE_FEE_PER_GAS: u128 = 80_000_000;

parameter_types! {
	// We allow for a 75% fullness of a 0.5s block
	pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);

	pub PrecompilesValue: precompiles::HydraDXPrecompiles<crate::Runtime> = precompiles::HydraDXPrecompiles::<_>::new();
	pub WeightPerGas: Weight = Weight::from_parts(WEIGHT_PER_GAS, 0);
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
use frame_support::traits::Currency as PalletCurrency;

type NegativeImbalance = <WethCurrency as PalletCurrency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	// this is called for substrate-based transactions
	fn on_unbalanceds<B>(_: impl Iterator<Item = NegativeImbalance>) {}

	// this is called from pallet_evm for Ethereum-based transactions
	// (technically, it calls on_unbalanced, which calls this when non-zero)
	fn on_nonzero_unbalanced(amount: NegativeImbalance) {
		let _ = DepositAll::<crate::Runtime>::deposit_fee(&TreasuryAccount::get(), WethAssetId::get(), amount.peek());
	}
}

pub struct FixedGasPrice;
impl FeeCalculator for FixedGasPrice {
	fn min_gas_price() -> (U256, Weight) {
		// Return some meaningful gas price and weight
		(DEFAULT_BASE_FEE_PER_GAS.into(), Weight::from_parts(7u64, 0))
	}
}

parameter_types! {
	/// The amount of gas per pov. A ratio of 4 if we convert ref_time to gas and we compare
	/// it with the pov_size for a block. E.g.
	/// ceil(
	///     (max_extrinsic.ref_time() / max_extrinsic.proof_size()) / WEIGHT_PER_GAS
	/// )
	pub const GasLimitPovSizeRatio: u64 = 4;
	/// The amount of gas per storage (in bytes): BLOCK_GAS_LIMIT / BLOCK_STORAGE_LIMIT
	/// The current definition of BLOCK_STORAGE_LIMIT is 40 KB, resulting in a value of 366.
	pub GasLimitStorageGrowthRatio: u64 = 366;
}

impl pallet_evm::Config for crate::Runtime {
	type AddressMapping = ExtendedAddressMapping;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressTruncated;
	type ChainId = crate::EVMChainId;
	type Currency = WethCurrency;
	type FeeCalculator = FixedGasPrice;
	type FindAuthor = FindAuthorTruncated<Aura>;
	type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
	type OnChargeTransaction = TransferEvmFees<DealWithFees>;
	type OnCreate = ();
	type PrecompilesType = precompiles::HydraDXPrecompiles<Self>;
	type PrecompilesValue = PrecompilesValue;
	type Runner = pallet_evm::runner::stack::Runner<Self>;
	type RuntimeEvent = crate::RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressTruncated;
	type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
	type GasLimitStorageGrowthRatio = GasLimitStorageGrowthRatio;
	type Timestamp = crate::Timestamp;
	type WeightInfo = pallet_evm::weights::SubstrateWeight<crate::Runtime>;
}

impl pallet_evm_chain_id::Config for crate::Runtime {}

parameter_types! {
	pub PostLogContent: pallet_ethereum::PostLogContent = pallet_ethereum::PostLogContent::BlockAndTxnHashes;
}

impl pallet_ethereum::Config for crate::Runtime {
	type RuntimeEvent = crate::RuntimeEvent;
	type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
	type PostLogContent = PostLogContent;
	type ExtraDataLength = sp_core::ConstU32<1>;
}
