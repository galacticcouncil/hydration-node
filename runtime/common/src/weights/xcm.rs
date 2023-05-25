#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_xcm::WeightInfo for WeightInfo<T> {
	fn send() -> Weight {
        Weight::zero()
	}
	fn teleport_assets() -> Weight {
        Weight::zero()
    }
	fn reserve_transfer_assets() -> Weight {
        Weight::zero()
	}
	fn execute() -> Weight {
        Weight::zero()
	}
	fn force_xcm_version() -> Weight {
        Weight::zero()
	}
	fn force_default_xcm_version() -> Weight {
        Weight::zero()
	}
	fn force_subscribe_version_notify() -> Weight {
        Weight::zero()
	}
	fn force_unsubscribe_version_notify() -> Weight {
        Weight::zero()
	}
	fn migrate_supported_version() -> Weight {
        Weight::zero()
	}
	fn migrate_version_notifiers() -> Weight {
        Weight::zero()
	}
	fn already_notified_target() -> Weight {
        Weight::zero()
	}
	fn notify_current_targets() -> Weight {
        Weight::zero()
	}
	fn notify_target_migration_fail() -> Weight {
        Weight::zero()
	}
	fn migrate_version_notify_targets() -> Weight {
        Weight::zero()
	}
	fn migrate_and_notify_old_targets() -> Weight {
        Weight::zero()
	}
}