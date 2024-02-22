// Originally created by Acala. Modified by GalacticCouncil.

// Copyright (C) 2020-2022 Acala Foundation, GalacticCouncil.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_support::assert_ok;

benchmarks! {

	pause_transaction {
		let origin = T::UpdateOrigin::try_successful_origin().unwrap();
	}: {
		assert_ok!(crate::Pallet::<T>::pause_transaction(origin, b"Balances".to_vec(), b"transfer".to_vec()));
	}

	unpause_transaction {
		let origin = T::UpdateOrigin::try_successful_origin().unwrap();
		crate::Pallet::<T>::pause_transaction(origin, b"Balances".to_vec(), b"transfer".to_vec())?;
		let origin = T::UpdateOrigin::try_successful_origin().unwrap();
	}:{
		assert_ok!(crate::Pallet::<T>::unpause_transaction(origin, b"Balances".to_vec(), b"transfer".to_vec()));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder.build(), super::Runtime);
}
