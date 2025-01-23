use std::marker::PhantomData;
use evm::ExitReason::Succeed;
use frame_support::ensure;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitive_types::{H256, U256};
use sp_arithmetic::FixedU128;
use sp_runtime::{DispatchError, RuntimeDebug};
use hydradx_traits::BoundErc20;
use hydradx_traits::evm::{CallContext, EVM};
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use primitives::{AccountId, AssetId, Balance, EvmAddress};
use crate::evm::Executor;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::evm::precompiles::handle::EvmDataWriter;
use crate::origins::Origin;

pub struct AaveTradeExecutor<Runtime>;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
    // Pool Addresses Provider
    GetPool = "getPool()",

    // Pool
    Supply = "supply(address,uint256,address,uint16)",
    Withdraw = "withdraw(address,uint256,address)",
    GetReserveData = "getReserveData(address)",
    GetConfiguration = "getConfiguration(address)",

    // AToken
    UnderlyingAssetAddress = "UNDERLYING_ASSET_ADDRESS()",
}

impl<Runtime> AaveTradeExecutor<Runtime>
{
    fn asset_address(asset_id: AssetId) -> Result<EvmAddress, DispatchError> {
        let asset = pallet_asset_registry::Pallet::<Runtime>::contract_address(asset_id)
            .ok_or_else(|| "Asset not registered")?;

        Ok(asset)
    }
    fn get_underlying_asset(atoken: AssetId) -> Result<EvmAddress, DispatchError> {
        // Get aToken EVM address
        let atoken_address = Self::asset_address(atoken)?;

        // Make view call to get underlying asset
        let context = CallContext::new_view(atoken_address);
        let data = Into::<u32>::into(Function::UnderlyingAssetAddress).to_be_bytes().to_vec();

        let (res, value) = Executor::<Runtime>::view(
            context,
            data,
            100_000,
        );

        ensure!(
            matches!(res, Succeed(Returned)),
            "Failed to get underlying asset address"
        );

        Ok(EvmAddress::from(H256::from_slice(&value)))
    }

    fn get_aave_pool_address() -> Result<EvmAddress, DispatchError> {
        let context = CallContext::new_view(Self::pap_address);
        let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();

        let (res, value) = Executor::<Runtime>::view(
            context,
            data,
            100_000,
        );

        ensure!(
            matches!(res, Succeed(Returned)),
            "Failed to get pool address"
        );

        Ok(EvmAddress::from(H256::from_slice(&value)))
    }

    fn supply_to_aave(
        who: AccountId,
        asset: AssetId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        let user_evm = AccountIdConverter::into_evm_address(who);
        let pool_address = Self::get_aave_pool_address()?;
        let asset_address = pallet_asset_registry::Pallet::<Runtime>::contract_address(asset).ok_or_else(|| "Asset not registered")?;

        let context = CallContext::new_call(pool_address, user_evm);
        let data = EvmDataWriter::new_with_selector(Function::Supply)
            .write(asset_address)
            .write(amount)
            .write(user_evm)
            .write(0u32)
            .build();

        let (res, _) = Executor::<Runtime>::call(
            context,
            data,
            U256::zero(),
            500_000,
        );

        ensure!(
            matches!(res, Succeed(Returned)),
            "Supply operation failed"
        );

        Ok(())
    }

    fn withdraw_from_aave(
        who: AccountId,
        atoken: AssetId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        let user_evm = AccountIdConverter::into_evm_address(who);
        let pool_address = Self::get_aave_pool_address()?;
        let underlying = Self::get_underlying_asset(atoken)?;

        let context = CallContext::new_call(pool_address, user_evm);
        let data = EvmDataWriter::new_with_selector(Function::Withdraw)
            .write(underlying)
            .write(amount)
            .write(user_evm)
            .build();

        let (res, _) = Executor::<Runtime>::call(
            context,
            data,
            U256::zero(),
            500_000,
        );

        ensure!(
            matches!(res, Succeed(Returned)),
            "Withdraw operation failed"
        );

        Ok(())
    }
}

impl<Runtime> TradeExecution<Origin, AccountId, AssetId, Balance> for AaveTradeExecutor<Runtime>
{
    type Error = DispatchError;

    fn calculate_sell(
        pool_type: PoolType<AssetId>,
        asset_in: AssetId,
        _asset_out: AssetId,
        amount_in: Balance,
    ) -> Result<Balance, ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        // For both supply and withdraw, amount out is always 1:1
        Ok(amount_in)
    }

    fn calculate_buy(
        pool_type: PoolType<AssetId>,
        asset_in: AssetId,
        _asset_out: AssetId,
        amount_out: Balance,
    ) -> Result<Balance, ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        Ok(amount_out)
    }

    fn execute_sell(
        who: AccountId,
        pool_type: PoolType<AssetId>,
        asset_in: AssetId,
        _asset_out: AssetId,
        amount_in: Balance,
        min_limit: Balance,
    ) -> Result<(), ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        ensure!(
            amount_in >= min_limit,
            ExecutorError::Error("Slippage exceeded".into())
        );

        if AaveTradeExecutor::<Runtime>::get_underlying_asset(asset_in).is_ok() {
            Self::withdraw_from_aave(who, asset_in, amount_in)
        } else {
            Self::supply_to_aave(who, asset_in, amount_in)
        }.map_err(ExecutorError::Error)
    }

    fn execute_buy(
        who: AccountId,
        pool_type: PoolType<AssetId>,
        asset_in: AssetId,
        _asset_out: AssetId,
        amount_out: Balance,
        max_limit: Balance,
    ) -> Result<(), ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        ensure!(
            amount_out <= max_limit,
            ExecutorError::Error("Slippage exceeded".into())
        );

        if Self::get_underlying_asset(asset_in).is_ok() {
            Self::withdraw_from_aave(who, asset_in, amount_out)
        } else {
            Self::supply_to_aave(who, asset_in, amount_out)
        }.map_err(ExecutorError::Error)
    }

    fn get_liquidity_depth(
        pool_type: PoolType<AssetId>,
        asset_in: AssetId,
        _asset_out: AssetId,
    ) -> Result<Balance, ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        let pool_address = Self::get_aave_pool_address().map_err(ExecutorError::Error)?;

        if Self::get_underlying_asset(asset_in).is_ok() {
            let context = CallContext::new_view(pool_address);
            let data = EvmDataWriter::new_with_selector(Function::GetReserveData)
                .write(underlying)
                .build();

            let (res, value) = Executor::<Runtime>::view(
                context,
                data,
                100_000,
            );

            ensure!(
                matches!(res, Succeed(Returned)),
                ExecutorError::Error("Failed to get reserve data".into())
            );

            // Parse available liquidity from reserve data
            // Value is tuple (configuration, liquidityIndex, currentLiquidityRate, ..., availableLiquidity)
            let available_liquidity = U256::from_big_endian(&value[192..224]); // availableLiquidity is at offset 192
            Ok(available_liquidity.try_into().unwrap_or(0))
        } else {
            // For wrapping (token -> aToken), check supply cap room
            let asset_address = AssetMapper::get_evm_address(asset_in)
                .ok_or_else(|| ExecutorError::Error("Asset not mapped to EVM address".into()))?;

            let context = CallContext::new_view(pool_address);
            let data = EvmDataWriter::new_with_selector(Function::GetConfiguration)
                .write(asset_address)
                .build();

            let (res, value) = Executor::<Runtime>::view(
                context,
                data,
                100_000,
            );

            ensure!(
                matches!(res, Succeed(Returned)),
                ExecutorError::Error("Failed to get configuration".into())
            );

            // Get current supply
            let data = EvmDataWriter::new_with_selector(Function::GetReserveData)
                .write(asset_address)
                .build();

            let (res, supply_data) = Executor::<Runtime>::view(
                context,
                data,
                100_000,
            );

            ensure!(
                matches!(res, Succeed(Returned)),
                ExecutorError::Error("Failed to get reserve data".into())
            );

            // Parse supply cap and current supply
            let supply_cap = U256::from_big_endian(&value[224..256]); // supplyCap at offset 224
            let current_supply = U256::from_big_endian(&supply_data[160..192]); // totalSupply at offset 160

            if supply_cap.is_zero() {
                // No supply cap
                Ok(Balance::MAX)
            } else {
                let available = supply_cap.saturating_sub(current_supply);
                Ok(available.try_into().unwrap_or(0))
            }
        }
    }

    fn calculate_spot_price_with_fee(
        pool_type: PoolType<AssetId>,
        _asset_in: AssetId,
        _asset_out: AssetId,
    ) -> Result<FixedU128, ExecutorError<Self::Error>> {
        if pool_type != PoolType::Aave {
            return Err(ExecutorError::NotSupported);
        }

        // Price is always 1:1
        Ok(FixedU128::from(1))
    }
}
