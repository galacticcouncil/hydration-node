// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! ERC-20 `Transfer(address,address,uint256)` log emission for orml-tokens
//! mutation hooks. Each transfer/deposit/slash on a non-bound asset becomes a
//! log buffered in `pallet_synthetic_logs`, which flushes synthetic ethereum
//! transactions on `on_finalize` so eth-rpc surfaces them.
//!
//! Bound erc20 assets bypass orml-tokens entirely (they route through the
//! EVM runner via `pallet_currencies`), so this handler never fires for them
//! — there's no double-emission risk.

use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::Runtime;
use frame_support::sp_runtime::DispatchResult;
use hydradx_traits::evm::{Erc20Mapping, InspectEvmAccounts};
use orml_traits::currency::{OnDeposit, OnSlash, OnTransfer};
use pallet_synthetic_logs::{encode_u256_be, h160_to_h256, Pallet as SyntheticLogs, TRANSFER_TOPIC};
use primitive_types::{H160, U256};
use primitives::{AccountId, AssetId, Balance};
use sp_std::{marker::PhantomData, vec::Vec};

/// `OnTransfer` / `OnDeposit` / `OnSlash` handler that emits ERC-20
/// `Transfer(from, to, amount)` logs. `amount == 0` is skipped — orml allows
/// zero-amount calls but indexers usually filter them anyway.
pub struct EmitErc20TransferLog;

fn evm_address_of(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

fn build_transfer_log(asset: AssetId, from: H160, to: H160, amount: Balance) -> ethereum::Log {
	let address = HydraErc20Mapping::asset_address(asset);
	let mut data = Vec::with_capacity(32);
	data.extend_from_slice(&encode_u256_be(U256::from(amount)));
	ethereum::Log {
		address,
		topics: sp_std::vec![TRANSFER_TOPIC, h160_to_h256(from), h160_to_h256(to)],
		data,
	}
}

fn push_transfer_log(asset: AssetId, from: H160, to: H160, amount: Balance) {
	let log = build_transfer_log(asset, from, to, amount);
	let address = HydraErc20Mapping::asset_address(asset);
	SyntheticLogs::<Runtime>::push(address, log);
}

impl OnTransfer<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_transfer(asset: AssetId, from: &AccountId, to: &AccountId, amount: Balance) -> DispatchResult {
		if amount == 0 || crate::evm::runner::is_in_evm() {
			return Ok(());
		}
		push_transfer_log(asset, evm_address_of(from), evm_address_of(to), amount);
		Ok(())
	}
}

impl OnDeposit<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_deposit(asset: AssetId, who: &AccountId, amount: Balance) -> DispatchResult {
		if amount == 0 || crate::evm::runner::is_in_evm() {
			return Ok(());
		}
		// Mint: Transfer(0x0, who, amount).
		push_transfer_log(asset, H160::zero(), evm_address_of(who), amount);
		Ok(())
	}
}

impl OnSlash<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_slash(asset: AssetId, who: &AccountId, amount: Balance) {
		if amount == 0 || crate::evm::runner::is_in_evm() {
			return;
		}
		// Burn-via-slash: Transfer(who, 0x0, amount). NOTE: orml's `OnSlash`
		// is invoked with the requested slash amount, before clamping to the
		// account's actual free balance. If the account holds less than
		// `amount`, the emitted log slightly overstates the burn — accepted
		// for v1; revisit if indexer fidelity demands it.
		push_transfer_log(asset, evm_address_of(who), H160::zero(), amount);
	}
}

/// Generic 2-tuple combinator for `OnDeposit` — orml-traits doesn't provide
/// tuple impls. Used to chain the existing circuit-breaker issuance fuse with
/// our log emission.
pub struct OnDepositTuple<A, B>(PhantomData<(A, B)>);

impl<A, B> OnDeposit<AccountId, AssetId, Balance> for OnDepositTuple<A, B>
where
	A: OnDeposit<AccountId, AssetId, Balance>,
	B: OnDeposit<AccountId, AssetId, Balance>,
{
	fn on_deposit(currency_id: AssetId, who: &AccountId, amount: Balance) -> DispatchResult {
		A::on_deposit(currency_id, who, amount)?;
		B::on_deposit(currency_id, who, amount)?;
		Ok(())
	}
}
