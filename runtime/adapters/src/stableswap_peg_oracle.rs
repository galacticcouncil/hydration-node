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

use hydradx_traits::RawOracle;
use pallet_stableswap::traits::{Peg, PegOracle as Oracle, Source};
use primitives::{AssetId, Balance, BlockNumber};
use sp_runtime::{traits::BlockNumberProvider, DispatchError, SaturatedConversion};
use sp_std::marker::PhantomData;

pub struct PegOracle<Runtime>(PhantomData<Runtime>);

impl<Runtime> Oracle<AssetId, Balance, BlockNumber> for PegOracle<Runtime>
where
	Runtime: pallet_ema_oracle::Config + frame_system::Config,
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
			_ => Err(DispatchError::Other("Unsupported peg oracle source")),
		}
	}
}
