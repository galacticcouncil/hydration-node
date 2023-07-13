use hydradx_traits::router::PoolType::Omnipool;
use orml_traits::MultiCurrency;
use pallet_route_executor::Trade;
use polkadot_xcm::latest::prelude::*;
use sp_core::Get;
use sp_runtime::traits::{Convert, Zero};
use sp_std::marker::PhantomData;
use sp_std::vec;
use xcm_executor::traits::AssetExchange;

pub struct OmniExchanger<T, TempAccount, CurrencyIdConvert, Currency>(
	PhantomData<(T, TempAccount, CurrencyIdConvert, Currency)>,
);

impl<T, TempAccount, CurrencyIdConvert, Currency> AssetExchange
	for OmniExchanger<T, TempAccount, CurrencyIdConvert, Currency>
where
	T: pallet_route_executor::Config,
	TempAccount: Get<T::AccountId>,
	CurrencyIdConvert: Convert<MultiAsset, Option<T::AssetId>>,
	Currency: MultiCurrency<T::AccountId, CurrencyId = T::AssetId, Balance = T::Balance>,
	T::Balance: From<u128> + Zero + Into<u128>,
{
	fn exchange_asset(
		origin: Option<&MultiLocation>,
		give: xcm_executor::Assets,
		want: &MultiAssets,
		maximal: bool,
	) -> Result<xcm_executor::Assets, xcm_executor::Assets> {
		use orml_utilities::with_transaction_result;

		let account = if origin.is_none() {
			TempAccount::get()
		} else {
			// TODO: we want to use temo account alwas becuase there is no sense using specific account for this "accounting/burning/minting/etc" temp work
			return Err(give);
		};
		let origin = T::RuntimeOrigin::from(frame_system::RawOrigin::Signed(account.clone())); //TODO: check how else it is done in hydra in a simpler way

		// TODO: log errors - investigate using defernsive or use log::warn "xcm::exchange-asset"
		if give.len() != 1 {
			return Err(give);
		}; // TODO: create an issue for this as it is easy to have multiple ExchangeAsset, and this would be just then an improvement

		//We assume only one asset wanted as translating into buy and sell is ambigous for multiple want assets
		if want.len() != 1 {
			return Err(give);
		};
		let Some(given) = give.fungible_assets_iter().next() else {
			return Err(give);
		};

		let Some(asset_in) = CurrencyIdConvert::convert(given.clone()) else { return Err(give) };
		let Some(wanted) = want.get(0) else { return Err(give) };
		let Some(asset_out) = CurrencyIdConvert::convert(wanted.clone()) else { return Err(give) };

		if maximal {
			// sell
			let Fungible(amount) = given.fun else { return Err(give) };
			let Fungible(min_buy_amount) = wanted.fun else { return Err(give) };

			with_transaction_result(|| {
				Currency::deposit(asset_in, &account, amount.into())?; // mint the incoming tokens
				pallet_route_executor::Pallet::<T>::sell(
					origin,
					asset_in,
					asset_out,
					amount.into(),
					min_buy_amount.into(),
					vec![Trade {
						pool: Omnipool,
						asset_in,
						asset_out,
					}],
				)?;
				debug_assert!(
					Currency::free_balance(asset_in, &account) == T::Balance::zero(),
					"Sell should not leave any of the incoming asset."
				);
				let amount_received = Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received >= min_buy_amount.into(),
					"Sell should return more than mininum buy amount."
				);
				Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				Ok(MultiAsset::from((wanted.id, amount_received.into())).into())
			})
			.map_err(|_| give)
		} else {
			// buy
			let Fungible(amount) = wanted.fun else { return Err(give) };
			let Fungible(max_sell_amount) = given.fun else { return Err(give) };

			with_transaction_result(|| {
				Currency::deposit(asset_in, &account, max_sell_amount.into())?; // mint the incoming tokens
				pallet_route_executor::Pallet::<T>::buy(
					origin,
					asset_in,
					asset_out,
					amount.into(),
					max_sell_amount.into(),
					vec![Trade {
						pool: Omnipool,
						asset_in,
						asset_out,
					}],
				)?;
				let mut assets = sp_std::vec::Vec::new();
				let left_over = Currency::free_balance(asset_in, &account);
				if left_over > T::Balance::zero() {
					Currency::withdraw(asset_in, &account, left_over)?; // burn left over tokens
					assets.push(MultiAsset::from((given.id, left_over.into())));
				}
				let amount_received = Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received == amount.into(),
					"Buy should return exactly the amount we specified."
				);
				Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				assets.push(MultiAsset::from((wanted.id, amount_received.into())));
				Ok(assets.into())
			})
			.map_err(|_| give)
		}
	}
}
