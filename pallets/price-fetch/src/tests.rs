use super::*;
use crate::mock::*;

use sp_runtime::offchain::{
    testing::{TestOffchainExt, self},
    OffchainExt, TransactionPoolExt
};
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use frame_support::{assert_ok, assert_noop};
use std::sync::Arc;
use sp_std::vec::Vec;

#[test]
fn parse_res_from_dia_should_work() {
	let data = "{\"Symbol\":\"BTC\",\"Name\":\"Bitcoin\",\"Price\":17202.936692749197,\"PriceYesterday\":18792.55191581324,\"VolumeYesterdayUSD\":233661096.57253397,\"Source\":\"diadata.org\",\"Time\":\"2020-11-26T20:02:19.699386233Z\",\"ITIN\":\"undefined\"}";

	let p = PriceFetch::parse_dia_res(data).unwrap();

	assert_eq!(p.price, Price::from_fraction(17202.936692749197));
	assert_eq!(p.time, "2020-11-26T20:02:19.699386233Z".as_bytes());
	assert_eq!(p.symbol, "BTC".as_bytes());

	//Failed parse should return None
	let invalid_data = "";
	assert_eq!(PriceFetch::parse_dia_res(invalid_data), None);
}

#[test]
fn fetch_price_req_should_work() {
	let url = b"https://api.diadata.org/v1/quotation/ETH".to_vec();

	let (offchain, state) = sp_core::offchain::testing::TestOffchainExt::new();
	let mut t = sp_io::TestExternalities::default();

	t.register_extension(OffchainExt::new(offchain));

	{
		let mut state = state.write();
		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "GET".into(),
			uri: "https://api.diadata.org/v1/quotation/ETH".into(),
			response: Some(br#"{"Symbol":"ETH","Name":"Ethereum","Price":599.5155962856843,"PriceYesterday":611.6692248881053,"VolumeYesterdayUSD":230899109.84247947,"Source":"diadata.org","Time":"2020-12-04T17:22:35.694940893Z","ITIN":"undefined"}"#.to_vec()),
			sent: true,
			..Default::default()
		});
	}
 
	let p1 = DiaPriceRecord {
		price: Price::from_fraction(599.5155962856843),
		time: b"2020-12-04T17:22:35.694940893Z".to_vec(),
		symbol: b"ETH".to_vec(),
	};

	t.execute_with(|| {
		let dia_price = PriceFetch::fetch_price(url).unwrap();

		assert_eq!(dia_price, p1);
	})
}

#[test]
fn start_new_fetcher_should_work() {
	sp_io::TestExternalities::default().execute_with(|| {
		assert_ok!(PriceFetch::start_fetcher(Origin::signed(Default::default())));

		let key = b"ETH";
		let should_be_fetcher = Fetcher {
			symbol: key.to_vec(),
			url: b"https://api.diadata.org/v1/quotation/ETH".to_vec(),
			end_fetching_at: 600,
		};

		let fetcher = <Fetchers<Test>>::get(&key.to_vec());

		assert_eq!(fetcher, should_be_fetcher);
	})
}

#[test]
fn start_existing_fetcher_should_fail() {
	sp_io::TestExternalities::default().execute_with(|| {
		assert_ok!(PriceFetch::start_fetcher(Origin::signed(Default::default())));

		assert_noop!(
			PriceFetch::start_fetcher(Origin::signed(Default::default())),
			Error::<Test>::FetcherAlreadyExist
		);
	})
}

#[test]
fn add_new_price_to_storage_should_work() {
	sp_io::TestExternalities::default().execute_with(|| {
		let key = b"ETH".to_vec();
		let p1 = FetchedPrice {
			price: Price::from(10),
			symbol: key.clone(),
			time: "2020-11-26T20:02:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		};

		let p2 = FetchedPrice {
			price: Price::from_fraction(8.23455),
			symbol: key.clone(),
			time: "2020-12-16T20:02:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		};

		let p3 = FetchedPrice {
			price: Price::from_fraction(11.432),
			symbol: key.clone(),
			time: "2020-10-20T20:02:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		};

		PriceFetch::add_new_price_to_list(p1.clone());
		let mut stored = <FetchedPrices<Test>>::get(key.clone());
		assert_eq!(stored[0], p1);

		PriceFetch::add_new_price_to_list(p2.clone());
		stored = <FetchedPrices<Test>>::get(key.clone());
		assert_eq!(stored[1], p2);

		PriceFetch::add_new_price_to_list(p3.clone());
		stored = <FetchedPrices<Test>>::get(key.clone());

		assert_eq!(stored[0], p1);
		assert_eq!(stored[1], p2);
		assert_eq!(stored[2], p3);
	})
}

#[test]
fn cal_avg_price_and_submit_should_work() {
	let mut _ext = new_test_ext();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();

	const PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";

	let keystore = KeyStore::new();
	SyncCryptoStore::sr25519_generate_new(&keystore, KEY_TYPE, Some(&format!("{}/hunter1", PHRASE))).unwrap();

	let mut t = sp_io::TestExternalities::default();
	t.register_extension(OffchainExt::new(offchain));
	t.register_extension(TransactionPoolExt::new(pool));
	t.register_extension(KeystoreExt(Arc::new(keystore)));

	t.execute_with(|| {
		let key = b"ETH".to_vec();
		let prices: Vec<Price> = [
			Price::from_fraction(1232.032342323423),
			Price::from_fraction(3223332.32032890342),
			Price::from_fraction(82339.3203842),
			Price::from_fraction(812341241234214.320381241242),
			Price::from_fraction(234214.1241242),
		]
		.to_vec();

		PriceFetch::add_new_price_to_list(FetchedPrice {
			price: Price::from(prices[0]),
			symbol: key.clone(),
			time: "2020-11-26T20:02:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		});

		PriceFetch::add_new_price_to_list(FetchedPrice {
			price: Price::from(prices[1]),
			symbol: key.clone(),
			time: "2020-11-26T20:03:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		});

		PriceFetch::add_new_price_to_list(FetchedPrice {
			price: Price::from(prices[2]),
			symbol: key.clone(),
			time: "2020-11-26T20:04:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		});

		PriceFetch::add_new_price_to_list(FetchedPrice {
			price: Price::from(prices[3]),
			symbol: key.clone(),
			time: "2020-11-26T20:05:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		});

		PriceFetch::add_new_price_to_list(FetchedPrice {
			price: Price::from(prices[4]),
			symbol: key.clone(),
			time: "2020-11-26T20:06:19.699386233Z".as_bytes().to_vec(),
			author: Default::default(),
		});

		let _result = PriceFetch::calc_and_submit_avg_price(Fetcher {
			symbol: key.to_vec(),
			url: b"https://api.diadata.org/v1/quotation/ETH".to_vec(),
			end_fetching_at: 600,
		});

		let mut sum = Price::from(0);
		prices.iter().for_each(|price| {
			sum = sum + price.clone();
		});

		let avg = sum / Price::from(5);

		let tx = pool_state.write().transactions.pop().unwrap();
		assert!(pool_state.read().transactions.is_empty());
		let tx = mock::Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature.unwrap().0, 0);
		assert_eq!(tx.call, mock::Call::PriceFetch(crate::Call::submit_new_avg_price(key.clone(), avg)));
        
	})
}


/*
#[test]
fn offchain_should_work() {
	use frame_support::traits::OffchainWorker;
	let mut ext = new_test_ext();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	ext.register_extension(OffchainExt::new(offchain));
	const PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";
	let keystore = KeyStore::new();
	keystore.write().sr25519_generate_new(
		crate::crypto::Public::ID,
		Some(&format!("{}/hunter1", PHRASE))
	).unwrap();
	let mut t = sp_io::TestExternalities::default();
	t.register_extension(OffchainExt::new(offchain));
	t.register_extension(TransactionPoolExt::new(pool));
	t.register_extension(KeystoreExt(keystore));
	ext.execute_with(|| {
		assert_ok!(PriceFetch::start_fetcher(Origin::signed(Default::default())));
		mock::run_to_block(10);
		PriceFetch::offchain_worker(3);
	})
}
*/
