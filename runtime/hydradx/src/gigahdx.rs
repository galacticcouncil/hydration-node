// SPDX-License-Identifier: Apache-2.0
//
// `MoneyMarketOperations` adapter that bridges `pallet-gigahdx` to the
// EVM-side AAVE V3 fork. See `specs/09-gigahdx-money-market-adapter.md`.
//
// `supply` mints aToken (GIGAHDX) on behalf of the user from their stHDX;
// `withdraw` burns aToken and returns stHDX. The pool address is read from
// `pallet_gigahdx::GigaHdxPoolContract` (settable via `set_pool_contract`).

use crate::evm::aave_trade_executor::Function as AaveFunction;
use crate::evm::evm_error_decoder::EvmErrorDecoder;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::evm::Erc20Currency;
use crate::evm::Executor;
use crate::Runtime;
use ethabi::ethereum_types::BigEndianHash;
use evm::ExitReason::Succeed;
use frame_support::sp_runtime::traits::Convert;
use frame_support::sp_runtime::DispatchError;
use hydradx_traits::evm::{CallContext, CallResult, Erc20Mapping, InspectEvmAccounts, ERC20, EVM};
use hydradx_traits::gigahdx::MoneyMarketOperations;
use primitive_types::U256;
use primitives::{AccountId, AssetId, Balance, EvmAddress};
use sp_core::H256;

const GAS_LIMIT: u64 = 500_000;

fn handle(result: CallResult) -> Result<(), DispatchError> {
	match &result.exit_reason {
		Succeed(_) => Ok(()),
		_ => {
			log::error!(
				target: "gigahdx::adapter",
				"AAVE EVM call failed: exit_reason={:?}, data=0x{}",
				result.exit_reason,
				hex::encode(&result.value),
			);
			Err(EvmErrorDecoder::convert(result))
		}
	}
}

pub struct AaveMoneyMarket;

impl AaveMoneyMarket {
	fn pool() -> Result<EvmAddress, DispatchError> {
		pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get()
			.ok_or(DispatchError::Other("gigahdx: pool contract not set"))
	}
}

impl MoneyMarketOperations<AccountId, AssetId, Balance> for AaveMoneyMarket {
	fn supply(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		let pool = Self::pool()?;

		// Approve the pool to pull `amount` of underlying.
		let approve_ctx = CallContext::new_call(asset_evm, who_evm);
		<Erc20Currency<Runtime> as ERC20>::approve(approve_ctx, pool, amount)?;

		// Pool.supply rounds the scaled balance down, so the actual aToken
		// minted may be 1+ wei less than `amount`. Read the user's aToken
		// balance before/after and return the delta — the pallet records that
		// as `Stakes.gigahdx`, preserving the invariant
		// `Stakes.gigahdx == aToken.balanceOf` that `LockableAToken.burn`'s
		// `freeBalance = balanceOf - locked` check relies on.
		let balance_before = Self::balance_of(who);

		let supply_ctx = CallContext::new_call(pool, who_evm);
		let mut data = Into::<u32>::into(AaveFunction::Supply).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset_evm).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());
		data.extend_from_slice(H256::from(who_evm).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::zero()).as_bytes()); // referralCode = 0
		handle(Executor::<Runtime>::call(supply_ctx, data, U256::zero(), GAS_LIMIT))?;

		let balance_after = Self::balance_of(who);
		Ok(balance_after.saturating_sub(balance_before))
	}

	fn withdraw(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		let pool = Self::pool()?;

		// Mirror the supply path — return the actual underlying received,
		// not the requested amount, so callers can reconcile against AAVE's
		// scaledBalance rounding. Symmetry with `supply` keeps round-trip
		// accounting consistent across rate drift.
		let balance_before = Self::balance_of(who);

		let withdraw_ctx = CallContext::new_call(pool, who_evm);
		let mut data = Into::<u32>::into(AaveFunction::Withdraw).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset_evm).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());
		data.extend_from_slice(H256::from(who_evm).as_bytes());
		handle(Executor::<Runtime>::call(withdraw_ctx, data, U256::zero(), GAS_LIMIT))?;

		let balance_after = Self::balance_of(who);
		Ok(balance_before.saturating_sub(balance_after))
	}

	fn balance_of(who: &AccountId) -> Balance {
		// Read aToken (GIGAHDX) balance via ERC20.balanceOf.
		let atoken_addr = HydraErc20Mapping::asset_address(crate::assets::GigaHdxAssetIdConst::get());
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		<Erc20Currency<Runtime> as ERC20>::balance_of(CallContext::new_view(atoken_addr), who_evm)
	}
}
