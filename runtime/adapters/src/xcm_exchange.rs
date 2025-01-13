use orml_traits::MultiCurrency;
use pallet_broadcast::types::ExecutionType;
use polkadot_xcm::v4::prelude::*;
use sp_core::Get;
use sp_runtime::traits::{Convert, Zero};
use sp_std::marker::PhantomData;
use sp_std::vec;
use xcm_executor::traits::AssetExchange;
use xcm_executor::AssetsInHolding;

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
	Runtime: pallet_route_executor::Config,
	TempAccount: Get<Runtime::AccountId>,
	CurrencyIdConvert: Convert<Asset, Option<Runtime::AssetId>>,
	Currency: MultiCurrency<Runtime::AccountId, CurrencyId = Runtime::AssetId, Balance = Runtime::Balance>,
	Runtime::Balance: From<u128> + Zero + Into<u128>,
	Runtime::AssetId: Into<u32>,
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
		let use_onchain_route = vec![];

		pallet_broadcast::Pallet::<Runtime>::add_to_context(ExecutionType::XcmExchange);

		let trade_result = if maximal {
			// sell
			let Fungible(amount) = given.fun else { return Err(give) };
			let Fungible(min_buy_amount) = wanted.fun else {
				return Err(give);
			};

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
					Currency::free_balance(asset_in, &account) == Runtime::Balance::zero(),
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
				if left_over > Runtime::Balance::zero() {
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

		pallet_broadcast::Pallet::<Runtime>::remove_from_context();

		trade_result
	}

	fn quote_exchange_price(_give: &Assets, _want: &Assets, _maximal: bool) -> Option<Assets> {
		todo!() // TODO:
	}
}
