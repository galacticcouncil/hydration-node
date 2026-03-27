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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]
#![allow(clippy::useless_conversion)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

use frame_support::dispatch::PostDispatchInfo;
use hydradx_traits::evm::ExtraGasSupport;
use hydradx_traits::evm::MaybeEvmCall;
use pallet_evm::{ExitReason, GasWeightMapping};
use sp_runtime::{traits::Dispatchable, DispatchError, DispatchResultWithInfo};
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
use frame_support::pallet_prelude::Weight;
use frame_support::traits::Get;
pub use pallet::*;

pub mod hyperbridge_cleanup;
pub use hyperbridge_cleanup::{do_cleanup_step, Stage};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::FullCodec;
	use frame_support::dispatch::DispatchErrorWithPostInfo;
	use frame_support::{
		dispatch::{GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;
	use pallet_evm::{ExitReason, ExitSucceed};
	use sp_runtime::traits::{Dispatchable, Hash};
	use sp_std::boxed::Box;

	pub type AccountId = u64;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching call type.
		type RuntimeCall: IsType<<Self as frame_system::Config>::RuntimeCall>
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ FullCodec
			+ TypeInfo
			+ From<frame_system::Call<Self>>
			+ Parameter;

		/// The trait to check whether RuntimeCall is [pallet_evm::Call::call].
		type EvmCallIdentifier: MaybeEvmCall<<Self as Config>::RuntimeCall>;

		type TreasuryManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type AaveManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type EmergencyAdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type TreasuryAccount: Get<Self::AccountId>;
		type DefaultAaveManagerAccount: Get<Self::AccountId>;
		type EmergencyAdminAccount: Get<Self::AccountId>;

		/// The origin to manage hyperbridge migration ongoing status.
		type MigrationOperatorOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Gas to Weight conversion.
		type GasWeightMapping: GasWeightMapping;

		/// The weight information for this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn aave_manager_account)]
	pub type AaveManagerAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery, T::DefaultAaveManagerAccount>;

	#[pallet::storage]
	#[pallet::whitelist_storage]
	#[pallet::getter(fn extra_gas)]
	pub type ExtraGas<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Whether the background ISMP storage cleanup is active.
	#[pallet::storage]
	pub type CleanupEnabled<T: Config> = StorageValue<_, bool, ValueQuery, DefaultCleanupState>;

	#[pallet::type_value]
	pub fn DefaultCleanupState() -> bool {
		true
	}

	/// Current stage of the background ISMP storage cleanup.
	#[pallet::storage]
	pub type CleanupStage<T: Config> = StorageValue<_, Stage, OptionQuery>;

	#[pallet::storage]
	#[pallet::whitelist_storage]
	#[pallet::unbounded]
	#[pallet::getter(fn last_evm_call_exit_reason)]
	pub type LastEvmCallExitReason<T: Config> = StorageValue<_, ExitReason, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The EVM call execution failed. This happens when the EVM returns an exit reason
		/// other than `ExitSucceed(Returned)` or `ExitSucceed(Stopped)`.
		EvmCallFailed,
		/// The provided call is not an EVM call. This extrinsic only accepts `pallet_evm::Call::call`.
		NotEvmCall,
		/// The EVM call ran out of gas.
		EvmOutOfGas,
		/// The EVM call resulted in an arithmetic overflow or underflow.
		EvmArithmeticOverflowOrUnderflow,
		/// Aave - supply cap has been exceeded.
		AaveSupplyCapExceeded,
		/// Aave - borrow cap has been exceeded.
		AaveBorrowCapExceeded,
		/// Aave - health factor is not below the threshold.
		AaveHealthFactorNotBelowThreshold,
		/// Aave - health factor is lesser than the liquidation threshold
		AaveHealthFactorLowerThanLiquidationThreshold,
		/// Aave - there is not enough collateral to cover a new borrow
		CollateralCannotCoverNewBorrow,
		/// Aave - the reserve is paused and no operations are allowed
		AaveReservePaused,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		TreasuryManagerCallDispatched {
			call_hash: T::Hash,
			result: DispatchResultWithPostInfo,
		},
		AaveManagerCallDispatched {
			call_hash: T::Hash,
			result: DispatchResultWithPostInfo,
		},
		EmergencyAdminCallDispatched {
			call_hash: T::Hash,
			result: DispatchResultWithPostInfo,
		},
		/// Emitted each block when cleanup deletes a batch of keys.
		HyperbridgeCleanupProgress { stage: Stage, keys_deleted: u32 },
		/// Emitted when all keys in a stage are removed and cleanup advances.
		HyperbridgeCleanupStageCompleted { stage: Stage },
		/// Emitted when all three stages are done and cleanup disables itself.
		HyperbridgeCleanupCompleted,
		/// Emitted when cleanup is paused or resumed via extrinsic.
		HyperbridgeCleanupStatusChanged { paused: bool },
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Reset the last EVM call exit reason on block finalization.
		fn on_finalize(_n: BlockNumberFor<T>) {
			LastEvmCallExitReason::<T>::kill();
		}

		/// Run a bounded chunk of ISMP storage cleanup during idle time.
		fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			if !CleanupEnabled::<T>::get() {
				return T::DbWeight::get().reads(1);
			}

			// Use the remaining weight capped to at most 70% of max_block.
			let max_block = T::BlockWeights::get().max_block;
			let cap_perbill = sp_runtime::Perbill::from_percent(70);
			let cap = Weight::from_parts(
				cap_perbill * max_block.ref_time(),
				cap_perbill * max_block.proof_size(),
			);

			let budget = remaining_weight.min(cap);
			let per_key_weight = T::DbWeight::get().reads_writes(2, 1);

			let k_ref = budget.ref_time().checked_div(per_key_weight.ref_time()).unwrap_or(0);
			let k_proof = budget.proof_size().checked_div(per_key_weight.proof_size()).unwrap_or(k_ref);

			let limit = k_ref.min(k_proof);
			if limit == 0 {
				return T::WeightInfo::cleanup_on_idle_limit_zero();
			}

			let limit_u32 = limit.min(u32::MAX as u64) as u32;
			let stage = CleanupStage::<T>::get().unwrap_or(Stage::StateCommitments);
			let (done, keys_deleted) = do_cleanup_step(stage, limit_u32);

			if keys_deleted > 0 {
				Self::deposit_event(Event::HyperbridgeCleanupProgress { stage, keys_deleted });
			}

			let base_cleanup_weight = T::WeightInfo::cleanup_on_idle(keys_deleted);
			if done {
				Self::deposit_event(Event::HyperbridgeCleanupStageCompleted { stage });

				return match stage.next() {
					Some(next) => {
						CleanupStage::<T>::put(next);
						base_cleanup_weight.saturating_add(T::DbWeight::get().writes(1))
					}
					None => {
						// All stages complete.
						CleanupEnabled::<T>::put(false);
						CleanupStage::<T>::kill();
						Self::deposit_event(Event::HyperbridgeCleanupCompleted);

						base_cleanup_weight.saturating_add(T::DbWeight::get().writes(2))
					}
				};
			}

			T::WeightInfo::cleanup_on_idle(keys_deleted)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			let call_len = call.encoded_size() as u32;

			T::WeightInfo::dispatch_as_treasury(call_len)
				.saturating_add(call_weight)
		})]
		pub fn dispatch_as_treasury(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			T::TreasuryManagerOrigin::ensure_origin(origin)?;

			let call_hash = T::Hashing::hash_of(&call);
			let call_len = call.encoded_size() as u32;

			let (result, actual_weight) = Self::do_dispatch(
				frame_system::Origin::<T>::Signed(T::TreasuryAccount::get()).into(),
				*call,
			);
			let actual_weight = actual_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_as_treasury(call_len)));

			Self::deposit_event(Event::<T>::TreasuryManagerCallDispatched { call_hash, result });

			Ok(actual_weight.into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			let call_len = call.encoded_size() as u32;

			T::WeightInfo::dispatch_as_aave_manager(call_len)
				.saturating_add(call_weight)
		})]
		pub fn dispatch_as_aave_manager(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			T::AaveManagerOrigin::ensure_origin(origin)?;

			let call_hash = T::Hashing::hash_of(&call);
			let call_len = call.encoded_size() as u32;

			let (result, actual_weight) = Self::do_dispatch(
				frame_system::Origin::<T>::Signed(AaveManagerAccount::<T>::get()).into(),
				*call,
			);
			let actual_weight =
				actual_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_as_aave_manager(call_len)));

			Self::deposit_event(Event::<T>::AaveManagerCallDispatched { call_hash, result });

			Ok(actual_weight.into())
		}

		/// Sets the Aave manager account to be used as origin for dispatching calls.
		///
		/// This doesn't actually changes any ACL in the pool.
		///
		/// This is intented to be mainly used in testnet environments, where the manager account
		/// can be different.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::note_aave_manager())]
		pub fn note_aave_manager(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			AaveManagerAccount::<T>::put(account);
			Ok(())
		}

		/// Dispatch a call with extra gas.
		///
		/// This allows executing calls with additional weight (gas) limit.
		/// The extra gas is not refunded, even if not used.
		#[pallet::call_index(3)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			let call_len = call.encoded_size() as u32;
			let gas_weight = T::GasWeightMapping::gas_to_weight(*extra_gas, true);
			T::WeightInfo::dispatch_with_extra_gas(call_len)
				.saturating_add(call_weight)
				.saturating_add(gas_weight)
		})]
		pub fn dispatch_with_extra_gas(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
			extra_gas: u64,
		) -> DispatchResultWithPostInfo {
			ExtraGas::<T>::set(extra_gas);
			let (result, actual_weight) = Self::do_dispatch(origin, *call);
			ExtraGas::<T>::kill();

			if extra_gas == 0u64 {
				return result;
			}

			// We need to add the extra gas to the actual weight - because evm execution does not account for it
			// If actual weight is None, we still account for extra gas
			let actual_weight = if let Some(weight) = actual_weight {
				let extra_weight = T::GasWeightMapping::gas_to_weight(extra_gas, true);
				Some(weight.saturating_add(extra_weight))
			} else {
				None
			};

			match result {
				Ok(_) => Ok(PostDispatchInfo {
					actual_weight,
					pays_fee: Pays::Yes,
				}),
				Err(err) => Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight,
						pays_fee: Pays::Yes,
					},
					error: err.error,
				}),
			}
		}

		/// Execute a single EVM call.
		/// This extrinsic will fail if the EVM call returns any other ExitReason than `ExitSucceed(Returned)` or `ExitSucceed(Stopped)`.
		/// Look the [hydradx_runtime::evm::runner::WrapRunner] implementation for details.
		///
		/// Parameters:
		/// - `origin`: Signed origin.
		/// - `call`: presumably `pallet_evm::Call::call` as boxed `RuntimeCall`.
		///
		/// Emits `EvmCallFailed` event when failed.
		#[pallet::call_index(4)]
		#[pallet::weight({
			let evm_call_weight = call.get_dispatch_info().call_weight;
			let evm_call_len = call.encoded_size() as u32;
			T::WeightInfo::dispatch_evm_call(evm_call_len)
				.saturating_add(evm_call_weight)
		})]
		pub fn dispatch_evm_call(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			ensure!(T::EvmCallIdentifier::is_evm_call(&call), Error::<T>::NotEvmCall);

			let (result, actual_weight) = Self::do_dispatch(origin, *call);
			let post_info = PostDispatchInfo {
				actual_weight,
				pays_fee: Pays::Yes,
			};

			if let Some(exit_reason) = LastEvmCallExitReason::<T>::get() {
				match exit_reason {
					ExitReason::Succeed(ExitSucceed::Returned) | ExitReason::Succeed(ExitSucceed::Stopped) => {}
					_ => {
						return Err(DispatchErrorWithPostInfo {
							post_info,
							error: Error::<T>::EvmCallFailed.into(),
						});
					}
				}
			}

			match result {
				Ok(_) => Ok(post_info),
				Err(err) => Err(DispatchErrorWithPostInfo {
					post_info,
					error: err.error,
				}),
			}
		}

		/// Dispatch a call as the emergency admin account.
		///
		/// This is a fast path for the Technical Committee to react to emergencies
		/// (e.g., pausing exploited markets) without waiting for a full referendum.
		/// The inner call is dispatched as a Signed origin from the configured
		/// emergency admin account.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `EmergencyAdminOrigin` (TC majority or Root).
		/// - `call`: The runtime call to dispatch as the emergency admin.
		///
		/// Emits `EmergencyAdminCallDispatched` with the call hash and dispatch result.
		#[pallet::call_index(5)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			let call_len = call.encoded_size() as u32;

			T::WeightInfo::dispatch_as_emergency_admin(call_len)
				.saturating_add(call_weight)
		})]
		pub fn dispatch_as_emergency_admin(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			T::EmergencyAdminOrigin::ensure_origin(origin)?;

			let call_hash = T::Hashing::hash_of(&call);
			let call_len = call.encoded_size() as u32;

			let (result, actual_weight) = Self::do_dispatch(
				frame_system::Origin::<T>::Signed(T::EmergencyAdminAccount::get()).into(),
				*call,
			);
			let actual_weight =
				actual_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_as_emergency_admin(call_len)));

			Self::deposit_event(Event::<T>::EmergencyAdminCallDispatched { call_hash, result });

			Ok(actual_weight.into())
		}

		/// Enable/pause the background ISMP storage cleanup. If enabled for the first time,
		/// starting from the first stage.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::pause_hyperbridge_cleanup())]
		pub fn pause_hyperbridge_cleanup(origin: OriginFor<T>, do_pause: bool) -> DispatchResult {
			T::MigrationOperatorOrigin::ensure_origin(origin)?;
			CleanupEnabled::<T>::put(!do_pause);

			Self::deposit_event(Event::HyperbridgeCleanupStatusChanged { paused: do_pause });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Dispatch the call from the specified account as Signed Origin.
	///
	/// Return the result and the actual weight of the dispatched call if there is some.
	fn do_dispatch(
		origin: T::RuntimeOrigin,
		call: <T as Config>::RuntimeCall,
	) -> (DispatchResultWithInfo<PostDispatchInfo>, Option<Weight>) {
		let result = call.dispatch(origin);

		let call_actual_weight = match result {
			Ok(call_post_info) => call_post_info.actual_weight,
			Err(call_err) => call_err.post_info.actual_weight,
		};

		(result, call_actual_weight)
	}
}

// PUBLIC API
impl<T: Config> Pallet<T> {
	/// Decrease the gas for a specific account.
	pub fn decrease_extra_gas(amount: u64) {
		if amount == 0 {
			return;
		}
		let current_value = ExtraGas::<T>::take();
		let new_value = current_value.saturating_sub(amount);
		if new_value > 0 {
			ExtraGas::<T>::set(new_value);
		}
	}

	pub fn set_last_evm_call_exit_reason(reason: &ExitReason) {
		LastEvmCallExitReason::<T>::put(reason);
	}
}

impl<T: Config> ExtraGasSupport for Pallet<T> {
	fn set_extra_gas(gas: u64) {
		ExtraGas::<T>::set(gas);
	}

	fn clear_extra_gas() {
		ExtraGas::<T>::kill();
	}

	fn out_of_gas_error() -> DispatchError {
		Error::<T>::EvmOutOfGas.into()
	}
}
