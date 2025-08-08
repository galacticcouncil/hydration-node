use crate::Config;
use frame_support::traits::Get;
use orml_traits::{GetByKey, Handler, Happened};
use sp_runtime::traits::Bounded;
use sp_std::marker::PhantomData;

pub trait AssetDepositLimiter<AccountId, AssetId, Balance> {
	type DepositLimit: GetByKey<AssetId, Option<Balance>>;
	type Period: Get<u128>;
	type Issuance: GetByKey<AssetId, Balance>;
	type OnLimitReached: Happened<AssetId>;
	type OnLockdownDeposit: Handler<(AssetId, AccountId, Balance)>;
	type OnDepositRelease: Handler<(AssetId, AccountId)>;
}

pub struct NoDepositLimit<T>(PhantomData<T>);

impl<T: Config> AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance> for NoDepositLimit<T> {
	type DepositLimit = NoIssuanceIncreaseLimit<T>;
	type Period = ();
	type Issuance = NoIssuance<T>;
	type OnLimitReached = ();
	type OnLockdownDeposit = ();
	type OnDepositRelease = ();
}

pub struct NoIssuanceIncreaseLimit<T>(PhantomData<T>);

impl<T: Config> GetByKey<T::AssetId, Option<T::Balance>> for NoIssuanceIncreaseLimit<T> {
	fn get(_: &T::AssetId) -> Option<T::Balance> {
		Some(T::Balance::max_value())
	}
}

pub struct NoIssuance<T>(PhantomData<T>);
impl<T: Config> GetByKey<T::AssetId, T::Balance> for NoIssuance<T> {
	fn get(_: &T::AssetId) -> T::Balance {
		T::Balance::default()
	}
}
