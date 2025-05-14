// This file is part of hydradx-adapters.

// Copyright (C) 2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{vec, Vec};
use ethabi::{decode, ParamType};
use evm::{ExitReason, ExitSucceed};
use frame_support::traits::UnixTime;
use hydradx_traits::{
	evm::{CallContext, EVM},
	RawEntry, RawOracle,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_stableswap::traits::PegOracle as Oracle;
use pallet_stableswap::types::PegSource;
use primitives::{constants::time::SECS_PER_BLOCK, AssetId, Balance, BlockNumber};
use sp_core::U256;
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating, Zero},
	DispatchError, RuntimeDebug, SaturatedConversion,
};
use sp_std::marker::PhantomData;

const VIEW_GAS_LIMIT: u64 = 100_000;
//NOTE: Money Market oracle is always 8 decimals
const MM_ORACLE_DENOM: u128 = 100_000_000;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AggregatorV3Interface {
	LatestRound = "latestRoundData()",
}

pub type CallResult = (ExitReason, Vec<u8>);

pub struct PegOracle<Runtime, Evm>(PhantomData<(Runtime, Evm)>);

impl<Runtime, Evm> Oracle<AssetId, Balance, BlockNumber> for PegOracle<Runtime, Evm>
where
	Runtime: pallet_ema_oracle::Config + frame_system::Config + pallet_timestamp::Config,
	Evm: EVM<CallResult>,
{
	type Error = DispatchError;

	fn get(peg_asset: AssetId, source: PegSource<AssetId>) -> Result<RawEntry<Balance, BlockNumber>, Self::Error> {
		match source {
			PegSource::Oracle((source, period, oracle_asset)) => {
				let entry =
					pallet_ema_oracle::Pallet::<Runtime>::get_raw_entry(source, oracle_asset, peg_asset, period)
						.map_err(|_| DispatchError::Other("PegOracle not available"))?;

				Ok(RawEntry {
					price: (entry.price.0, entry.price.1),
					volume: Default::default(),
					liquidity: Default::default(),
					updated_at: entry.updated_at.saturated_into(),
				})
			}
			PegSource::Value(peg) => Ok(RawEntry {
				price: peg,
				volume: Default::default(),
				liquidity: Default::default(),
				updated_at: frame_system::Pallet::<Runtime>::current_block_number().saturated_into(),
			}),
			//NOTE: Money Market oracles must have 8 decimals so this oracle is hardcoded with 8
			//decimals.
			PegSource::MMOracle(addr) => {
				let ctx = CallContext::new_view(addr);
				let data = Into::<u32>::into(AggregatorV3Interface::LatestRound)
					.to_be_bytes()
					.to_vec();
				let (r, value) = Evm::view(ctx, data, VIEW_GAS_LIMIT);
				if r != ExitReason::Succeed(ExitSucceed::Returned) {
					log::error!(target: "stableswap-peg-oracle",
						"Failed to get peg oracle value. Contract: {:?}, Reason: {:?}, Response: {:?}", addr, r, value);

					return Err(DispatchError::Other("PetOracle not available"));
				}

				let param_types = vec![
					ParamType::Uint(80),  //roundId
					ParamType::Uint(256), //answer
					ParamType::Uint(256), //createdAt
					ParamType::Uint(256), //updatedAt
					ParamType::Uint(80),  //answeredInRound
				];

				let decoded = decode(&param_types, value.as_ref()).map_err(|e| {
					log::error!(target: "stableswap-peg-oracle",
						"Failed to decode returned value. Contract: {:?}, Value: {:?}, Err: {:?}", addr, value, e);
					DispatchError::Other("PegOracle not available")
				})?;

				let price_num = decoded[1].clone().into_uint().unwrap_or_default();
				let updated_at = decoded[3].clone().into_uint().unwrap_or_default();

				let price_num: u128 = TryInto::try_into(price_num).unwrap_or_default();
				if price_num.is_zero() {
					log::error!(target: "stableswap-peg-oracle",
						"Oracle's price can't be zero. Conract: {:?}, Price: {:?}, UpdatedAt: {:?}", addr, price_num, updated_at);

					return Err(DispatchError::Other("PegOracle not available"));
				}

				let now = U256::from(pallet_timestamp::Pallet::<Runtime>::now().as_secs());
				let diff_blocks: BlockNumber = now
					.saturating_sub(updated_at)
					.saturated_into::<u128>()
					.saturating_div(SECS_PER_BLOCK.into())
					.saturated_into::<BlockNumber>();

				if diff_blocks.is_zero() {
					log::error!(target: "stableswap-peg-oracle",
						"Oracle can't be updated in the same block. Constract: {:?}, DiffBlocks: {:?}", addr, diff_blocks);

					return Err(DispatchError::Other("PegOracle not available"));
				}

				let current_block = frame_system::Pallet::<Runtime>::current_block_number();
				let updated_at = current_block.saturating_sub(diff_blocks.into());

				if updated_at.is_zero() {
					log::error!(target: "stableswap-peg-oracle",
						"Calculated updated at is 0th block. CurrentBlock: {:?}, DiffBlocks: {:?}", current_block, diff_blocks);

					return Err(DispatchError::Other("PegOracle not available"));
				}

				Ok(RawEntry {
					price: (price_num, MM_ORACLE_DENOM),
					volume: Default::default(),
					liquidity: Default::default(),
					updated_at: updated_at.saturated_into(),
				})
			}
		}
	}
}
