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
use hydradx_traits::{
	evm::{CallContext, EVM},
	RawOracle,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_stableswap::traits::{Peg, PegOracle as Oracle, Source};
use primitives::{AssetId, Balance, BlockNumber};
use sp_runtime::{traits::BlockNumberProvider, DispatchError, RuntimeDebug, SaturatedConversion};
use sp_std::marker::PhantomData;

const VIEW_GAS_LIMIT: u64 = 100_000;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AggregatorV3Interface {
	Decimals = "decimals()",
	LatestRound = "latestRoundData()",
}

pub type CallResult = (ExitReason, Vec<u8>);

pub struct PegOracle<Runtime, Evm>(PhantomData<(Runtime, Evm)>);

impl<Runtime, Evm> Oracle<AssetId, Balance, BlockNumber> for PegOracle<Runtime, Evm>
where
	Runtime: pallet_ema_oracle::Config + frame_system::Config,
	Evm: EVM<CallResult>,
{
	type Error = DispatchError;

	fn get(source: Source<AssetId>) -> Result<Peg<BlockNumber>, Self::Error> {
		match source {
			Source::Oracle((source, period, asset_a, asset_b)) => {
				let entry = pallet_ema_oracle::Pallet::<Runtime>::get_raw_entry(source, asset_a, asset_b, period)
					.map_err(|_| DispatchError::Other("PegOracle not available"))?;

				Ok(Peg {
					val: (entry.price.0, entry.price.1),
					updated_at: entry.updated_at.saturated_into(),
				})
			}
			Source::Value(peg) => Ok(Peg {
				val: peg,
				updated_at: frame_system::Pallet::<Runtime>::current_block_number().saturated_into(),
			}),
			Source::ChainlinkOracle(addr) => {
				//TODO: handle decimals from contracts. Is it necessary?

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

					DispatchError::Other("PetOracle not available")
				})?;

				let price_raw = decoded[1].clone().into_uint().ok_or_else(|| {
					log::error!(target: "stableswap-peg-oracle",
						"Failed to convert decoded price to uint:  raw_decoded: {:?}", decoded[1]);
					DispatchError::Other("PetOracle not available")
				})?;

				let price_raw = decoded[3].clone().into_uint().ok_or_else(|| {
					log::error!(target: "stableswap-peg-oracle",
						"Failed to convert decoded updated_at to uint:  raw_decoded: {:?}", decoded[3]);
					DispatchError::Other("PetOracle not available")
				})?;

				//TODO::
				// * convert updatedAt timestamp to block number
				// * create num & denom from `answer` and noramlize to 18 decimals

				return Err(DispatchError::Other("TODO: Not Implemented"));
			}
		}
	}
}
