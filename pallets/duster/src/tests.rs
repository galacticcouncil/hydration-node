use super::*;
use crate::mock::{Duster, ExtBuilder, Origin, Test, Tokens, ALICE, DUSTER, TREASURY};
use frame_support::{assert_noop, assert_ok};

#[test]
fn dust_account_works() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);

			for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
				assert_ne!(who, *ALICE, "Alice account should have been removed!");
			}

			assert_eq!(Tokens::free_balance(0, &*DUSTER), 10_000);
		});
}
#[test]
fn dust_account_with_sufficient_balance_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 1_000_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}
#[test]
fn dust_account_with_exact_dust_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}
/*
use frame_system::InitKind;
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use sp_runtime::app_crypto::sp_core::offchain::TransactionPoolExt;
use sp_runtime::offchain::{
	testing::{self, TestOffchainExt},
	OffchainDbExt,
};
use sp_std::vec::Vec;
use std::sync::Arc;


#[test]
fn move_dust_test() {
	let mut ext = new_test_ext();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();

	const PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";

	let keystore = KeyStore::new();
	let r = SyncCryptoStore::sr25519_generate_new(&keystore, KEY_TYPE, Some(&format!("{}/hunter1", PHRASE))).unwrap();

	const ALICE_PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what bahno";
	let alice = SyncCryptoStore::sr25519_generate_new(&keystore, KEY_TYPE, Some(&format!("{}/alice", PHRASE))).unwrap();


	let mut t = new_test_ext();
	t.register_extension(OffchainDbExt::new(offchain));
	t.register_extension(TransactionPoolExt::new(pool));
	t.register_extension(KeystoreExt(Arc::new(keystore)));

	t.execute_with(|| {
		let b = Tokens::free_balance(1, &r);
		println!("{:?}",b);

		//Duster::transfer_dust_signed(&alice,1,100);
		//assert_ok!(Duster::transfer_dust(&alice, &r,1,100));
		assert_ok!(Duster::dust_account(Origin::signed(r), *ALICE,1));

		let b = Tokens::free_balance(1, &r);
		let b = Tokens::free_balance(1, &alice);
		let b = Tokens::free_balance(1, &ALICE);
		let b = Tokens::free_balance(1, &TREASURY);
		println!("{:?}",b);
	});
}*/
