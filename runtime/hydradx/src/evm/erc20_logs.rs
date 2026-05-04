// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::Runtime;
use frame_support::sp_runtime::DispatchResult;
use hydradx_traits::evm::{Erc20Mapping, InspectEvmAccounts};
use orml_traits::currency::{
	OnDeposit, OnRepatriate, OnReserve, OnSlash, OnSlashReserved, OnTransfer, OnUnreserve, OnWithdraw,
};
use orml_traits::BalanceStatus;
use pallet_synthetic_logs::{
	encode_u256_be, h160_to_h256, reserved_address_of, Pallet as SyntheticLogs, TRANSFER_TOPIC,
};
use primitive_types::{H160, U256};
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::{AccountId, AssetId, Balance};
use sp_std::{marker::PhantomData, vec::Vec};

pub struct EmitErc20TransferLog;

fn evm_address_of(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

fn push_transfer_log(asset: AssetId, from: H160, to: H160, amount: Balance) {
	let address = HydraErc20Mapping::asset_address(asset);
	let mut data = Vec::with_capacity(32);
	data.extend_from_slice(&encode_u256_be(U256::from(amount)));
	let log = ethereum::Log {
		address,
		topics: sp_std::vec![TRANSFER_TOPIC, h160_to_h256(from), h160_to_h256(to)],
		data,
	};
	if !crate::evm::runner::append_to_current_evm_frame(log.clone()) {
		SyntheticLogs::<Runtime>::push(address, log);
	}
}

impl OnTransfer<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_transfer(asset: AssetId, from: &AccountId, to: &AccountId, amount: Balance) -> DispatchResult {
		if amount == 0 {
			return Ok(());
		}
		push_transfer_log(asset, evm_address_of(from), evm_address_of(to), amount);
		Ok(())
	}
}

impl OnDeposit<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_deposit(asset: AssetId, who: &AccountId, amount: Balance) -> DispatchResult {
		if amount == 0 {
			return Ok(());
		}
		push_transfer_log(asset, H160::zero(), evm_address_of(who), amount);
		Ok(())
	}
}

impl OnSlash<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_slash(asset: AssetId, who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		push_transfer_log(asset, evm_address_of(who), H160::zero(), amount);
	}
}

impl OnWithdraw<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_withdraw(asset: AssetId, who: &AccountId, amount: Balance) -> DispatchResult {
		if amount == 0 {
			return Ok(());
		}
		push_transfer_log(asset, evm_address_of(who), H160::zero(), amount);
		Ok(())
	}
}

impl OnReserve<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_reserve(asset: AssetId, who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(asset, owner, reserved_address_of(owner), amount);
	}
}

impl OnUnreserve<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_unreserve(asset: AssetId, who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(asset, reserved_address_of(owner), owner, amount);
	}
}

impl OnSlashReserved<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_slash_reserved(asset: AssetId, who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(asset, reserved_address_of(owner), H160::zero(), amount);
	}
}

impl OnRepatriate<AccountId, AssetId, Balance> for EmitErc20TransferLog {
	fn on_repatriate(
		asset: AssetId,
		slashed: &AccountId,
		beneficiary: &AccountId,
		amount: Balance,
		status: BalanceStatus,
	) {
		if amount == 0 {
			return;
		}
		let from = reserved_address_of(evm_address_of(slashed));
		let to = match status {
			BalanceStatus::Free => evm_address_of(beneficiary),
			BalanceStatus::Reserved => reserved_address_of(evm_address_of(beneficiary)),
		};
		push_transfer_log(asset, from, to, amount);
	}
}

impl pallet_balances::BalancesHooks<AccountId, Balance> for EmitErc20TransferLog {
	fn on_transfer(from: &AccountId, to: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		push_transfer_log(CORE_ASSET_ID, evm_address_of(from), evm_address_of(to), amount);
	}

	fn on_mint(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		push_transfer_log(CORE_ASSET_ID, H160::zero(), evm_address_of(who), amount);
	}

	fn on_burn(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		push_transfer_log(CORE_ASSET_ID, evm_address_of(who), H160::zero(), amount);
	}

	fn on_dust_lost(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		push_transfer_log(CORE_ASSET_ID, evm_address_of(who), H160::zero(), amount);
	}

	fn on_reserve(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(CORE_ASSET_ID, owner, reserved_address_of(owner), amount);
	}

	fn on_unreserve(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(CORE_ASSET_ID, reserved_address_of(owner), owner, amount);
	}

	fn on_slash_reserved(who: &AccountId, amount: Balance) {
		if amount == 0 {
			return;
		}
		let owner = evm_address_of(who);
		push_transfer_log(CORE_ASSET_ID, reserved_address_of(owner), H160::zero(), amount);
	}

	fn on_repatriate(
		slashed: &AccountId,
		beneficiary: &AccountId,
		amount: Balance,
		status: frame_support::traits::tokens::BalanceStatus,
	) {
		if amount == 0 {
			return;
		}
		let from = reserved_address_of(evm_address_of(slashed));
		let to = match status {
			frame_support::traits::tokens::BalanceStatus::Free => evm_address_of(beneficiary),
			frame_support::traits::tokens::BalanceStatus::Reserved => reserved_address_of(evm_address_of(beneficiary)),
		};
		push_transfer_log(CORE_ASSET_ID, from, to, amount);
	}
}

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
