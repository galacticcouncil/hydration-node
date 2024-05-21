use hex_literal::hex;
use hydradx_runtime::Currencies;
use hydradx_traits::evm::InspectEvmAccounts;
use orml_traits::MultiCurrency;
use primitives::{AccountId, AssetId, Balance};
use sp_core::H160;

// subkey inspect --network hydradx //Alice
// Private key: e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a
// EVM Address: d43593c715fdd31c61141abd04a99fd6822c8558
// Account Id: 	d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d
// SS58(63): 	7NPoMQbiA6trJKkjB35uk96MeJD4PGWkLQLH7k7hXEkZpiba
pub(crate) fn alith_evm_address() -> H160 {
	hex!["d43593c715fdd31c61141abd04a99fd6822c8558"].into()
}
pub(crate) fn alith_evm_account() -> AccountId {
	hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into()
}
pub(crate) fn alith_truncated_account() -> AccountId {
	hydradx_runtime::EVMAccounts::truncated_account_id(alith_evm_address())
}
pub(crate) fn alith_secret_key() -> [u8; 32] {
	hex!("e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a")
}

pub(crate) struct MockAccount(AccountId);

impl MockAccount {
	pub fn new(address: AccountId) -> Self {
		Self(address)
	}
	pub fn address(&self) -> AccountId {
		self.0.clone()
	}

	pub fn balance(&self, asset: AssetId) -> Balance {
		Currencies::free_balance(asset, &self.0)
	}
}

//pub(crate) static TreasuryAccount: MockAccount = MockAccount::new(Treasury::account_id());
