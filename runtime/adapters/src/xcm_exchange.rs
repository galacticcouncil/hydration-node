use hydradx_traits::router::{AssetPair, RouteProvider};
use orml_traits::MultiCurrency;
use pallet_broadcast::types::ExecutionType;
use polkadot_xcm::v4::prelude::*;
use sp_core::Get;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::BoundedVec;
use sp_std::marker::PhantomData;
use xcm_executor::traits::AssetExchange;
use xcm_executor::AssetsInHolding;
use pallet_circuit_breaker::fuses::issuance::IssuanceIncreaseFuse;

/// Implements `AssetExchange` to support the `ExchangeAsset` XCM instruction.
///
/// Uses pallet-route-executor to execute trades.
///
/// Will map exchange instructions with `maximal = true` to sell (selling all of `give` asset) and `false` to buy
/// (buying exactly `want` amount of asset).
///
/// NOTE: Currenty limited to one asset each for `give` and `want`.
pub struct XcmAssetExchanger<Runtime, TempAccount, CurrencyIdConvert, Currency>(
	PhantomData<(Runtime, TempAccount, CurrencyIdConvert, Currency)>,
);

impl<Runtime, TempAccount, CurrencyIdConvert, Currency> AssetExchange
	for XcmAssetExchanger<Runtime, TempAccount, CurrencyIdConvert, Currency>
where
	Runtime: pallet_route_executor::Config + pallet_circuit_breaker::Config,
	TempAccount: Get<Runtime::AccountId>,
	CurrencyIdConvert: Convert<Asset, Option<<Runtime as pallet_route_executor::Config>::AssetId>>,
	Currency: MultiCurrency<Runtime::AccountId, CurrencyId = <Runtime as pallet_route_executor::Config>::AssetId, Balance = <Runtime as pallet_route_executor::Config>::Balance>,
	<Runtime as pallet_route_executor::Config>::Balance: From<u128> + Zero + Into<u128>,
	<Runtime as pallet_route_executor::Config>::AssetId: Into<u32>,
	<Runtime as pallet_route_executor::Config>::AssetId: Into<<Runtime as pallet_circuit_breaker::Config>::AssetId>,
{
	fn exchange_asset(
		_origin: Option<&Location>,
		give: AssetsInHolding,
		want: &Assets,
		maximal: bool,
	) -> Result<AssetsInHolding, AssetsInHolding> {
		use orml_utilities::with_transaction_result;

		let account = TempAccount::get();
		let origin = Runtime::RuntimeOrigin::from(frame_system::RawOrigin::Signed(account.clone()));

		if give.len() != 1 {
			log::warn!(target: "xcm::exchange-asset", "Only one give asset is supported.");
			return Err(give);
		};

		//We assume only one asset wanted as translating into buy and sell is ambigous for multiple want assets
		if want.len() != 1 {
			log::warn!(target: "xcm::exchange-asset", "Only one want asset is supported.");
			return Err(give);
		};
		let Some(given) = give.fungible_assets_iter().next() else {
			return Err(give);
		};

		let Some(asset_in) = CurrencyIdConvert::convert(given.clone()) else {
			return Err(give);
		};
		let Some(wanted) = want.get(0) else { return Err(give) };
		let Some(asset_out) = CurrencyIdConvert::convert(wanted.clone()) else {
			return Err(give);
		};
		let use_onchain_route = BoundedVec::new();

		if pallet_broadcast::Pallet::<Runtime>::add_to_context(ExecutionType::XcmExchange).is_err() {
			log::error!(target: "xcm::exchange-asset", "Failed to add to context.");
			return Err(give);
		};

		let trade_result = if maximal {
			// sell
			let Fungible(amount) = given.fun else { return Err(give) };
			let Fungible(min_buy_amount) = wanted.fun else {
				return Err(give);
			};

			if !IssuanceIncreaseFuse::<Runtime>::can_mint(asset_in.into(), amount.into()) {
				log::warn!(target: "xcm::exchange-asset", "Circuit breaker triggered for asset {:?}. Asset will be trapped.", asset_in);
				return Err(give);
			}

			with_transaction_result(|| {
				Currency::deposit(asset_in, &account, amount.into())?; // mint the incoming tokens
				pallet_route_executor::Pallet::<Runtime>::sell(
					origin,
					asset_in,
					asset_out,
					amount.into(),
					min_buy_amount.into(),
					use_onchain_route,
				)?;
				debug_assert!(
					Currency::free_balance(asset_in, &account) == <Runtime as pallet_route_executor::Config>::Balance::zero(),
					"Sell should not leave any of the incoming asset."
				);
				let amount_received = Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received >= min_buy_amount.into(),
					"Sell should return more than mininum buy amount."
				);
				Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				let holding: Asset = (wanted.id.clone(), amount_received.into()).into();

				Ok(holding.into())
			})
			.map_err(|_| give.clone())
		} else {
			// buy
			let Fungible(amount) = wanted.fun else { return Err(give) };
			let Fungible(max_sell_amount) = given.fun else {
				return Err(give);
			};

			let route = pallet_route_executor::Pallet::<Runtime>::get_route(AssetPair::new(asset_in, asset_out));
			let Ok(amount_in) = pallet_route_executor::Pallet::<Runtime>::calculate_expected_amount_in(
				&route,
				amount.into(),
			) else {
				log::warn!(target: "xcm::exchange-asset", "Failed to calculate expected amount in for route: {:?}", route);
				return Err(give);
			};

			if !IssuanceIncreaseFuse::<Runtime>::can_mint(asset_in.into(), amount_in.into().into()) {
				log::warn!(target: "xcm::exchange-asset", "Circuit breaker triggered for asset {:?}. Asset will be trapped.", asset_in);
				return Err(give);
			}

			with_transaction_result(|| {
				Currency::deposit(asset_in, &account, max_sell_amount.into())?; // mint the incoming tokens
				pallet_route_executor::Pallet::<Runtime>::buy(
					origin,
					asset_in,
					asset_out,
					amount.into(),
					max_sell_amount.into(),
					use_onchain_route,
				)?;
				let mut assets = sp_std::vec::Vec::with_capacity(2);
				let left_over = Currency::free_balance(asset_in, &account);
				if left_over > <Runtime as pallet_route_executor::Config>::Balance::zero() {
					Currency::withdraw(asset_in, &account, left_over)?; // burn left over tokens
					let holding: Asset = (given.id.clone(), left_over.into()).into();
					assets.push(holding);
				}
				let amount_received = Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received == amount.into(),
					"Buy should return exactly the amount we specified."
				);
				Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				let holding: Asset = (wanted.id.clone(), amount_received.into()).into();
				assets.push(holding);
				Ok(assets.into())
			})
			.map_err(|_| give.clone())
		};
		if pallet_broadcast::Pallet::<Runtime>::remove_from_context().is_err() {
			log::error!(target: "xcm::exchange-asset", "Failed to remove from context.");
			return Err(give);
		};

		trade_result
	}

	fn quote_exchange_price(give: &Assets, want: &Assets, maximal: bool) -> Option<Assets> {
		if give.len() != 1 {
			log::warn!(target: "xcm::exchange-asset", "Only one give asset is supported.");
			return None;
		};

		//We assume only one asset wanted as translating into buy and sell is ambigous for multiple want assets
		if want.len() != 1 {
			log::warn!(target: "xcm::exchange-asset", "Only one want asset is supported.");
			return None;
		};

		let given = give.get(0)?;
		let asset_in = CurrencyIdConvert::convert(given.clone())?;

		let wanted = want.get(0)?;
		let asset_out = CurrencyIdConvert::convert(wanted.clone())?;

		let route = pallet_route_executor::Pallet::<Runtime>::get_route(AssetPair::new(asset_in, asset_out));

		if maximal {
			// sell
			let Fungible(amount) = given.fun else { return None };
			let amount =
				pallet_route_executor::Pallet::<Runtime>::calculate_expected_amount_out(&route, amount.into()).ok()?;
			Some(
				Asset {
					id: wanted.id.clone(),
					fun: Fungible(amount.into()),
				}
				.into(),
			)
		} else {
			// buy
			let Fungible(amount) = wanted.fun else { return None };
			let amount =
				pallet_route_executor::Pallet::<Runtime>::calculate_expected_amount_in(&route, amount.into()).ok()?;
			Some(
				Asset {
					id: given.id.clone(),
					fun: Fungible(amount.into()),
				}
				.into(),
			)
		}
	}
}
