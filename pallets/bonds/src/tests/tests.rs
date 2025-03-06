// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

use crate::tests::mock::*;
use crate::*;

#[test]
fn parse_name_should_work() {
	let n = Pallet::<Test>::bond_name(0_u32, 1689844300000_u64);
	assert_eq!(Pallet::<Test>::parse_bond_name(n), Ok(0_u32));

	let n = Pallet::<Test>::bond_name(u32::MAX, 1689844300000_u64);
	assert_eq!(Pallet::<Test>::parse_bond_name(n), Ok(u32::MAX));

	let n = Pallet::<Test>::bond_name(1, 1689844300000_u64);
	assert_eq!(Pallet::<Test>::parse_bond_name(n), Ok(1));

	let n = Pallet::<Test>::bond_name(13_124, 1689844300000_u64);
	assert_eq!(Pallet::<Test>::parse_bond_name(n), Ok(13_124));

	let n = Pallet::<Test>::bond_name(789_970_979, 1689844300000_u64);
	assert_eq!(Pallet::<Test>::parse_bond_name(n), Ok(789_970_979));
}
