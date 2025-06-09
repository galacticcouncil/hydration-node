// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

//! Track configurations for governance.

use super::*;
use once_cell::sync::OnceCell;
use primitives::constants::{
	currency::UNITS,
	time::{HOURS, MINUTES},
};

const fn percent(x: i32) -> sp_arithmetic::FixedI64 {
	sp_arithmetic::FixedI64::from_rational(x as u128, 100)
}

use pallet_referenda::Curve;
const APP_LINEAR: Curve = Curve::make_linear(7, 7, percent(50), percent(100));
const APP_LINEAR_FLAT: Curve = Curve::make_linear(4, 7, percent(50), percent(100));
const APP_RECIP: Curve = Curve::make_reciprocal(1, 7, percent(80), percent(50), percent(100));
const SUP_LINEAR: Curve = Curve::make_linear(7, 7, percent(0), percent(50));
const SUP_LINEAR_FROM_25: Curve = Curve::make_linear(7, 7, percent(0), percent(25));
const SUP_RECIP: Curve = Curve::make_reciprocal(5, 7, percent(1), percent(0), percent(50));
const SUP_FAST_RECIP: Curve = Curve::make_reciprocal(3, 7, percent(1), percent(0), percent(50));
const SUP_WHITELISTED_CALLER: Curve = Curve::make_reciprocal(1, 28, percent(3), percent(2), percent(50));

// Using OnceCell to initialize the tracks only once at Runtime. This satisfies the 'static lifetime requirement.
type TrackTuple = (u16, pallet_referenda::TrackInfo<Balance, BlockNumber>);
static TRACKS: OnceCell<Vec<TrackTuple>> = OnceCell::new();

pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
	type Id = u16;
	type RuntimeOrigin = <RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin;
	fn tracks() -> &'static [TrackTuple] {
		let root_enactment_period = if is_testnet() { MINUTES } else { 10 * MINUTES };

		TRACKS.get_or_init(|| {
			vec![
				(
					0,
					pallet_referenda::TrackInfo {
						name: "root",
						max_deciding: 3,
						decision_deposit: 1_000_000 * UNITS,
						prepare_period: HOURS,
						decision_period: 7 * DAYS,
						confirm_period: 12 * HOURS,
						min_enactment_period: root_enactment_period,
						min_approval: APP_RECIP,
						min_support: SUP_LINEAR,
					},
				),
				(
					1,
					pallet_referenda::TrackInfo {
						name: "whitelisted_caller",
						max_deciding: 3,
						decision_deposit: 50_000 * UNITS,
						prepare_period: 10 * MINUTES,
						decision_period: 3 * DAYS,
						confirm_period: 4 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_RECIP,
						min_support: SUP_WHITELISTED_CALLER,
					},
				),
				(
					2,
					pallet_referenda::TrackInfo {
						name: "referendum_canceller",
						max_deciding: 3,
						decision_deposit: 250_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 3 * DAYS,
						confirm_period: 60 * MINUTES,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_LINEAR_FLAT,
						min_support: SUP_FAST_RECIP,
					},
				),
				(
					3,
					pallet_referenda::TrackInfo {
						name: "referendum_killer",
						max_deciding: 3,
						decision_deposit: 750_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 3 * DAYS,
						confirm_period: HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_LINEAR_FLAT,
						min_support: SUP_FAST_RECIP,
					},
				),
				(
					4,
					pallet_referenda::TrackInfo {
						name: "general_admin",
						max_deciding: 3,
						decision_deposit: 250_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 3 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_RECIP,
						min_support: SUP_RECIP,
					},
				),
				(
					5,
					pallet_referenda::TrackInfo {
						name: "treasurer",
						max_deciding: 3,
						decision_deposit: 750_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 12 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_RECIP,
						min_support: SUP_LINEAR_FROM_25,
					},
				),
				(
					6,
					pallet_referenda::TrackInfo {
						name: "spender",
						max_deciding: 3,
						decision_deposit: 100_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 3 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_LINEAR,
						min_support: SUP_RECIP,
					},
				),
				(
					7,
					pallet_referenda::TrackInfo {
						name: "tipper",
						max_deciding: 3,
						decision_deposit: 10_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 3 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_LINEAR_FLAT,
						min_support: SUP_FAST_RECIP,
					},
				),
				(
					8,
					pallet_referenda::TrackInfo {
						name: "omnipool_admin",
						max_deciding: 3,
						decision_deposit: 250_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 3 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_RECIP,
						min_support: SUP_RECIP,
					},
				),
				(
					9,
					pallet_referenda::TrackInfo {
						name: "economic_parameters",
						max_deciding: 3,
						decision_deposit: 750_000 * UNITS,
						prepare_period: 60 * MINUTES,
						decision_period: 7 * DAYS,
						confirm_period: 12 * HOURS,
						min_enactment_period: 10 * MINUTES,
						min_approval: APP_RECIP,
						min_support: SUP_LINEAR_FROM_25,
					},
				),
			]
		})
	}
	fn track_for(id: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
		if let Ok(system_origin) = frame_system::RawOrigin::try_from(id.clone()) {
			match system_origin {
				frame_system::RawOrigin::Root => Ok(0),
				_ => Err(()),
			}
		} else if let Ok(custom_origin) = origins::Origin::try_from(id.clone()) {
			match custom_origin {
				origins::Origin::WhitelistedCaller => Ok(1),
				origins::Origin::ReferendumCanceller => Ok(2),
				origins::Origin::ReferendumKiller => Ok(3),
				origins::Origin::GeneralAdmin => Ok(4),
				origins::Origin::Treasurer => Ok(5),
				origins::Origin::Spender => Ok(6),
				origins::Origin::Tipper => Ok(7),
				origins::Origin::OmnipoolAdmin => Ok(8),
				origins::Origin::EconomicParameters => Ok(9),
			}
		} else {
			Err(())
		}
	}
}
pallet_referenda::impl_tracksinfo_get!(TracksInfo, Balance, BlockNumber);
