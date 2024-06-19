use hex_literal::hex;
use hydradx_runtime::Currencies;
use hydradx_traits::evm::InspectEvmAccounts;
use orml_traits::MultiCurrency;
use primitives::{AccountId, AssetId, Balance};
use sp_core::H160;

// Private key: 42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14
// Address: 	0x222222ff7Be76052e023Ec1a306fCca8F9659D80
// Account Id: 	45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000
// SS58(63): 	7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb
pub(crate) fn alith_evm_address() -> H160 {
	hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"].into()
}
pub(crate) fn alith_evm_account() -> AccountId {
	hex!["45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000"].into()
}
pub(crate) fn alith_truncated_account() -> AccountId {
	hydradx_runtime::EVMAccounts::truncated_account_id(alith_evm_address())
}
pub(crate) fn alith_secret_key() -> [u8; 32] {
	hex!("42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14")
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
