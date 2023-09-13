// Copyright 2019-2022 PureStake Inc.
// This file is part Utils package, originally developed by PureStake

// Utils is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Utils is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Utils.  If not, see <http://www.gnu.org/licenses/>.

//! Cost calculations.
//! TODO: PR EVM to make those cost calculations public.

use crate::evm::precompile::EvmResult;
use pallet_evm::{ExitError, PrecompileFailure};

pub fn log_costs(topics: usize, data_len: usize) -> EvmResult<u64> {
	// Cost calculation is copied from EVM code that is not publicly exposed by the crates.
	// https://github.com/rust-blockchain/evm/blob/master/gasometer/src/costs.rs#L148

	const G_LOG: u64 = 375;
	const G_LOGDATA: u64 = 8;
	const G_LOGTOPIC: u64 = 375;

	let topic_cost = G_LOGTOPIC.checked_mul(topics as u64).ok_or(PrecompileFailure::Error {
		exit_status: ExitError::OutOfGas,
	})?;

	let data_cost = G_LOGDATA.checked_mul(data_len as u64).ok_or(PrecompileFailure::Error {
		exit_status: ExitError::OutOfGas,
	})?;

	G_LOG
		.checked_add(topic_cost)
		.ok_or(PrecompileFailure::Error {
			exit_status: ExitError::OutOfGas,
		})?
		.checked_add(data_cost)
		.ok_or(PrecompileFailure::Error {
			exit_status: ExitError::OutOfGas,
		})
}
