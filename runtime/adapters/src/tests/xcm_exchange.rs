use crate::tests::mock::AccountId;
use crate::tests::mock::AssetId as CurrencyId;
use crate::tests::mock::*;
use crate::tests::mock::{DAI, HDX, NATIVE_AMOUNT};
use crate::xcm_exchange::XcmAssetExchanger;
use frame_support::{assert_noop, assert_ok, parameter_types};
use hydradx_traits::router::{AssetPair, PoolType, Trade};
use orml_traits::MultiCurrency;
use polkadot_xcm::latest::prelude::*;
use pretty_assertions::assert_eq;
use sp_runtime::traits::Convert;
use sp_runtime::{FixedU128, SaturatedConversion};
use xcm_executor::traits::AssetExchange;
use xcm_executor::AssetsInHolding;

parameter_types! {
	pub ExchangeTempAccount: AccountId = 12345;
	pub DefaultPoolType: PoolType<crate::tests::mock::AssetId>  = PoolType::Omnipool;
}

const BUY: bool = false;
const SELL: bool = true;
const UNITS: u128 = 1_000_000_000_000;

pub struct CurrencyIdConvert;

impl Convert<Location, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: Location) -> Option<CurrencyId> {
		match location {
			Location {
				parents: 0,
				interior: Junctions::X1(a),
			} => match (*a)[0] {
				GeneralIndex(index) => Some(index.saturated_into()),
				_ => None,
			},
			_ => None,
		}
	}
}

impl Convert<Asset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: Asset) -> Option<CurrencyId> {
		let Asset { id: asset_id, .. } = asset;
		Self::convert(asset_id.0)
	}
}

#[test]
fn xcm_exchanger_allows_selling_supported_assets() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give: Assets = Asset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want: Assets = Asset::from((GeneralIndex(HDX.into()), wanted_amount)).into();

			// get the price before executing the swap
			let expected_amount_out =
				XcmAssetExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert, Currencies>::quote_exchange_price(
					&give, &want, SELL,
				)
				.unwrap();

			// Act
			let received: Assets = exchange_asset(None, give, &want, SELL)
				.expect("should return ok")
				.into();

			// Assert
			let mut iter = received.inner().iter().filter(|asset| asset.is_fungible(None));
			let asset_received = iter.next().expect("there should be at least one asset");
			assert!(iter.next().is_none(), "there should only be one asset returned");
			let Fungible(received_amount) = asset_received.fun else {
				panic!("should be fungible")
			};
			assert!(received_amount >= wanted_amount);
			assert_eq!(Tokens::free_balance(DAI, &ExchangeTempAccount::get()), 0);
			assert_eq!(Balances::free_balance(ExchangeTempAccount::get()), 0);

			// verify that quote_exchange_price returns correct value
			assert_eq!(expected_amount_out.len(), 1, "there should only be one asset returned");
			let wanted = expected_amount_out.get(0).unwrap();
			assert_eq!(wanted.id.0, GeneralIndex(HDX.into()).into());
			let Fungible(amount_out) = wanted.fun else {
				panic!("should be fungible")
			};
			assert_eq!(received_amount, amount_out);
		});
}

#[test]
fn xcm_exchanger_should_work_with_onchain_route() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(CHARLIE, HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			create_xyk_pool(HDX, DOT);

			assert_ok!(RouteExecutor::set_route(
				RuntimeOrigin::signed(CHARLIE),
				AssetPair::new(DAI, DOT),
				vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DAI,
						asset_out: HDX,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
				],
			));

			let give = Asset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 40 * UNITS; // 50 - 10 to cover fees
			let want: Assets = Asset::from((GeneralIndex(DOT.into()), wanted_amount)).into();

			// Act
			let received: Assets = exchange_asset(None, give, &want, SELL)
				.expect("should return ok")
				.into();

			// Assert
			let mut iter = received.inner().iter().filter(|asset| asset.is_fungible(None));
			let asset_received = iter.next().expect("there should be at least one asset");
			assert!(iter.next().is_none(), "there should only be one asset returned");
			let Fungible(received_amount) = asset_received.fun else {
				panic!("should be fungible")
			};
			assert!(received_amount >= wanted_amount);
			assert_eq!(Tokens::free_balance(DAI, &ExchangeTempAccount::get()), 0);
			assert_eq!(Balances::free_balance(ExchangeTempAccount::get()), 0);
		});
}

#[test]
fn xcm_exchanger_allows_buying_supported_assets() {
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
			let give_asset = Asset::from((GeneralIndex(DAI.into()), given_amount));
			let give = give_asset.into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want_asset = Asset::from((GeneralIndex(HDX.into()), wanted_amount));
			let want: Assets = want_asset.clone().into();

			// get the price before executing the swap
			let expected_amount_in =
				XcmAssetExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert, Currencies>::quote_exchange_price(
					&give, &want, BUY,
				)
				.unwrap();

			// Act
			let received: Assets = exchange_asset(None, give, &want, BUY).expect("should return ok").into();

			// Assert
			let mut iter = received.inner().iter().filter(|asset| asset.is_fungible(None));
			let asset_received = iter.next().expect("there should be at least one asset");
			let left_over = iter
				.next()
				.expect("there should be at least some left_over asset_in")
				.clone();

			assert!(iter.next().is_none(), "there should only be two assets returned");

			let Fungible(left_over_amount) = left_over.fun else {
				panic!("should be fungible")
			};

			assert_eq!(left_over, (GeneralIndex(DAI.into()), left_over_amount).into());
			assert!(left_over_amount < given_amount);
			assert_eq!(*asset_received, want_asset);

			let Fungible(received_amount) = asset_received.fun else {
				panic!("should be fungible")
			};

			assert!(received_amount == wanted_amount);
			assert_eq!(Tokens::free_balance(DAI, &ExchangeTempAccount::get()), 0);
			assert_eq!(Balances::free_balance(ExchangeTempAccount::get()), 0);

			// verify that quote_exchange_price returns correct value
			assert_eq!(expected_amount_in.len(), 1, "there should only be one asset returned");
			let given = expected_amount_in.get(0).unwrap();
			assert_eq!(given.id.0, GeneralIndex(DAI.into()).into());
			let Fungible(amount_in) = given.fun else {
				panic!("should be fungible")
			};
			assert_eq!(given_amount - left_over_amount, amount_in);
		});
}

#[test]
fn xcm_exchanger_should_not_allow_trading_for_multiple_assets() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give: Assets = Asset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want1: Asset = Asset::from((GeneralIndex(HDX.into()), wanted_amount));
			let want2: Asset = Asset::from((GeneralIndex(DAI.into()), wanted_amount));
			let want: Assets = vec![want1, want2].into();

			// Act and assert
			assert_noop!(exchange_asset(None, give.clone(), &want, SELL), give);
		});
}

#[test]
fn xcm_exchanger_works_with_specified_origin() {
	// Arrange
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let give = Asset::from((GeneralIndex(DAI.into()), 100 * UNITS)).into();
			let wanted_amount = 45 * UNITS; // 50 - 5 to cover fees
			let want = Asset::from((GeneralIndex(HDX.into()), wanted_amount)).into();

			// Act and assert
			assert_ok!(exchange_asset(Some(&Location::here()), give, &want, SELL));
		});
}

fn exchange_asset(
	origin: Option<&Location>,
	give: Assets,
	want: &Assets,
	is_sell: bool,
) -> Result<AssetsInHolding, AssetsInHolding> {
	XcmAssetExchanger::<Test, ExchangeTempAccount, CurrencyIdConvert, Currencies>::exchange_asset(
		origin,
		give.into(),
		want,
		is_sell,
	)
}

fn create_xyk_pool(asset_a: u32, asset_b: u32) {
	let amount = 100000 * ONE;
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		DAVE,
		asset_a,
		amount as i128,
	));

	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		DAVE,
		asset_b,
		amount as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE),
		asset_a,
		amount,
		asset_b,
		amount,
	));
}
