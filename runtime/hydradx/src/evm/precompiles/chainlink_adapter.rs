use crate::{
	Currencies, EmaOracle, Router,
	assets::LRNA,
	evm::EvmAddress,
	evm::precompiles::{
		handle::{FunctionModifier, PrecompileHandleExt},
		substrate::RuntimeHelper,
		succeed, Output,
	},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_runtime::{
	RuntimeDebug,
	traits::{Dispatchable, Get},
};
use codec::{Encode, Decode, EncodeLike};
use frame_support::traits::{IsType, OriginTrait};
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use hydra_dx_math::support::rational::{round_u512_to_rational, Rounding};
use hydradx_adapters::OraclePriceProvider;
use hydradx_traits::{
	AggregatedPriceOracle, Inspect, OraclePeriod, Source,
	oracle::PriceOracle,
	router::{AssetPair, RouteProvider},
};
use orml_traits::MultiCurrency;
use pallet_ema_oracle::Price;
use pallet_evm::{ExitRevert, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult};
use primitive_types::{H160, U256, U512};
use primitives::{
	AssetId, Balance,
	constants::chain::OMNIPOOL_SOURCE,
};
use sp_std::marker::PhantomData;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum AggregatorInterface {
	LatestAnswer = "latestAnswer()",
	LatestTimestamp = "latestTimestamp()",
	LatestRound = "latestRound()",
	GetAnswer = "getAnswer(uint256)",
	GetTimestamp = "getTimestamp(uint256)",
}

pub struct ChainlinkOraclePrecompile<QuoteAsset, Runtime>(PhantomData<(QuoteAsset, Runtime)>);

impl<QuoteAsset, Runtime> Precompile for ChainlinkOraclePrecompile<QuoteAsset, Runtime>
where
	QuoteAsset: Get<AssetId>,
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config
		+ pallet_evm_accounts::Config
		+ pallet_ema_oracle::Config
		+ pallet_route_executor::Config,
	EmaOracle: AggregatedPriceOracle<AssetId, BlockNumberFor<Runtime>, Price>,
	Router: RouteProvider<AssetId>,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let address = handle.code_address();
		if let Some((asset_id, period, source)) = decode_evm_address(address) {
			log::debug!(target: "evm", "chainlink: asset_id: {:?}, period: {:?}, source: {:?}", asset_id, period, source);

			let selector = match handle.read_selector() {
				Ok(selector) => selector,
				Err(e) => return Err(e),
			};

			handle.check_function_modifier(FunctionModifier::View)?;

			return match selector {
				AggregatorInterface::GetAnswer => Self::get_oracle_entry(asset_id, period, source, handle),
				_ => Self::not_supported(),
			};
		}
		Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid price oracle data".into(),
		})
	}
}

impl<QuoteAsset, Runtime> ChainlinkOraclePrecompile<QuoteAsset, Runtime>
where
	QuoteAsset: Get<AssetId>,
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config
		+ pallet_evm_accounts::Config
		+ pallet_ema_oracle::Config
		+ pallet_route_executor::Config,
	EmaOracle: AggregatedPriceOracle<AssetId, BlockNumberFor<Runtime>, Price>,
	Router: RouteProvider<AssetId>,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
{
	/// Returned price is always quoted by `QuoteAsset`.
	/// If `source` is empty, the route is obtained from the Router pallet and final price calculated by multiplication.
	/// Oracle prices for omnipool are quoted by LRNA, so in the case that the Omnipool is specified as a source,
	/// two prices (one for ASSET/LRNA and second one for QuoteAsset/LRNA) are fetched and one final price is calculated from them.
	fn get_oracle_entry(
		asset_id: AssetId,
		period: OraclePeriod,
		source: Source,
		handle: &mut impl PrecompileHandle,
	) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		let decimals =
			<pallet_asset_registry::Pallet<Runtime>>::decimals(asset_id.into()).ok_or(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Decimals not available".into()),
			})?;

		// use Router to get a route
		let price = if source == [0; 8] {
			let route = Router::get_route(AssetPair {
				asset_in: asset_id,
				asset_out: QuoteAsset::get(),
			});
			let price = OraclePriceProvider::<AssetId, EmaOracle, LRNA>::price(&route, period).ok_or(
				PrecompileFailure::Error {
					exit_status: pallet_evm::ExitError::Other("Price not available".into()),
				},
			)?;

			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id, QuoteAsset::get(), price, source);
			price
		}
		// special case: all Omnipool prices are quoted with LRNA asset
		else if source == OMNIPOOL_SOURCE {
			let (asset_a_price, _block_number) =
				<pallet_ema_oracle::Pallet<Runtime>>::get_price(asset_id, LRNA::get(), period, source).map_err(
					|_| PrecompileFailure::Error {
						exit_status: pallet_evm::ExitError::Other("Price not available".into()),
					},
				)?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id, LRNA::get(), asset_a_price, source);

			let (asset_b_price, _block_number) =
				<pallet_ema_oracle::Pallet<Runtime>>::get_price(QuoteAsset::get(), LRNA::get(), period, source)
					.map_err(|_| PrecompileFailure::Error {
						exit_status: pallet_evm::ExitError::Other("Price not available".into()),
					})?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", LRNA::get(), QuoteAsset::get(), asset_b_price, source);

			let nominator = U512::from(asset_a_price.n)
				.checked_mul(U512::from(asset_b_price.d))
				.ok_or(PrecompileFailure::Error {
					exit_status: pallet_evm::ExitError::Other("Price conversion failed.".into()),
				})?;
			let denominator = U512::from(asset_a_price.d)
				.checked_mul(U512::from(asset_b_price.n))
				.ok_or(PrecompileFailure::Error {
					exit_status: pallet_evm::ExitError::Other("Price conversion failed.".into()),
				})?;

			let rat_as_u128 = round_u512_to_rational((nominator, denominator), Rounding::Nearest);

			Price::from(rat_as_u128)
		} else {
			let (price, _block_number) =
				<pallet_ema_oracle::Pallet<Runtime>>::get_price(asset_id, QuoteAsset::get(), period, source).map_err(
					|_| PrecompileFailure::Error {
						exit_status: pallet_evm::ExitError::Other("Price not available".into()),
					},
				)?;
			log::debug!(target: "evm", "chainlink: base asset: {:?}, quote asset: {:?}, price: {:?}, source {:?}", asset_id, QuoteAsset::get(), price, source);
			price
		};

		// return value should be int256, but the price is always a positive number so we can use uint256
		let price_u256 = convert_price_to_u256(price, decimals)?;
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
	let oracle_address_prefix = &(H160::from(hex!("0000000000000100000000000000000000000000"))[0..7]);

	&address.to_fixed_bytes()[0..7] == oracle_address_prefix
}

/// Converts pallet_ema_oracle::Price to U256. The price is stored as integer, integer part + fractional part.
/// The fractional part contains `decimals` number of decimal places.
/// E.g. 123.456789 is stored as 123456 if three decimals are used.
fn convert_price_to_u256(price: Price, decimals: u8) -> Result<U256, PrecompileFailure> {
	// avoid panic in exponentiation. Max 256bit number has 78 digits.
	if decimals > 70 {
		return Err(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("Too many decimals".into()),
		});
	};

	U256::exp10(decimals.into())
		.checked_mul(price.n.into())
		.ok_or(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("Price conversion failed.".into()),
		})?
		.checked_div(price.d.into())
		.ok_or(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("Price conversion failed.".into()),
		})
}

/// Encoding is 7 bytes for precompile prefix 0x00000000000001,
/// followed by 1 byte for encoded OraclePeriod enum, 8 bytes for Source, and 4 bytes for AssetId.
pub fn encode_evm_address(asset_id: AssetId, period: OraclePeriod, source: Source) -> Option<EvmAddress> {
	let mut evm_address_bytes = [0u8; 20];

	let period_u32 = period.encode();
	// OraclePeriod is enum ancoded as Vec<u8>.
	if period_u32.len() > 1 {
		return None;
	}

	evm_address_bytes[6] = 1;

	evm_address_bytes[7] = period_u32[0];

	evm_address_bytes[8..(8 + source.len())].copy_from_slice(&source[..]);

	let asset_id_bytes: [u8; 4] = asset_id.to_be_bytes();
	evm_address_bytes[16..(16 + asset_id_bytes.len())].copy_from_slice(&asset_id_bytes[..]);

	Some(EvmAddress::from(evm_address_bytes))
}

pub fn decode_evm_address(evm_address: EvmAddress) -> Option<(AssetId, OraclePeriod, Source)> {
	if !is_oracle_address(evm_address) {
		return None;
	}

	let evm_address_bytes = evm_address.to_fixed_bytes();

	let mut asset_id: u32 = 0;
	for byte in evm_address_bytes[16..20].iter() {
		asset_id = (asset_id << 8) | (*byte as u32);
	}

	let mut source: Source = [0; 8];
	source.copy_from_slice(&evm_address_bytes[8..16]);

	let period_u32 = evm_address_bytes[7];
	let period: OraclePeriod = Decode::decode(&mut &[period_u32; 1][..]).unwrap();

	Some((asset_id, period, source))
}

#[test]
fn encode_evm_address_should_work() {
	assert_eq!(
		encode_evm_address(4, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE),
		Some(H160::from(hex!("00000000000001026f6d6e69706f6f6c00000004")))
	);
}

#[test]
fn decode_evm_address_should_work() {
	assert_eq!(
		decode_evm_address(H160::from(hex!("00000000000001026f6d6e69706f6f6c00000004"))),
		Some((4, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE))
	);
}
