use crate::{Config, Pallet};
use frame_support::traits::OriginTrait;
use frame_system::pallet_prelude::OriginFor;
use polkadot_xcm::latest::prelude::*;
use sp_core::Get;
use sp_runtime::traits::{AccountIdConversion, Convert};
use std::marker::PhantomData;
use xcm_executor::traits::AssetExchange;

pub struct OmniExchanger<T, TempAccount, CurrencyIdConvert>(PhantomData<(T, TempAccount, CurrencyIdConvert)>);

impl<T, TempAccount, CurrencyIdConvert> AssetExchange for OmniExchanger<T, TempAccount, CurrencyIdConvert>
where
	T: Config,
	TempAccount: Get<T::AccountId>,
	CurrencyIdConvert: Convert<MultiAsset, Option<T::AssetId>>,
{
	fn exchange_asset(
		origin: Option<&MultiLocation>,
		give: xcm_executor::Assets,
		want: &MultiAssets,
		maximal: bool,
	) -> Result<xcm_executor::Assets, xcm_executor::Assets> {
		use orml_utilities::with_transaction_result;
		use sp_runtime::traits::Convert;

		if maximal {
			// sell
			let account = if origin.is_none() {
				TempAccount::get()
			} else {
				return Err(give);
			};
			let origin = T::RuntimeOrigin::from(frame_system::RawOrigin::Signed(account.clone())); //TODO: check how else it is done in hydra in a simpler way
			if give.len() != 1 {
				return Err(give);
			}; // TODO: we assume only one asset given
			if want.len() != 1 {
				return Err(give);
			}; // TODO: we assume only one asset wanted
			let given = give
				.fungible_assets_iter()
				.next()
				.expect("length of 1 checked above; qed");
			// TODO: log errors
			let Fungible(amount) = given.fun else { return Err(give) };
			let Some(asset_in) = CurrencyIdConvert::convert(given) else { return Err(give) };
			let Some(wanted) = want.get(0) else { return Err(give) };
			let Fungible(min_buy_amount) = wanted.fun else { return Err(give) };
			let Some(asset_out) = CurrencyIdConvert::convert(wanted.clone()) else { return Err(give) }; // TODO: unnecessary clone, maybe?

			with_transaction_result(|| {
				// TODO: mint

				Pallet::<T>::sell(origin, asset_in, asset_out, amount, min_buy_amount)
			})
			.map_err(|_| give)
			.map(|_| todo!("burn and return"))
		} else {
			// buy
			Err(give) // TODO
		}
	}
}
