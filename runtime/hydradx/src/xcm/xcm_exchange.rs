use common_runtime::AccountId;
use polkadot_xcm::latest::prelude::*;
use sp_core::Get;
use sp_runtime::traits::Convert;
use sp_std::marker::PhantomData;
use xcm_executor::traits::AssetExchange;

//TODO: copy unit tests from omnipool and adapt them

pub struct OmniExchanger<T, TempAccount, CurrencyIdConvert>(PhantomData<(T, TempAccount, CurrencyIdConvert)>);

impl<T, TempAccount, CurrencyIdConvert> AssetExchange for OmniExchanger<T, TempAccount, CurrencyIdConvert>
where
	T: pallet_omnipool::Config,
	TempAccount: Get<T::AccountId>,
	CurrencyIdConvert: Convert<MultiAsset, Option<T::AssetId>>,
{
	fn exchange_asset(
		origin: Option<&MultiLocation>,
		give: xcm_executor::Assets,
		want: &MultiAssets,
		maximal: bool,
	) -> Result<xcm_executor::Assets, xcm_executor::Assets> {
		use orml_traits::MultiCurrency;
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
		let given = give
			.fungible_assets_iter()
			.next()
			.expect("length of 1 checked above; qed"); // TODO: Use let Some(give), else Err, and also log

		let Some(asset_in) = CurrencyIdConvert::convert(given.clone()) else { return Err(give) };
		let Some(wanted) = want.get(0) else { return Err(give) };
		let Some(asset_out) = CurrencyIdConvert::convert(wanted.clone()) else { return Err(give) };

		if maximal {
			// sell
			let Fungible(amount) = given.fun else { return Err(give) };
			let Fungible(min_buy_amount) = wanted.fun else { return Err(give) };

			with_transaction_result(|| {
				T::Currency::deposit(asset_in, &account, amount)?; // mint the incoming tokens
				pallet_omnipool::Pallet::<T>::sell(origin, asset_in, asset_out, amount, min_buy_amount)?;
				debug_assert!(
					T::Currency::free_balance(asset_in, &account) == 0,
					"Sell should not leave any of the incoming asset."
				);
				let amount_received = T::Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received >= min_buy_amount,
					"Sell should return more than mininum buy amount."
				);
				T::Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				Ok(MultiAsset::from((wanted.id, amount_received)).into())
			})
			.map_err(|_| give)
		} else {
			// buy
			let Fungible(amount) = wanted.fun else { return Err(give) };
			let Fungible(max_sell_amount) = given.fun else { return Err(give) };

			with_transaction_result(|| {
				T::Currency::deposit(asset_in, &account, max_sell_amount)?; // mint the incoming tokens
				pallet_omnipool::Pallet::<T>::buy(origin, asset_out, asset_in, amount, max_sell_amount)?;
				let mut assets = sp_std::vec::Vec::new();
				let left_over = T::Currency::free_balance(asset_in, &account);
				if left_over > 0 {
					T::Currency::withdraw(asset_in, &account, left_over)?; // burn left over tokens
					assets.push(MultiAsset::from((given.id, left_over)));
				}
				let amount_received = T::Currency::free_balance(asset_out, &account);
				debug_assert!(
					amount_received == amount,
					"Buy should return exactly the amount we specified."
				);
				T::Currency::withdraw(asset_out, &account, amount_received)?; // burn the received tokens
				assets.push(MultiAsset::from((wanted.id, amount_received)));
				Ok(assets.into())
			})
			.map_err(|_| give)
		}
	}
}
