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

use crate::evm::evm_fee::FeeCurrencyOverrideOrDefault;
use crate::evm::runner::WrapRunner;
use crate::types::ShortOraclePrice;
pub use crate::{
	evm::accounts_conversion::{ExtendedAddressMapping, FindAuthorTruncated},
	AssetLocation, Aura, NORMAL_DISPATCH_RATIO,
};
use crate::{NativeAssetId, LRNA};
pub use fp_evm::GenesisAccount as EvmGenesisAccount;
use frame_support::{
	parameter_types,
	traits::{Defensive, FindAuthor},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
	ConsensusEngineId,
};
use hex_literal::hex;
use hydradx_adapters::price::ConvertAmount;
use hydradx_adapters::{AssetFeeOraclePriceProvider, OraclePriceProvider};
use hydradx_traits::oracle::OraclePeriod;
use orml_tokens::CurrencyAdapter;
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_evm::EnsureAddressTruncated;
use pallet_transaction_payment::Multiplier;
use primitives::{constants::chain::MAXIMUM_BLOCK_WEIGHT, AssetId};
use sp_core::{Get, U256};

mod accounts_conversion;
mod evm_fee;
pub mod permit;
pub mod precompiles;
mod runner;

// Current approximation of the gas per second consumption considering
// EVM execution over compiled WASM (on 4.4Ghz CPU).
// Given the 500ms Weight, from which 75% only are used for transactions,
// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~=
// 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;
// Approximate ratio of the amount of Weight per Gas.
const WEIGHT_PER_GAS: u64 = WEIGHT_REF_TIME_PER_SECOND / GAS_PER_SECOND;

// Fixed gas price of 0.016 gwei per gas
pub const DEFAULT_BASE_FEE_PER_GAS: u128 = 16_000_000;

parameter_types! {
	// We allow for a 75% fullness of a 0.5s block
	pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);

	pub PrecompilesValue: precompiles::HydraDXPrecompiles<crate::Runtime> = precompiles::HydraDXPrecompiles::<_>::new();
	pub WeightPerGas: Weight = Weight::from_parts(WEIGHT_PER_GAS, 0);
}

const MOONBEAM_PARA_ID: u32 = 2004;
pub const WETH_ASSET_LOCATION: AssetLocation = AssetLocation(polkadot_xcm::v3::MultiLocation {
	parents: 1,
	interior: polkadot_xcm::v3::prelude::X3(
		polkadot_xcm::v3::prelude::Parachain(MOONBEAM_PARA_ID),
		polkadot_xcm::v3::prelude::PalletInstance(110),
		polkadot_xcm::v3::prelude::AccountKey20 {
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

parameter_types! {
	pub PostLogContent: pallet_ethereum::PostLogContent = pallet_ethereum::PostLogContent::BlockAndTxnHashes;
}

pub struct TransactionPaymentMultiplier;

impl Get<Multiplier> for TransactionPaymentMultiplier {
	fn get() -> Multiplier {
		crate::TransactionPayment::next_fee_multiplier()
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

	pub const OracleEvmPeriod: OraclePeriod = OraclePeriod::Short;

	pub const SuicideQuickClearLimit: u32 = 0;
}

impl pallet_evm::Config for crate::Runtime {
	type AddressMapping = ExtendedAddressMapping;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressTruncated;
	type ChainId = crate::EVMChainId;
	type Currency = WethCurrency;
	type FeeCalculator = crate::DynamicEvmFee;
	type FindAuthor = FindAuthorTruncated<Aura>;
	type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
	type OnChargeTransaction = evm_fee::TransferEvmFees<
		evm_fee::DepositEvmFeeToTreasury,
		FeeCurrencyOverrideOrDefault<WethAssetId>, // Get account's fee payment asset
		WethAssetId,
		ConvertAmount<ShortOraclePrice>,
		FungibleCurrencies<crate::Runtime>, // Multi currency support
	>;
	type OnCreate = ();
	type PrecompilesType = precompiles::HydraDXPrecompiles<Self>;
	type PrecompilesValue = PrecompilesValue;
	type Runner = WrapRunner<
		Self,
		pallet_evm::runner::stack::Runner<Self>, // Evm runner that we wrap
		hydradx_adapters::price::FeeAssetBalanceInCurrency<
			crate::Runtime,
			ConvertAmount<ShortOraclePrice>,
			FeeCurrencyOverrideOrDefault<WethAssetId>, // Get account's fee payment asset
			FungibleCurrencies<crate::Runtime>,        // Account balance inspector
		>,
	>;
	type RuntimeEvent = crate::RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressTruncated;
	type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
	type GasLimitStorageGrowthRatio = GasLimitStorageGrowthRatio;
	type Timestamp = crate::Timestamp;
	type WeightInfo = pallet_evm::weights::SubstrateWeight<crate::Runtime>;
	type SuicideQuickClearLimit = SuicideQuickClearLimit;
}

impl pallet_evm_chain_id::Config for crate::Runtime {}

impl pallet_ethereum::Config for crate::Runtime {
	type RuntimeEvent = crate::RuntimeEvent;
	type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
	type PostLogContent = PostLogContent;
	type ExtraDataLength = sp_core::ConstU32<1>;
}

pub struct EvmNonceProvider;
impl pallet_evm_accounts::EvmNonceProvider for EvmNonceProvider {
	fn get_nonce(evm_address: sp_core::H160) -> U256 {
		crate::EVM::account_basic(&evm_address).0.nonce
	}
}

impl pallet_evm_accounts::Config for crate::Runtime {
	type RuntimeEvent = crate::RuntimeEvent;
	type FeeMultiplier = sp_core::ConstU32<50>;
	type EvmNonceProvider = EvmNonceProvider;
	type ControllerOrigin = crate::SuperMajorityTechCommittee;
	type WeightInfo = crate::weights::pallet_evm_accounts::HydraWeight<crate::Runtime>;
}

parameter_types! {
	pub const DefaultBaseFeePerGas: u128 = DEFAULT_BASE_FEE_PER_GAS;
	pub const MinBaseFeePerGas: u128 = DEFAULT_BASE_FEE_PER_GAS.saturating_div(10);
	pub const MaxBaseFeePerGas: u128 = 14415000000; //To reach 10 dollar per omnipool trade
}

impl pallet_dynamic_evm_fee::Config for crate::Runtime {
	type AssetId = AssetId;
	type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
	type MinBaseFeePerGas = MinBaseFeePerGas;
	type MaxBaseFeePerGas = MaxBaseFeePerGas;
	type FeeMultiplier = TransactionPaymentMultiplier;
	type NativePriceOracle = AssetFeeOraclePriceProvider<
		NativeAssetId,
		crate::MultiTransactionPayment,
		crate::Router,
		OraclePriceProvider<AssetId, crate::EmaOracle, LRNA>,
		crate::MultiTransactionPayment,
		OracleEvmPeriod,
	>;
	type WethAssetId = WethAssetId;
	type WeightInfo = crate::weights::pallet_dynamic_evm_fee::HydraWeight<crate::Runtime>;
}
