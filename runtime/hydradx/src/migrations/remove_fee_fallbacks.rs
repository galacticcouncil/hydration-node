// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

//! Migrations retiring the DOT/XYK fee fallbacks and the treasury-pooled insufficient-asset
//! existential deposit.

use crate::{AccountId, Currencies, NativeAssetId, Runtime, TreasuryAccount, Weight};
use frame_support::{
	migrations::{MigrationId, SteppedMigration, SteppedMigrationError},
	traits::OnRuntimeUpgrade,
	weights::WeightMeter,
};
use orml_traits::currency::MultiLockableCurrency;
use pallet_transaction_multi_payment::{AcceptedCurrencies, AccountCurrencyMap};

/// Lock id previously used to pool insufficient-asset EDs on the treasury account.
const SUFFICIENCY_LOCK: frame_support::traits::LockIdentifier = *b"insuffED";

const PALLET_MIGRATIONS_ID: &[u8; 24] = b"remove-fee-fallbacks-v1_";

/// Drops `AccountCurrencyMap` entries pointing at an asset that is no longer a valid fee
/// currency — the accounts that opted into the removed XYK/DOT tier.
///
/// Stepped because the map is unbounded. `pallet-migrations` holds normal extrinsics back until
/// it finishes, so no affected account can transact against a half-purged map.
pub struct PurgeUnsupportedFeeCurrencies;

impl SteppedMigration for PurgeUnsupportedFeeCurrencies {
	type Cursor = AccountId;
	type Identifier = MigrationId<24>;

	fn id() -> Self::Identifier {
		MigrationId {
			pallet_id: *PALLET_MIGRATIONS_ID,
			version_from: 0,
			version_to: 1,
		}
	}

	fn step(
		mut cursor: Option<Self::Cursor>,
		meter: &mut WeightMeter,
	) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
		let native = NativeAssetId::get();
		// Per entry: read the map entry, read `AcceptedCurrencies`, and in the worst case remove.
		let required = <Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 1);

		if meter.remaining().any_lt(required) {
			return Err(SteppedMigrationError::InsufficientWeight { required });
		}

		loop {
			if meter.try_consume(required).is_err() {
				break;
			}

			let mut iter = match &cursor {
				Some(last) => {
					AccountCurrencyMap::<Runtime>::iter_from(AccountCurrencyMap::<Runtime>::hashed_key_for(last))
				}
				None => AccountCurrencyMap::<Runtime>::iter(),
			};

			match iter.next() {
				Some((who, currency)) => {
					if currency != native && !AcceptedCurrencies::<Runtime>::contains_key(currency) {
						AccountCurrencyMap::<Runtime>::remove(&who);
					}
					cursor = Some(who);
				}
				None => {
					cursor = None;
					break;
				}
			}
		}

		Ok(cursor)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		use codec::Encode;
		let (total, unsupported) = count_fee_currencies();
		log::info!(
			target: "runtime::migration",
			"PurgeUnsupportedFeeCurrencies: {unsupported} of {total} AccountCurrencyMap entries to purge",
		);
		Ok((total, unsupported).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use codec::Decode;
		let (total_before, unsupported_before) = <(u32, u32)>::decode(&mut &state[..])
			.map_err(|_| "PurgeUnsupportedFeeCurrencies: bad pre_upgrade state")?;
		let (total, unsupported) = count_fee_currencies();
		frame_support::ensure!(
			unsupported == 0,
			"unsupported fee currencies remain in AccountCurrencyMap"
		);
		frame_support::ensure!(
			total == total_before - unsupported_before,
			"PurgeUnsupportedFeeCurrencies removed a supported entry"
		);
		Ok(())
	}
}

#[cfg(feature = "try-runtime")]
fn count_fee_currencies() -> (u32, u32) {
	let native = NativeAssetId::get();
	AccountCurrencyMap::<Runtime>::iter_values().fold((0, 0), |(total, unsupported), currency| {
		let bad = currency != native && !AcceptedCurrencies::<Runtime>::contains_key(currency);
		(total + 1, unsupported + u32::from(bad))
	})
}

/// Retires the treasury-side accounting of the old insufficient-asset ED: the pooled HDX lock and
/// the payment counter.
///
/// Pre-existing `sufficients` refs are deliberately left in place and are never released — nothing
/// on chain attributes a ref to the retired scheme, and `sufficients` is shared with EVM account
/// machinery, so releasing on kill could consume refs the scheme never created. An emptied legacy
/// holder lingers as an empty system account instead of being reaped. No one is refunded anything:
/// the pooled scheme already paid out zero in practice.
pub struct RetireInsufficientEdPool;

impl OnRuntimeUpgrade for RetireInsufficientEdPool {
	fn on_runtime_upgrade() -> Weight {
		let _ = <Currencies as MultiLockableCurrency<AccountId>>::remove_lock(
			SUFFICIENCY_LOCK,
			NativeAssetId::get(),
			&TreasuryAccount::get(),
		);

		let counter_key = frame_support::storage::storage_prefix(b"AssetRegistry", b"ExistentialDepositCounter");
		frame_support::storage::unhashed::kill(&counter_key);

		log::info!(
			target: "runtime::migration",
			"RetireInsufficientEdPool: released the treasury ED lock and removed the ED counter",
		);

		<Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 3)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		frame_support::ensure!(
			!pallet_balances::Locks::<Runtime>::get(TreasuryAccount::get())
				.iter()
				.any(|lock| lock.id == SUFFICIENCY_LOCK),
			"treasury still carries the insufficient-asset ED lock"
		);

		let counter_key = frame_support::storage::storage_prefix(b"AssetRegistry", b"ExistentialDepositCounter");
		frame_support::ensure!(
			!frame_support::storage::unhashed::exists(&counter_key),
			"ExistentialDepositCounter still present"
		);

		Ok(())
	}
}
