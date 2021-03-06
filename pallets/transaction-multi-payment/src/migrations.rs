use frame_support::traits::PalletVersion;

pub fn migrate_to_2_0_1() -> frame_support::weights::Weight {
	let version_201 = PalletVersion::new(2, 0, 1);

	// TODO: do the storage type change

	// Not sure - this might be done automatically on upgrade?! to store the current crate version
	version_201.put_into_storage();

	0
}
