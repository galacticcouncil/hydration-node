use crate::*;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade};

mod unversioned {
	use super::*;

	pub struct InnerMigrateV1ToV2<T: crate::Config, BifrostAccount: Get<T::AccountId>>(
		core::marker::PhantomData<(T, BifrostAccount)>,
	);
}

impl<T: crate::Config, BifrostAccount: Get<T::AccountId>> UncheckedOnRuntimeUpgrade
	for unversioned::InnerMigrateV1ToV2<T, BifrostAccount>
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(target: "runtime::ema-oracle", "v1->v2 migration started");

		ExternalSources::<T>::insert(BIFROST_SOURCE, ());

		let bifrost_account = BifrostAccount::get();
		let dot_vdot = ordered_pair(5, 15);
		AuthorizedAccounts::<T>::insert((BIFROST_SOURCE, dot_vdot, &bifrost_account), ());

		log::info!(target: "runtime::ema-oracle", "v1->v2 migration finished");

		T::DbWeight::get().reads_writes(0, 2)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		let bifrost_account = BifrostAccount::get();
		assert!(ExternalSources::<T>::contains_key(BIFROST_SOURCE));
		assert!(AuthorizedAccounts::<T>::contains_key((
			BIFROST_SOURCE,
			ordered_pair(5, 15),
			&bifrost_account
		)));
		Ok(())
	}
}

pub type MigrateV1ToV2<T, BifrostAccount> = VersionedMigration<
	1,
	2,
	unversioned::InnerMigrateV1ToV2<T, BifrostAccount>,
	Pallet<T>,
	<T as frame_system::Config>::DbWeight,
>;
