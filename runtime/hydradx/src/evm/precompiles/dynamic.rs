// This file is part of https://github.com/galacticcouncil/HydraDX-node
// Copyright (C) 2021-2025  Intergalactic, Limited (GIB).
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

use crate::evm::precompiles::{is_precompile, is_standard_precompile};
use pallet_evm::{ExitRevert, IsPrecompileResult, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult};
use precompile_utils::precompile_set::{PrecompileCheckSummary, PrecompileChecks, PrecompileSetFragment};
use primitive_types::H160;
use sp_std::marker::PhantomData;

/// A trait for dynamic precompiles that can be identified by an address.
pub trait DynamicPrecompile {
	fn is_precompile(address: H160, gas: u64) -> IsPrecompileResult;
}

/// A wrapper for dynamic precompiles that applies security checks.
pub struct DynamicPrecompileWrapper<P> {
	_phantom: PhantomData<P>,
}

impl<P> PrecompileSetFragment for DynamicPrecompileWrapper<P>
where
	P: Precompile + DynamicPrecompile,
{
	fn new() -> Self {
		Self { _phantom: PhantomData }
	}

	fn is_precompile(&self, address: H160, gas: u64) -> IsPrecompileResult {
		P::is_precompile(address, gas)
	}

	fn execute<R: pallet_evm::Config>(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let address = handle.code_address();

		if let IsPrecompileResult::Answer {
			is_precompile: false, ..
		} = self.is_precompile(address, handle.remaining_gas())
		{
			return None;
		}

		// Disallow calling custom precompiles with DELEGATECALL or CALLCODE
		if handle.context().address != address && is_precompile(address) && !is_standard_precompile(address) {
			return Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "precompile cannot be called with DELEGATECALL or CALLCODE".into(),
			}));
		}

		Some(P::execute(handle))
	}

	fn used_addresses(&self) -> sp_std::vec::Vec<H160> {
		sp_std::vec![]
	}

	fn summarize_checks(&self) -> sp_std::vec::Vec<PrecompileCheckSummary> {
		// TODO: consider adding checks
		sp_std::vec![]
	}
}
