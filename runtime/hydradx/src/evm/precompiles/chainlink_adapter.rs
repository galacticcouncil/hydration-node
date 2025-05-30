use crate::{
	assets::LRNA,
	evm::precompiles::{
		handle::{FunctionModifier, PrecompileHandleExt},
		substrate::RuntimeHelper,
		succeed, Output,
	},
	evm::EvmAddress,
	EmaOracle, Router,
};
use codec::{Decode, Encode, EncodeLike};
use frame_support::traits::{IsType, OriginTrait};
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use hydra_dx_math::support::rational::{round_to_rational, Rounding};
use hydradx_adapters::OraclePriceProvider;
use hydradx_traits::{
	oracle::PriceOracle,
	router::{AssetPair, RouteProvider},
	AggregatedPriceOracle, Inspect, OraclePeriod, Source,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_ema_oracle::Price;
use pallet_evm::{ExitRevert, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult};
use primitive_types::{H160, U128, U256};
use primitives::{constants::chain::OMNIPOOL_SOURCE, AssetId};
use sp_runtime::{traits::Dispatchable, RuntimeDebug};
use sp_std::{cmp::Ordering, marker::PhantomData};

const EMPTY_SOURCE: Source = [0u8; 8];

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AggregatorInterface {
	LatestAnswer = "latestAnswer()",
	LatestTimestamp = "latestTimestamp()",
	LatestRound = "latestRound()",
	GetAnswer = "getAnswer(uint256)",
	GetTimestamp = "getTimestamp(uint256)",
	Decimals = "decimals()",
}

pub struct ChainlinkOraclePrecompile<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for ChainlinkOraclePrecompile<Runtime>
where
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_ema_oracle::Config
		+ pallet_route_executor::Config,
	EmaOracle: AggregatedPriceOracle<AssetId, BlockNumberFor<Runtime>, Price>,
	Router: RouteProvider<AssetId>,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let address = handle.code_address();
		if let Some((asset_id_a, asset_id_b, period, source)) = decode_oracle_address(address) {
			log::debug!(target: "evm", "chainlink: asset_id_a: {:?}, asset_id_b: {:?}, period: {:?}, source: {:?}", asset_id_a, asset_id_b, period, source);

			let selector = match handle.read_selector() {
				Ok(selector) => selector,
				Err(e) => return Err(e),
			};

			handle.check_function_modifier(FunctionModifier::View)?;

			return match selector {
				AggregatorInterface::GetAnswer | AggregatorInterface::LatestAnswer => {
					Self::get_oracle_entry(asset_id_a, asset_id_b, period, source, handle)
				}
				AggregatorInterface::Decimals => Ok(succeed(Output::encode_uint::<u8>(8_u8))),
				_ => Self::not_supported(),
			};
		}
		Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid price oracle data".into(),
		})
	}
}

impl<Runtime> ChainlinkOraclePrecompile<Runtime>
where
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_ema_oracle::Config
		+ pallet_route_executor::Config,
	EmaOracle: AggregatedPriceOracle<AssetId, BlockNumberFor<Runtime>, Price>,
	Router: RouteProvider<AssetId>,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
{
	/// If `source` is empty, the route is obtained from the Router pallet and final price calculated by multiplication.
	/// Oracle prices for omnipool are quoted by LRNA, so in the case that the Omnipool is specified as a source,
	/// two prices (one for Asset_A/LRNA and second one for Asset_B/LRNA) are fetched and one final price is calculated from them.
	/// Returned price has 8 decimals.
	fn get_oracle_entry(
		asset_id_a: AssetId,
		asset_id_b: AssetId,
		period: OraclePeriod,
		source: Source,
		handle: &mut impl PrecompileHandle,
	) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		// In case of empty source, we retrieve onchain route
		let price = if source == EMPTY_SOURCE {
			let route = Router::get_route(AssetPair {
				asset_in: asset_id_a,
				asset_out: asset_id_b,
			});
			let price = OraclePriceProvider::<AssetId, EmaOracle, LRNA>::price(&route, period).ok_or(
				PrecompileFailure::Error {
					exit_status: pallet_evm::ExitError::Other("Price not available".into()),
				},
			)?;

			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id_a, asset_id_b, price, source);
			price
		}
		// special case: all Omnipool prices are quoted with LRNA asset
		else if source == OMNIPOOL_SOURCE {
			let (asset_a_price, _block_number) =
				<pallet_ema_oracle::Pallet<Runtime>>::get_price(asset_id_a, LRNA::get(), period, source).map_err(
					|_| PrecompileFailure::Error {
						exit_status: pallet_evm::ExitError::Other("Price not available".into()),
					},
				)?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id_a, LRNA::get(), asset_a_price, source);

			let (asset_b_price, _block_number) =
				<pallet_ema_oracle::Pallet<Runtime>>::get_price(asset_id_b, LRNA::get(), period, source).map_err(
					|_| PrecompileFailure::Error {
						exit_status: pallet_evm::ExitError::Other("Price not available".into()),
					},
				)?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", LRNA::get(), asset_id_b, asset_b_price, source);

			let nominator = U128::full_mul(asset_a_price.n.into(), asset_b_price.d.into());
			let denominator = U128::full_mul(asset_a_price.d.into(), asset_b_price.n.into());

			let rat_as_u128 = round_to_rational((nominator, denominator), Rounding::Nearest);

			Price::from(rat_as_u128)
		} else {
			let (price, _block_number) = <pallet_ema_oracle::Pallet<Runtime>>::get_price(
				asset_id_a, asset_id_b, period, source,
			)
			.map_err(|_| PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Price not available".into()),
			})?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id_a, asset_id_b, price, source);
			price
		};

		let asset_a_decimals =
			<pallet_asset_registry::Pallet<Runtime>>::decimals(asset_id_a.into()).ok_or(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Decimals not available".into()),
			})?;
		let asset_b_decimals =
			<pallet_asset_registry::Pallet<Runtime>>::decimals(asset_id_b.into()).ok_or(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Decimals not available".into()),
			})?;

		let decimals_diff = U128::from(asset_a_decimals.abs_diff(asset_b_decimals));
		let decimals_adjustment = U128::from(10u128)
			.checked_pow(decimals_diff)
			.ok_or(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Price conversion failed".into()),
			})?;

		let price = match asset_a_decimals.cmp(&asset_b_decimals) {
			Ordering::Greater => {
				let nominator = U256::from(price.n);
				let denominator = U128::full_mul(price.d.into(), decimals_adjustment);

				round_to_rational((nominator, denominator), Rounding::Nearest).into()
			}
			Ordering::Less => {
				let nominator = U128::full_mul(price.n.into(), decimals_adjustment);
				let denominator = U256::from(price.d);

				round_to_rational((nominator, denominator), Rounding::Nearest).into()
			}
			Ordering::Equal => price,
		};

		// return value should be int256, but the price is always a positive number so we can use uint256
		let price_u256 = convert_price_to_u256(price)?;
		let encoded = Output::encode_uint::<U256>(price_u256);

		Ok(succeed(encoded))
	}

	fn not_supported() -> PrecompileResult {
		Err(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("not supported".into()),
		})
	}
}

pub fn is_oracle_address(address: H160) -> bool {
	let oracle_address_prefix = &(H160::from(hex!("0000010000000000000000000000000000000000"))[0..3]);

	&address.to_fixed_bytes()[0..3] == oracle_address_prefix
}

/// Converts pallet_ema_oracle::Price to U256. The price is stored as one integer: integer part + fractional part.
/// The fractional part contains 8 decimals.
/// E.g. 7.1234 is stored as 712_340_000.
fn convert_price_to_u256(price: Price) -> Result<U256, PrecompileFailure> {
	U128::from(100_000_000) // 8 decimals
		.full_mul(price.n.into())
		.checked_div(price.d.into())
		.ok_or(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("Price conversion failed".into()),
		})
}

#[test]
fn normalize_price_to_u256_should_work() {
	// price = 111_222_333_444.555
	let price = Price {
		n: 111_222_333_444_555u128,
		d: 1_000u128,
	};
	let price_u256 = convert_price_to_u256(price).unwrap();
	assert_eq!(price_u256, 11_122_233_344_455_500_000u128.into());

	// price = 111_222_333.111222333
	let price = Price {
		n: 111_222_333_111_222_333u128,
		d: 1_000_000_000u128,
	};
	let price_u256 = convert_price_to_u256(price).unwrap();
	assert_eq!(price_u256, 11_122_233_311_122_233u128.into());

	// price = 0.1234
	let price = Price {
		n: 1_234u128,
		d: 10_000u128,
	};
	let price_u256 = convert_price_to_u256(price).unwrap();
	assert_eq!(price_u256, 12_340_000u128.into());

	// price = 0.000001234
	let price = Price {
		n: 1_234u128,
		d: 1_000_000_000u128,
	};
	let price_u256 = convert_price_to_u256(price).unwrap();
	assert_eq!(price_u256, 123u128.into());
}
/// Encoding is 3 bytes for precompile prefix 0x000001,
/// followed by 1 byte for encoded OraclePeriod enum, 8 bytes for Source, and two times 4 bytes for AssetId.
pub fn encode_oracle_address(
	asset_id_a: AssetId,
	asset_id_b: AssetId,
	period: OraclePeriod,
	source: Source,
) -> EvmAddress {
	let mut oracle_address_bytes = [0u8; 20];

	let period_u32 = period.encode();

	oracle_address_bytes[2] = 1;

	oracle_address_bytes[3] = period_u32[0];

	oracle_address_bytes[4..(4 + source.len())].copy_from_slice(&source[..]);

	let asset_id_bytes: [u8; 4] = asset_id_a.to_be_bytes();
	oracle_address_bytes[12..(12 + asset_id_bytes.len())].copy_from_slice(&asset_id_bytes[..]);

	let asset_id_bytes: [u8; 4] = asset_id_b.to_be_bytes();
	oracle_address_bytes[16..(16 + asset_id_bytes.len())].copy_from_slice(&asset_id_bytes[..]);

	EvmAddress::from(oracle_address_bytes)
}

pub fn decode_oracle_address(oracle_address: EvmAddress) -> Option<(AssetId, AssetId, OraclePeriod, Source)> {
	if !is_oracle_address(oracle_address) {
		return None;
	}

	let oracle_address_bytes = oracle_address.to_fixed_bytes();

	let mut asset_id_a: u32 = 0;
	for byte in oracle_address_bytes[12..16].iter() {
		asset_id_a = (asset_id_a << 8) | (*byte as u32);
	}

	let mut asset_id_b: u32 = 0;
	for byte in oracle_address_bytes[16..20].iter() {
		asset_id_b = (asset_id_b << 8) | (*byte as u32);
	}

	let mut source: Source = EMPTY_SOURCE;
	source.copy_from_slice(&oracle_address_bytes[4..12]);

	let period_u32 = oracle_address_bytes[3];
	match Decode::decode(&mut &[period_u32; 1][..]) {
		Ok(period) => Some((asset_id_a, asset_id_b, period, source)),
		_ => None,
	}
}

/// Runtime API definition for the Chainlink adapter.
pub mod runtime_api {
	#![cfg_attr(not(feature = "std"), no_std)]

	use super::{AssetId, OraclePeriod, Source};
	use codec::Codec;

	sp_api::decl_runtime_apis! {
		/// The API to query EVM account conversions.
		pub trait ChainlinkAdapterApi<AccountId, EvmAddress> where
			AccountId: Codec,
			EvmAddress: Codec,
		{
			fn encode_oracle_address(asset_id_a: AssetId, asset_id_b: AssetId, period: OraclePeriod, source: Source) -> EvmAddress;
			fn decode_oracle_address(oracle_address: EvmAddress) -> Option<(AssetId, AssetId, OraclePeriod, Source)>;
		}
	}
}

#[test]
fn encoded_oracle_period_is_one_byte() {
	use codec::MaxEncodedLen;
	// OraclePeriod is enum encoded as Vec<u8>. We don't expect it to be more than 1 byte.
	assert_eq!(OraclePeriod::max_encoded_len(), 1);
}

#[test]
fn encode_oracle_address_should_work() {
	assert_eq!(
		encode_oracle_address(4, 5, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE),
		H160::from(hex!("000001026f6d6e69706f6f6c0000000400000005"))
	);
}

#[test]
fn decode_oracle_address_should_work() {
	assert_eq!(
		decode_oracle_address(H160::from(hex!("000001026f6d6e69706f6f6c0000000400000005"))),
		Some((4, 5, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE))
	);
}
