use super::*;
use crate::tests::mock::AssetId as CurrencyId;
use crate::tests::mock::{Balances, Tokens};
use crate::xcm_exchange::OmniExchanger;
use frame_support::{assert_noop, parameter_types};
use polkadot_xcm::latest::prelude::*;
use pretty_assertions::assert_eq;
use sp_runtime::traits::Convert;
use sp_runtime::SaturatedConversion;
use xcm_executor::traits::AssetExchange;

parameter_types! {
	pub ExchangeTempAccount: AccountId = 12345;
}

const BUY: bool = false;
const SELL: bool = true;
const UNITS: u128 = 1_000_000_000_000;

pub struct CurrencyIdConvert;

impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<CurrencyId> {
		match location {
			MultiLocation {
				parents: 0,
				interior: X1(GeneralIndex(index)),
			} => Some(index.saturated_into()),
			_ => None,
		}
	}
}

impl Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset {
			id: Concrete(location), ..
		} = asset
		{
			Self::convert(location)
		} else {
			None
		}
	}
}

#[test]
fn omni_exchanger_allows_selling_supported_assets() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give = MultiAsset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want = MultiAsset::from((GeneralIndex(HDX.into()), wanted_amount)).into();
			// Act
			let received =
				OmniExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert>::exchange_asset(None, give, &want, SELL)
					.expect("should return ok");
			// Assert
			let mut iter = received.fungible_assets_iter();
			let asset_received = iter.next().expect("there should be at least one asset");
			assert!(iter.next().is_none(), "there should only be one asset returned");
			let Fungible(received_amount) = asset_received.fun else { panic!("should be fungible")};
			assert!(received_amount >= wanted_amount);
			assert_eq!(Tokens::free_balance(DAI, &ExchangeTempAccount::get()), 0);
			assert_eq!(Balances::free_balance(&ExchangeTempAccount::get()), 0);
		});
}

#[test]
fn omni_exchanger_allows_buying_supported_assets() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let given_amount = 100 * UNITS;
			let give_asset = MultiAsset::from((GeneralIndex(DAI.into()), given_amount));
			let give = give_asset.into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want_asset = MultiAsset::from((GeneralIndex(HDX.into()), wanted_amount));
			let want = want_asset.clone().into();
			// Act
			let received =
				OmniExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert>::exchange_asset(None, give, &want, BUY)
					.expect("should return ok");
			// Assert
			let mut iter = received.fungible_assets_iter();
			let asset_received = iter.next().expect("there should be at least one asset");
			let left_over = iter.next().expect("there should be at least some left_over asset_in");
			assert!(iter.next().is_none(), "there should only be two assets returned");
			let Fungible(left_over_amount) = left_over.fun else { panic!("should be fungible")};
			assert_eq!(left_over, (GeneralIndex(DAI.into()), left_over_amount).into());
			assert!(left_over_amount < given_amount);
			assert_eq!(asset_received, want_asset);
			let Fungible(received_amount) = asset_received.fun else { panic!("should be fungible")};
			assert!(received_amount == wanted_amount);
			assert_eq!(Tokens::free_balance(DAI, &ExchangeTempAccount::get()), 0);
			assert_eq!(Balances::free_balance(&ExchangeTempAccount::get()), 0);
		});
}

#[test]
fn omni_exchanger_should_not_allow_trading_for_multiple_assets() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give: MultiAssets = MultiAsset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want1: MultiAsset = MultiAsset::from((GeneralIndex(HDX.into()), wanted_amount));
			let want2: MultiAsset = MultiAsset::from((GeneralIndex(DAI.into()), wanted_amount));
			let want: MultiAssets = vec![want1, want2].into();

			// Act and assert
			assert_noop!(
				OmniExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert>::exchange_asset(
					None,
					give.clone().into(),
					&want,
					SELL
				),
				give
			);
		});
}
