use super::*;
use crate::tests::mock::AssetId as CurrencyId;
use crate::xcm_exchange::OmniExchanger;
use frame_support::{assert_noop, parameter_types};
use polkadot_xcm::latest::prelude::*;
use pretty_assertions::assert_eq;
use sp_runtime::traits::Convert;
use sp_runtime::Permill;
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
fn omni_exchanger_exchanges_supported_assets() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give = MultiAsset::from((GeneralIndex(DAI.into()), 50 * UNITS)).into();
			let want = MultiAsset::from((GeneralIndex(HDX.into()), 100 * UNITS)).into();
			assert_ok!(
				OmniExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert>::exchange_asset(None, give, &want, SELL)
			);
		});
}
