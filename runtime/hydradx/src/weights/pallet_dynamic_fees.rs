// This file is part of HydraDX.

// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;
use crate::*;

pub struct WeightInfo<T>(PhantomData<T>);

pub struct HydraWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_dynamic_fees::WeightInfo for HydraWeight<T> {
	fn set_asset_fee() -> Weight {
		Weight::zero()
	}

	fn remove_asset_fee() -> Weight {
		Weight::zero()
	}
}