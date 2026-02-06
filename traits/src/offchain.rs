// This file is part of hydradx-traits.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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

use frame_system::offchain::CreateTransactionBase;

/// Trait for creating bare (unsigned) extrinsics for offchain workers.
/// This is used for unsigned transactions, not inherents.
///
/// NOTE: This trait is available in frame-system >= 42.0.0 as `frame_system::offchain::CreateBare`.
/// Remove this local definition when upgrading to that version.
pub trait CreateBare<LocalCall>: CreateTransactionBase<LocalCall> {
	/// Create a bare extrinsic (unsigned transaction).
	fn create_bare(call: Self::RuntimeCall) -> Self::Extrinsic;
}
