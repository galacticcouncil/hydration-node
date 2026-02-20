#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use codec::Encode;
use core::marker::PhantomData;
use ethabi::decode;
use ethabi::ParamType;
use evm::ExitReason;
use evm::ExitSucceed;
use frame_support::ensure;
use frame_support::pallet_prelude::RuntimeDebug;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::evm::CallContext;
use hydradx_traits::router::PoolType;
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::Price;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use precompile_utils::evm::writer::EvmDataWriter;
use primitive_types::U256;
use primitives::EvmAddress;
use sp_arithmetic::traits::SaturatedConversion;
use sp_std::boxed::Box;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

pub trait DataProvider {
	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> (ExitReason, Vec<u8>);

	fn borrowing_contract() -> EvmAddress;

	fn address_to_asset(address: EvmAddress) -> Option<AssetId>;
}

const GAS_LIMIT: u64 = 1000_000;
const LOG_TARGET: &str = "aave_simulator";

#[module_evm_utility_macro::generate_function_selector]
#[derive(Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	// Pool
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
	GetReserveData = "getReserveData(address)",
	GetConfiguration = "getConfiguration(address)",
	GetReservesList = "getReservesList()",
	// AToken
	UnderlyingAssetAddress = "UNDERLYING_ASSET_ADDRESS()",
	ScaledTotalSupply = "scaledTotalSupply()",
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
pub struct ReserveData {
	pub configuration: U256,
	pub liquidity_index: U256,
	pub current_liquidity_rate: U256,
	pub variable_borrow_index: U256,
	pub current_variable_borrow_rate: U256,
	pub current_stable_borrow_rate: U256,
	pub last_update_timestamp: U256,
	pub id: u16,
	pub atoken_address: EvmAddress,
	pub stable_debt_token_address: EvmAddress,
	pub variable_debt_token_address: EvmAddress,
	pub interest_rate_strategy_address: EvmAddress,
	pub accrued_to_treasury: U256,
	pub scaled_total_supply: U256,
}

#[allow(dead_code)]
impl ReserveData {
	fn decimals(&self) -> u8 {
		//bit 48-55: Decimals
		let mask = U256::from(0xFF) << 48;
		((self.configuration & mask) >> 48).saturated_into()
	}

	fn supply_cap_raw(&self) -> U256 {
		//bit 116-151 supply cap in whole tokens, supplyCap == 0 => no cap
		let mask = U256::from((1u128 << 36) - 1) << 116;
		(self.configuration & mask) >> 116
	}

	fn supply_cap(&self) -> U256 {
		if self.supply_cap_raw().is_zero() {
			U256::MAX
		} else {
			self.supply_cap_raw().saturating_mul(
				U256::from(10)
					.checked_pow(self.decimals().into())
					.unwrap_or_else(U256::one),
			)
		}
	}

	fn current_supply(&self) -> U256 {
		self.scaled_total_supply
			.saturating_add(self.accrued_to_treasury)
			.saturating_mul(self.liquidity_index)
			/ U256::from(10).pow(27.into())
	}

	fn available_supply(&self) -> U256 {
		self.supply_cap().saturating_sub(self.current_supply())
	}
}

#[derive(Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq)]
pub struct Snapshot {
	/// Map of aave reserves
	pub reserves: BTreeMap<AssetId, ReserveData>,
	/// Aave pool contract address
	pub contract: EvmAddress,
}

//NOTE: This is tmp. dummy impl. of aave simulator that always trade 1:1 and doesn't do any checks.
pub struct Simulator<DataProvider>(PhantomData<DataProvider>);

impl<DP: DataProvider> Simulator<DP> {
	fn get_reserves_list(aave: EvmAddress) -> Result<Vec<EvmAddress>, SimulatorError> {
		let ctx = CallContext::new_view(aave);
		let data = EvmDataWriter::new_with_selector(Function::GetReservesList).build();

		let (exit_reason, value) = DP::view(ctx, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get reserves list reason: {:?}, value: {:?}", exit_reason, value);
			return Err(SimulatorError::Other);
		}

		let param_types = vec![ParamType::Array(Box::new(ParamType::Address))];

		let decoded = decode(&param_types, value.as_ref()).map_err(|_| {
			log::error!(target: LOG_TARGET, "to decore reserves list");
			SimulatorError::Other
		})?;

		// Convert decoded addresses to EvmAddress format
		let addresses = decoded[0]
			.clone()
			.into_array()
			.ok_or(SimulatorError::Other)?
			.into_iter()
			.filter_map(|addr| addr.into_address())
			.map(|addr| EvmAddress::from_slice(addr.as_bytes()))
			.collect();

		Ok(addresses)
	}

	fn get_reserve_data(aave: EvmAddress, reserve: EvmAddress) -> Result<ReserveData, SimulatorError> {
		let ctc = CallContext::new_view(aave);
		let data = EvmDataWriter::new_with_selector(Function::GetReserveData)
			.write(reserve)
			.build();

		let (exit_reason, value) = DP::view(ctc, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get reserves data, reason: {:?}, value: {:?}", exit_reason, value);
			return Err(SimulatorError::Other);
		}

		let param_types = vec![
			ParamType::Uint(256), // configuration
			ParamType::Uint(256), // liquidityIndex
			ParamType::Uint(256), // variableBorrowIndex
			ParamType::Uint(256), // currentLiquidityRate
			ParamType::Uint(256), // currentVariableBorrowRate
			ParamType::Uint(256), // currentStableBorrowRate
			ParamType::Uint(256), // lastUpdateTimestamp
			ParamType::Uint(16),  // id
			ParamType::Address,   // aTokenAddress
			ParamType::Address,   // stableDebtTokenAddress
			ParamType::Address,   // variableDebtTokenAddress
			ParamType::Address,   // interestRateStrategyAddress
			ParamType::Uint(256), // accruedToTreasury
		];

		let decoded = decode(&param_types, value.as_ref()).map_err(|_| {
			log::error!(target: LOG_TARGET, "to decode reserve data");
			SimulatorError::Other
		})?;

		// Ensure sufficient length
		ensure!(decoded.len() == param_types.len(), {
			log::error!(target: LOG_TARGET, "invalid reserve data");
			SimulatorError::Other
		});

		let a_token = EvmAddress::from_slice(decoded[8].clone().into_address().unwrap_or_default().as_ref());
		Ok(ReserveData {
			configuration: decoded[0].clone().into_uint().unwrap_or_default(),
			liquidity_index: decoded[1].clone().into_uint().unwrap_or_default(),
			current_liquidity_rate: decoded[3].clone().into_uint().unwrap_or_default(),
			variable_borrow_index: decoded[2].clone().into_uint().unwrap_or_default(),
			current_variable_borrow_rate: decoded[4].clone().into_uint().unwrap_or_default(),
			current_stable_borrow_rate: decoded[5].clone().into_uint().unwrap_or_default(),
			last_update_timestamp: decoded[6].clone().into_uint().unwrap_or_default(),
			id: decoded[7].clone().into_uint().unwrap_or_default().saturated_into(),
			atoken_address: a_token,
			stable_debt_token_address: EvmAddress::from_slice(
				decoded[9].clone().into_address().unwrap_or_default().as_ref(),
			),
			variable_debt_token_address: EvmAddress::from_slice(
				decoded[10].clone().into_address().unwrap_or_default().as_ref(),
			),
			interest_rate_strategy_address: EvmAddress::from_slice(
				decoded[11].clone().into_address().unwrap_or_default().as_ref(),
			),
			accrued_to_treasury: decoded[12].clone().into_uint().unwrap_or_default(),
			scaled_total_supply: Simulator::<DP>::get_scaled_total_supply(a_token)?,
		})
	}

	fn get_scaled_total_supply(reserve: EvmAddress) -> Result<U256, SimulatorError> {
		let ctx = CallContext::new_view(reserve);
		let data = EvmDataWriter::new_with_selector(Function::ScaledTotalSupply).build();

		let (exit_reason, value) = DP::view(ctx, data, GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: LOG_TARGET, "to get scaled total supply, reserve: {:?}, reason: {:?}, value: {:?}", reserve, exit_reason, value );
			return Err(SimulatorError::Other);
		}

		ensure!(value.len() <= 32, {
			log::error!(target: LOG_TARGET, "invalid scaled total supply");
			SimulatorError::Other
		});
		Ok(U256::from_big_endian(value.as_slice()))
	}
}

impl<DP: DataProvider> AmmSimulator for Simulator<DP> {
	type Snapshot = Snapshot;

	fn snapshot() -> Self::Snapshot {
		let mut snapshot = Snapshot {
			reserves: BTreeMap::new(),
			contract: DP::borrowing_contract(),
		};

		let Ok(reserves) = Self::get_reserves_list(snapshot.contract) else {
			return snapshot;
		};

		for addr in reserves {
			let Ok(reserve) = Self::get_reserve_data(snapshot.contract, addr) else {
				snapshot.reserves.clear();
				break;
			};

			let Some(asset_id) = DP::address_to_asset(addr) else {
				log::error!(target: LOG_TARGET, "to map reserve address to asset, reserve: {:?}", addr);
				snapshot.reserves.clear();
				break;
			};

			snapshot.reserves.insert(asset_id, reserve);
		}

		snapshot
	}

	fn pool_type() -> PoolType<AssetId> {
		PoolType::Aave
	}

	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		_max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if snapshot.reserves.get(&asset_in).is_none() && snapshot.reserves.get(&asset_out).is_none() {
			return Err(SimulatorError::AssetNotFound);
		}

		Ok((
			snapshot.clone(),
			TradeResult {
				amount_in: amount_out,
				amount_out,
			},
		))
	}

	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if snapshot.reserves.get(&asset_in).is_none() && snapshot.reserves.get(&asset_out).is_none() {
			return Err(SimulatorError::AssetNotFound);
		}

		Ok((
			snapshot.clone(),
			TradeResult {
				amount_in,
				amount_out: amount_in,
			},
		))
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Price, SimulatorError> {
		if snapshot.reserves.get(&asset_in).is_none() && snapshot.reserves.get(&asset_out).is_none() {
			return Err(SimulatorError::AssetNotFound);
		}
		Ok(Ratio { n: 1, d: 1 })
	}

	fn can_trade(_asset_in: AssetId, _asset_out: AssetId, _snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		// no, Dave, you cannot trade this now.
		None
	}
}
