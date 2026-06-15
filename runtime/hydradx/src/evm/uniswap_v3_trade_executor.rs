use crate::evm::executor::{BalanceOf, NonceIdOf};
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::evm::precompiles::handle::EvmDataWriter;
use crate::evm::Executor;
use ethabi::{decode, ParamType};
use evm::ExitReason::Succeed;
use evm::ExitSucceed;
use frame_support::ensure;
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use hydradx_traits::evm::{CallContext, Erc20Mapping, InspectEvmAccounts, EVM};
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_broadcast::types::Asset;
use pallet_evm::{AddressMapping, GasWeightMapping};
use primitive_types::U256;
use primitives::{AccountId, AssetId, Balance, EvmAddress};
use sp_arithmetic::traits::{SaturatedConversion, Saturating};
use sp_arithmetic::FixedPointNumber;
use sp_arithmetic::FixedU128;
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;
use sp_std::vec;

pub struct UniswapV3TradeExecutor<T>(PhantomData<T>);

pub type UniswapV3 = UniswapV3TradeExecutor<crate::Runtime>;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool(address,address,uint24)",
	Slot0 = "slot0()",
	Liquidity = "liquidity()",
	Token0 = "token0()",
	Token1 = "token1()",
	BalanceOf = "balanceOf(address)",
	QuoteExactInputSingle = "quoteExactInputSingle((address,address,uint256,uint24,uint160))",
	QuoteExactOutputSingle = "quoteExactOutputSingle((address,address,uint256,uint24,uint160))",
	ExactInputSingle = "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))",
	ExactOutputSingle = "exactOutputSingle((address,address,uint24,address,uint256,uint256,uint160))",
	Approve = "approve(address,uint256)",
}

const VIEW_GAS_LIMIT: u64 = 500_000;
const QUOTE_GAS_LIMIT: u64 = 1_000_000;
const TRADE_GAS_LIMIT: u64 = 1_000_000;
const IN_GIVEN_OUT_ROUNDING: Balance = 1;
const FEE_DENOMINATOR: u128 = 1_000_000;

pub fn evm_token_address(asset: AssetId) -> EvmAddress {
	HydraErc20Mapping::asset_address(asset)
}

pub fn sort_tokens(a: EvmAddress, b: EvmAddress) -> (EvmAddress, EvmAddress) {
	if a < b {
		(a, b)
	} else {
		(b, a)
	}
}

fn price_token1_per_token0(sqrt_price_x96: U256) -> FixedU128 {
	let scaled = sqrt_price_x96
		.checked_mul(U256::from(1_000_000_000u64))
		.unwrap_or(U256::MAX)
		>> 96;
	let inner = scaled.checked_mul(scaled).unwrap_or(U256::MAX).saturated_into::<u128>();
	FixedU128::from_inner(inner)
}

impl<T> UniswapV3TradeExecutor<T>
where
	T: frame_system::Config
		+ pallet_evm::Config
		+ pallet_dispatcher::Config
		+ pallet_parameters::Config
		+ pallet_evm_accounts::Config,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + frame_support::traits::IsType<sp_runtime::AccountId32>,
	BalanceOf<T>: TryFrom<U256> + Into<U256> + Default,
	NonceIdOf<T>: Into<T::Nonce>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
{
	fn factory() -> Result<EvmAddress, ExecutorError<DispatchError>> {
		pallet_parameters::Pallet::<T>::uniswap_v3_factory()
			.ok_or(ExecutorError::Error("uniswapv3: factory not configured".into()))
	}

	fn quoter() -> Result<EvmAddress, ExecutorError<DispatchError>> {
		pallet_parameters::Pallet::<T>::uniswap_v3_quoter()
			.ok_or(ExecutorError::Error("uniswapv3: quoter not configured".into()))
	}

	pub fn pool_address(
		factory: EvmAddress,
		asset_a: AssetId,
		asset_b: AssetId,
		fee: u32,
	) -> Result<Option<EvmAddress>, ExecutorError<DispatchError>> {
		let (token0, token1) = sort_tokens(evm_token_address(asset_a), evm_token_address(asset_b));
		let context = CallContext::new_view(factory);
		let data = EvmDataWriter::new_with_selector(Function::GetPool)
			.write(token0)
			.write(token1)
			.write(U256::from(fee))
			.build();
		let result = Executor::<T>::view(context, data, VIEW_GAS_LIMIT);
		ensure!(
			matches!(result.exit_reason, Succeed(ExitSucceed::Returned)),
			ExecutorError::Error("uniswapv3: getPool failed".into())
		);
		let decoded = decode(&[ParamType::Address], result.value.as_ref())
			.map_err(|_| ExecutorError::Error("uniswapv3: getPool decode failed".into()))?;
		let pool = decoded
			.first()
			.and_then(|token| token.clone().into_address())
			.map(|addr| EvmAddress::from_slice(addr.as_bytes()))
			.ok_or(ExecutorError::Error("uniswapv3: getPool returned no address".into()))?;
		Ok((pool != EvmAddress::zero()).then_some(pool))
	}

	pub fn quote_out_given_in(
		asset_in: AssetId,
		asset_out: AssetId,
		fee: u32,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<DispatchError>> {
		let quoter = Self::quoter()?;
		let token_in = evm_token_address(asset_in);
		let token_out = evm_token_address(asset_out);
		let data = EvmDataWriter::new_with_selector(Function::QuoteExactInputSingle)
			.write(token_in)
			.write(token_out)
			.write(U256::from(amount_in))
			.write(U256::from(fee))
			.write(U256::zero())
			.build();
		let result = Executor::<T>::view(CallContext::new_view(quoter), data, QUOTE_GAS_LIMIT);
		ensure!(
			matches!(result.exit_reason, Succeed(ExitSucceed::Returned)),
			ExecutorError::Error("uniswapv3: quote failed".into())
		);
		ensure!(
			result.value.len() >= 32,
			ExecutorError::Error("uniswapv3: quote returned no data".into())
		);
		let amount_out = U256::from_big_endian(&result.value[0..32]);
		Ok(amount_out.saturated_into::<u128>())
	}

	pub fn quote_in_given_out(
		asset_in: AssetId,
		asset_out: AssetId,
		fee: u32,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<DispatchError>> {
		let quoter = Self::quoter()?;
		let token_in = evm_token_address(asset_in);
		let token_out = evm_token_address(asset_out);
		let data = EvmDataWriter::new_with_selector(Function::QuoteExactOutputSingle)
			.write(token_in)
			.write(token_out)
			.write(U256::from(amount_out))
			.write(U256::from(fee))
			.write(U256::zero())
			.build();
		let result = Executor::<T>::view(CallContext::new_view(quoter), data, QUOTE_GAS_LIMIT);
		ensure!(
			matches!(result.exit_reason, Succeed(ExitSucceed::Returned)),
			ExecutorError::Error("uniswapv3: quote failed".into())
		);
		ensure!(
			result.value.len() >= 32,
			ExecutorError::Error("uniswapv3: quote returned no data".into())
		);
		let amount_in = U256::from_big_endian(&result.value[0..32]).saturated_into::<u128>();
		Ok(amount_in.saturating_add(IN_GIVEN_OUT_ROUNDING))
	}

	pub fn spot_price_with_fee(
		asset_a: AssetId,
		asset_b: AssetId,
		fee: u32,
	) -> Result<FixedU128, ExecutorError<DispatchError>> {
		let factory = Self::factory()?;
		let pool = Self::pool_address(factory, asset_a, asset_b, fee)?
			.ok_or(ExecutorError::Error("uniswapv3: pool not found".into()))?;
		let data = EvmDataWriter::new_with_selector(Function::Slot0).build();
		let result = Executor::<T>::view(CallContext::new_view(pool), data, VIEW_GAS_LIMIT);
		ensure!(
			matches!(result.exit_reason, Succeed(ExitSucceed::Returned)),
			ExecutorError::Error("uniswapv3: slot0 failed".into())
		);
		ensure!(
			result.value.len() >= 32,
			ExecutorError::Error("uniswapv3: slot0 returned no data".into())
		);
		let sqrt_price_x96 = U256::from_big_endian(&result.value[0..32]);
		let price = price_token1_per_token0(sqrt_price_x96);
		let raw = if evm_token_address(asset_a) < evm_token_address(asset_b) {
			price
				.reciprocal()
				.ok_or(ExecutorError::Error("uniswapv3: zero price".into()))?
		} else {
			price
		};
		let fee_factor = FixedU128::from_inner(
			FEE_DENOMINATOR
				.saturating_sub(fee as u128)
				.saturating_mul(1_000_000_000_000u128),
		);
		Ok(raw.saturating_mul(fee_factor))
	}

	pub fn liquidity_depth(
		asset_a: AssetId,
		asset_b: AssetId,
		fee: u32,
	) -> Result<Balance, ExecutorError<DispatchError>> {
		let factory = Self::factory()?;
		let pool = Self::pool_address(factory, asset_a, asset_b, fee)?
			.ok_or(ExecutorError::Error("uniswapv3: pool not found".into()))?;
		let token_a = evm_token_address(asset_a);
		let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
			.write(pool)
			.build();
		let result = Executor::<T>::view(CallContext::new_view(token_a), data, VIEW_GAS_LIMIT);
		ensure!(
			matches!(result.exit_reason, Succeed(ExitSucceed::Returned)),
			ExecutorError::Error("uniswapv3: balanceOf failed".into())
		);
		ensure!(
			result.value.len() >= 32,
			ExecutorError::Error("uniswapv3: balanceOf returned no data".into())
		);
		Ok(U256::from_big_endian(&result.value[0..32]).saturated_into::<u128>())
	}

	pub fn find_pool(
		asset_a: AssetId,
		asset_b: AssetId,
		fee: u32,
	) -> Result<Option<EvmAddress>, ExecutorError<DispatchError>> {
		let factory = Self::factory()?;
		Self::pool_address(factory, asset_a, asset_b, fee)
	}

	pub fn trade_weight() -> Weight {
		<T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(TRADE_GAS_LIMIT + QUOTE_GAS_LIMIT, true)
	}

	fn swap_router() -> Result<EvmAddress, ExecutorError<DispatchError>> {
		pallet_parameters::Pallet::<T>::uniswap_v3_swap_router()
			.ok_or(ExecutorError::Error("uniswapv3: swap router not configured".into()))
	}

	fn do_sell(
		who: OriginFor<T>,
		asset_in: AssetId,
		asset_out: AssetId,
		fee: u32,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<Balance, ExecutorError<DispatchError>> {
		let who_account =
			ensure_signed(who.clone()).map_err(|_| ExecutorError::Error("uniswapv3: bad origin".into()))?;
		let _ = pallet_evm_accounts::Pallet::<T>::bind_evm_address(who);
		let trader = pallet_evm_accounts::Pallet::<T>::evm_address(&who_account);
		let router = Self::swap_router()?;
		let token_in = evm_token_address(asset_in);
		let token_out = evm_token_address(asset_out);

		let approve = EvmDataWriter::new_with_selector(Function::Approve)
			.write(router)
			.write(U256::from(amount_in))
			.build();
		let approve_result = Executor::<T>::call(
			CallContext::new_call(token_in, trader),
			approve,
			U256::zero(),
			TRADE_GAS_LIMIT,
		);
		ensure!(
			matches!(approve_result.exit_reason, Succeed(_)),
			ExecutorError::Error("uniswapv3: approve failed".into())
		);

		let swap = EvmDataWriter::new_with_selector(Function::ExactInputSingle)
			.write(token_in)
			.write(token_out)
			.write(U256::from(fee))
			.write(trader)
			.write(U256::from(amount_in))
			.write(U256::from(min_limit))
			.write(U256::zero())
			.build();
		let swap_result = Executor::<T>::call(
			CallContext::new_call(router, trader),
			swap,
			U256::zero(),
			TRADE_GAS_LIMIT,
		);
		ensure!(
			matches!(swap_result.exit_reason, Succeed(_)),
			ExecutorError::Error("uniswapv3: swap failed".into())
		);

		let amount_out = if swap_result.value.len() >= 32 {
			U256::from_big_endian(&swap_result.value[0..32]).saturated_into::<u128>()
		} else {
			min_limit
		};
		Ok(amount_out)
	}

	fn do_buy(
		who: OriginFor<T>,
		asset_in: AssetId,
		asset_out: AssetId,
		fee: u32,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<Balance, ExecutorError<DispatchError>> {
		let who_account =
			ensure_signed(who.clone()).map_err(|_| ExecutorError::Error("uniswapv3: bad origin".into()))?;
		let _ = pallet_evm_accounts::Pallet::<T>::bind_evm_address(who);
		let trader = pallet_evm_accounts::Pallet::<T>::evm_address(&who_account);
		let router = Self::swap_router()?;
		let token_in = evm_token_address(asset_in);
		let token_out = evm_token_address(asset_out);

		let approve = EvmDataWriter::new_with_selector(Function::Approve)
			.write(router)
			.write(U256::from(max_limit))
			.build();
		let approve_result = Executor::<T>::call(
			CallContext::new_call(token_in, trader),
			approve,
			U256::zero(),
			TRADE_GAS_LIMIT,
		);
		ensure!(
			matches!(approve_result.exit_reason, Succeed(_)),
			ExecutorError::Error("uniswapv3: approve failed".into())
		);

		let swap = EvmDataWriter::new_with_selector(Function::ExactOutputSingle)
			.write(token_in)
			.write(token_out)
			.write(U256::from(fee))
			.write(trader)
			.write(U256::from(amount_out))
			.write(U256::from(max_limit))
			.write(U256::zero())
			.build();
		let swap_result = Executor::<T>::call(
			CallContext::new_call(router, trader),
			swap,
			U256::zero(),
			TRADE_GAS_LIMIT,
		);
		ensure!(
			matches!(swap_result.exit_reason, Succeed(_)),
			ExecutorError::Error("uniswapv3: swap failed".into())
		);

		let amount_in = if swap_result.value.len() >= 32 {
			U256::from_big_endian(&swap_result.value[0..32]).saturated_into::<u128>()
		} else {
			max_limit
		};
		Ok(amount_in)
	}
}

impl<T> TradeExecution<OriginFor<T>, AccountId, AssetId, Balance> for UniswapV3TradeExecutor<T>
where
	T: frame_system::Config
		+ pallet_evm::Config
		+ pallet_dispatcher::Config
		+ pallet_parameters::Config
		+ pallet_evm_accounts::Config
		+ pallet_broadcast::Config,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + frame_support::traits::IsType<sp_runtime::AccountId32>,
	BalanceOf<T>: TryFrom<U256> + Into<U256> + Default,
	NonceIdOf<T>: Into<T::Nonce>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
{
	type Error = DispatchError;

	fn calculate_out_given_in(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		Self::quote_out_given_in(asset_in, asset_out, fee, amount_in)
	}

	fn calculate_in_given_out(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		Self::quote_in_given_out(asset_in, asset_out, fee, amount_out)
	}

	fn execute_sell(
		who: OriginFor<T>,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		let amount_out = Self::do_sell(who.clone(), asset_in, asset_out, fee, amount_in, min_limit)?;
		let trader = ensure_signed(who).map_err(|_| ExecutorError::Error("uniswapv3: bad origin".into()))?;
		let filler = pallet_evm_accounts::Pallet::<T>::truncated_account_id(Self::swap_router().unwrap_or_default());
		pallet_broadcast::Pallet::<T>::deposit_trade_event(
			trader,
			filler,
			pallet_broadcast::types::Filler::UniswapV3,
			pallet_broadcast::types::TradeOperation::ExactIn,
			vec![Asset::new(asset_in, amount_in)],
			vec![Asset::new(asset_out, amount_out)],
			vec![],
		);
		Ok(())
	}

	fn execute_buy(
		who: OriginFor<T>,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		let amount_in = Self::do_buy(who.clone(), asset_in, asset_out, fee, amount_out, max_limit)?;
		let trader = ensure_signed(who).map_err(|_| ExecutorError::Error("uniswapv3: bad origin".into()))?;
		let filler = pallet_evm_accounts::Pallet::<T>::truncated_account_id(Self::swap_router().unwrap_or_default());
		pallet_broadcast::Pallet::<T>::deposit_trade_event(
			trader,
			filler,
			pallet_broadcast::types::Filler::UniswapV3,
			pallet_broadcast::types::TradeOperation::ExactOut,
			vec![Asset::new(asset_in, amount_in)],
			vec![Asset::new(asset_out, amount_out)],
			vec![],
		);
		Ok(())
	}

	fn get_liquidity_depth(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		Self::liquidity_depth(asset_a, asset_b, fee)
	}

	fn calculate_spot_price_with_fee(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		let PoolType::UniswapV3(fee) = pool_type else {
			return Err(ExecutorError::NotSupported);
		};
		Self::spot_price_with_fee(asset_a, asset_b, fee)
	}
}

pub mod runtime_api {
	use super::AssetId;
	use super::EvmAddress;
	use codec::Codec;
	use sp_runtime::traits::MaybeDisplay;

	sp_api::decl_runtime_apis! {
		pub trait UniswapV3Api<Balance>
		  where Balance: Codec + MaybeDisplay
		{
			fn pool(asset_a: AssetId, asset_b: AssetId, fee: u32) -> Option<EvmAddress>;
			fn quote_sell(asset_in: AssetId, asset_out: AssetId, fee: u32, amount_in: Balance) -> Option<Balance>;
			fn quote_buy(asset_in: AssetId, asset_out: AssetId, fee: u32, amount_out: Balance) -> Option<Balance>;
			fn liquidity_depth(asset_in: AssetId, asset_out: AssetId, fee: u32) -> Option<Balance>;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn evm_token_address_should_map_registry_asset_to_precompile() {
		sp_io::TestExternalities::default().execute_with(|| {
			assert_eq!(evm_token_address(2), EvmAddress::from_low_u64_be(0x1_0000_0002));
		});
	}

	#[test]
	fn sort_tokens_should_order_ascending() {
		let lo = EvmAddress::from_low_u64_be(0x1_0000_0001);
		let hi = EvmAddress::from_low_u64_be(0x1_0000_0002);
		assert_eq!(sort_tokens(hi, lo), (lo, hi));
		assert_eq!(sort_tokens(lo, hi), (lo, hi));
	}

	#[test]
	fn price_at_sqrt_two_pow_96_should_be_one() {
		assert_eq!(price_token1_per_token0(U256::from(1) << 96), FixedU128::from(1));
	}

	#[test]
	fn price_at_sqrt_two_pow_97_should_be_four() {
		assert_eq!(price_token1_per_token0(U256::from(1) << 97), FixedU128::from(4));
	}

	#[test]
	fn price_at_sqrt_two_pow_95_should_be_one_quarter() {
		assert_eq!(
			price_token1_per_token0(U256::from(1) << 95),
			FixedU128::from_rational(1, 4)
		);
	}

	#[test]
	fn price_should_saturate_when_sqrt_price_overflows() {
		assert_eq!(price_token1_per_token0(U256::MAX), FixedU128::from_inner(u128::MAX));
	}

	#[test]
	fn sort_tokens_should_return_same_pair_when_equal() {
		let a = EvmAddress::from_low_u64_be(0x1_0000_0007);
		assert_eq!(sort_tokens(a, a), (a, a));
	}
}
