// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use super::*;
use crate::mock::*;
use hex_literal::hex;

#[test]
fn no_previous_chain() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(GenesisHistory::previous_chain(), Chain::default());
	})
}

#[test]
fn some_previous_chain() {
	let chain = Chain {
		genesis_hash: H256::from(hex!("0ed32bfcab4a83517fac88f2aa7cbc2f88d3ab93be9a12b6188a036bf8a943c2")),
		last_block_hash: H256::from(hex!("5800478f2cac4166d40c1ebe80dddbec47275d4b102f228b8a3af54d86d64837")),
	};
	ExtBuilder { chain: chain.clone() }.build().execute_with(|| {
		assert_eq!(GenesisHistory::previous_chain(), chain.clone());
	})
}
