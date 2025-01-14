use primitives::{AccountId, AssetId};

pub(crate) type Balance = u128;

pub(crate) struct TestingSuite {
}


impl TestingSuite{

    pub(crate) fn default() -> Self{
        TestingSuite{
        }
    }

    pub(crate) fn with_balances(&mut self, entries: (AccountId, Balance)) -> Self{
        self
    }

    pub(crate) fn build(&self) -> Self{
        self
    }

    pub(crate) fn query_balance(&self, account: AccountId, asset_id: AssetId, on_result: impl Fn(Balance)) -> Self{
        if asset_id == 0 {
            self.query_native_balance(account, on_result);
        }else{
            //query tokens
            on_result(0);
        }
        self
    }

    pub(crate) fn query_native_balance(&self, account: AccountId, on_result: Fn(Balance)) -> Self{
        on_result(0);
        self
    }

}