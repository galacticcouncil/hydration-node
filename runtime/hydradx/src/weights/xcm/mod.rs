// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod pallet_xcm_benchmarks_fungible;
mod pallet_xcm_benchmarks_generic;

use crate::{BaseXcmWeight, MaxAssetsIntoHolding, RouterWeightInfo, Runtime};
use frame_support::weights::Weight;
use pallet_xcm_benchmarks_generic::WeightInfo as XcmGeneric;
use polkadot_xcm::latest::InteriorLocation;
use polkadot_xcm::v4::{QueryId, Response, WeightLimit, WildFungibility, Xcm, XcmWeightInfo};
use polkadot_xcm::DoubleEncoded;
use sp_std::vec;
use sp_std::vec::Vec;

use cumulus_primitives_core::{
	All, AllCounted, AllOf, AllOfCounted, Asset, AssetFilter, Assets, Junction, Junctions, Location, OriginKind,
	QueryResponseInfo,
};
use hydradx_traits::router::{AmmTradeWeights, PoolType, Trade};
use polkadot_xcm::prelude::{MaybeErrorCode, NetworkId, XcmError};

trait WeighAssets {
	fn weigh_assets(&self, weight: Weight) -> Weight;
}

const MAX_ASSETS: u64 = 100;

impl WeighAssets for AssetFilter {
	fn weigh_assets(&self, weight: Weight) -> Weight {
		match self {
			Self::Definite(assets) => weight.saturating_mul(assets.inner().iter().count() as u64),
			Self::Wild(asset) => match asset {
				All => weight.saturating_mul(MAX_ASSETS),
				AllOf { fun, .. } => match fun {
					WildFungibility::Fungible => weight,
					// Magic number 2 has to do with the fact that we could have up to 2 times
					// MaxAssetsIntoHolding in the worst-case scenario.
					WildFungibility::NonFungible => weight.saturating_mul((MaxAssetsIntoHolding::get() * 2) as u64),
				},
				AllCounted(count) => weight.saturating_mul(MAX_ASSETS.min(*count as u64)),
				AllOfCounted { count, .. } => weight.saturating_mul(MAX_ASSETS.min(*count as u64)),
			},
		}
	}
}

impl WeighAssets for Assets {
	fn weigh_assets(&self, weight: Weight) -> Weight {
		weight.saturating_mul(self.inner().iter().count() as u64)
	}
}

pub struct HydraXcmWeight<Call>(core::marker::PhantomData<Call>);
///!NOTE - We use BaseXcmWeight to not break anything, except for instructions where we really need to increase weights
impl<Call> XcmWeightInfo<Call> for HydraXcmWeight<Call> {
	fn withdraw_asset(_assets: &Assets) -> Weight {
		BaseXcmWeight::get()
	}
	fn reserve_asset_deposited(_assets: &Assets) -> Weight {
		BaseXcmWeight::get()
	}
	fn receive_teleported_asset(_assets: &Assets) -> Weight {
		// XCM Executor does not currently support receive_teleported_asset
		Weight::MAX
	}
	fn query_response(
		_query_id: &u64,
		_response: &Response,
		_max_weight: &Weight,
		_querier: &Option<Location>,
	) -> Weight {
		BaseXcmWeight::get()
	}
	fn transfer_asset(_assets: &Assets, _dest: &Location) -> Weight {
		BaseXcmWeight::get()
	}
	fn transfer_reserve_asset(_assets: &Assets, _dest: &Location, _xcm: &Xcm<()>) -> Weight {
		BaseXcmWeight::get()
	}
	fn transact(
		_origin_type: &OriginKind,
		_fallback_max_weight: &cumulus_primitives_core::Weight,
		_call: &DoubleEncoded<Call>,
	) -> Weight {
		BaseXcmWeight::get()
	}
	fn hrmp_new_channel_open_request(_sender: &u32, _max_message_size: &u32, _max_capacity: &u32) -> Weight {
		// XCM Executor does not currently support HRMP channel operations
		Weight::MAX
	}
	fn hrmp_channel_accepted(_recipient: &u32) -> Weight {
		// XCM Executor does not currently support HRMP channel operations
		Weight::MAX
	}
	fn hrmp_channel_closing(_initiator: &u32, _sender: &u32, _recipient: &u32) -> Weight {
		// XCM Executor does not currently support HRMP channel operations
		Weight::MAX
	}
	fn clear_origin() -> Weight {
		BaseXcmWeight::get()
	}
	fn descend_origin(_who: &InteriorLocation) -> Weight {
		BaseXcmWeight::get()
	}
	fn report_error(_query_response_info: &QueryResponseInfo) -> Weight {
		BaseXcmWeight::get()
	}

	fn deposit_asset(_assets: &AssetFilter, _dest: &Location) -> Weight {
		BaseXcmWeight::get()
	}
	fn deposit_reserve_asset(_assets: &AssetFilter, _dest: &Location, _xcm: &Xcm<()>) -> Weight {
		BaseXcmWeight::get()
	}
	fn exchange_asset(_give: &AssetFilter, _receive: &Assets, is_sell: &bool) -> Weight {
		//Route can be up max to 9 trades, and stableswap is the most expensive trade, then omnipool
		let worst_case_trades = vec![
			Trade {
				pool: PoolType::Stableswap(100),
				asset_in: 0,
				asset_out: 1,
			},
			Trade {
				pool: PoolType::Omnipool,
				asset_in: 1,
				asset_out: 2,
			},
			Trade {
				pool: PoolType::Stableswap(101),
				asset_in: 2,
				asset_out: 3,
			},
			Trade {
				pool: PoolType::Omnipool,
				asset_in: 3,
				asset_out: 4,
			},
			Trade {
				pool: PoolType::Stableswap(102),
				asset_in: 4,
				asset_out: 5,
			},
			Trade {
				pool: PoolType::Omnipool,
				asset_in: 5,
				asset_out: 6,
			},
			Trade {
				pool: PoolType::Stableswap(103),
				asset_in: 6,
				asset_out: 7,
			},
			Trade {
				pool: PoolType::Omnipool,
				asset_in: 7,
				asset_out: 8,
			},
			Trade {
				pool: PoolType::Stableswap(105),
				asset_in: 8,
				asset_out: 9,
			},
		];

		let route_weight = if *is_sell {
			RouterWeightInfo::sell_weight(&worst_case_trades)
		} else {
			RouterWeightInfo::buy_weight(&worst_case_trades)
		};

		XcmGeneric::<Runtime>::exchange_asset().saturating_add(route_weight) //Exchange asset already contains a router trade so we are overestimating it, which is fine
	}
	fn initiate_reserve_withdraw(_assets: &AssetFilter, _reserve: &Location, _xcm: &Xcm<()>) -> Weight {
		BaseXcmWeight::get()
	}
	fn initiate_teleport(_assets: &AssetFilter, _dest: &Location, _xcm: &Xcm<()>) -> Weight {
		Weight::MAX
	}

	fn report_holding(_response_info: &QueryResponseInfo, _assets: &AssetFilter) -> Weight {
		BaseXcmWeight::get()
	}
	fn buy_execution(_fees: &Asset, _weight_limit: &WeightLimit) -> Weight {
		BaseXcmWeight::get()
	}

	fn refund_surplus() -> Weight {
		BaseXcmWeight::get()
	}
	fn set_error_handler(_xcm: &Xcm<Call>) -> Weight {
		BaseXcmWeight::get()
	}
	fn set_appendix(_xcm: &Xcm<Call>) -> Weight {
		BaseXcmWeight::get()
	}
	fn clear_error() -> Weight {
		BaseXcmWeight::get()
	}

	fn claim_asset(_assets: &Assets, _ticket: &Location) -> Weight {
		BaseXcmWeight::get()
	}
	fn trap(_code: &u64) -> Weight {
		BaseXcmWeight::get()
	}
	fn subscribe_version(_query_id: &QueryId, _max_response_weight: &Weight) -> Weight {
		BaseXcmWeight::get()
	}
	fn unsubscribe_version() -> Weight {
		BaseXcmWeight::get()
	}
	fn burn_asset(assets: &Assets) -> Weight {
		BaseXcmWeight::get()
	}
	fn expect_asset(assets: &Assets) -> Weight {
		BaseXcmWeight::get()
	}
	fn expect_origin(_origin: &Option<Location>) -> Weight {
		BaseXcmWeight::get()
	}
	fn expect_error(_error: &Option<(u32, XcmError)>) -> Weight {
		BaseXcmWeight::get()
	}
	fn expect_transact_status(_transact_status: &MaybeErrorCode) -> Weight {
		BaseXcmWeight::get()
	}
	fn query_pallet(_module_name: &Vec<u8>, _response_info: &QueryResponseInfo) -> Weight {
		BaseXcmWeight::get()
	}
	fn expect_pallet(
		_index: &u32,
		_name: &Vec<u8>,
		_module_name: &Vec<u8>,
		_crate_major: &u32,
		_min_crate_minor: &u32,
	) -> Weight {
		BaseXcmWeight::get()
	}
	fn report_transact_status(_response_info: &QueryResponseInfo) -> Weight {
		BaseXcmWeight::get()
	}
	fn clear_transact_status() -> Weight {
		BaseXcmWeight::get()
	}
	fn universal_origin(_: &Junction) -> Weight {
		Weight::MAX
	}
	fn export_message(_: &NetworkId, _: &Junctions, _: &Xcm<()>) -> Weight {
		Weight::MAX
	}
	fn lock_asset(_: &Asset, _: &Location) -> Weight {
		Weight::MAX
	}
	fn unlock_asset(_: &Asset, _: &Location) -> Weight {
		Weight::MAX
	}
	fn note_unlockable(_: &Asset, _: &Location) -> Weight {
		Weight::MAX
	}
	fn request_unlock(_: &Asset, _: &Location) -> Weight {
		Weight::MAX
	}
	fn set_fees_mode(_: &bool) -> Weight {
		BaseXcmWeight::get()
	}
	fn set_topic(_topic: &[u8; 32]) -> Weight {
		BaseXcmWeight::get()
	}
	fn clear_topic() -> Weight {
		BaseXcmWeight::get()
	}
	fn alias_origin(_: &Location) -> Weight {
		Weight::MAX
	}
	fn unpaid_execution(_: &WeightLimit, _: &Option<Location>) -> Weight {
		BaseXcmWeight::get()
	}
}
