use crate::xcm_account_derivation::HashedDescriptionDescribeFamilyAllTerminal;
use polkadot_xcm::latest::prelude::*;
use pretty_assertions::assert_eq;
use xcm_executor::traits::Convert;

#[test]
fn test_hashed_family_all_terminal_converter() {
	type Converter<AccountId> = HashedDescriptionDescribeFamilyAllTerminal<AccountId>;

	assert_eq!(
		[
			129, 211, 14, 6, 146, 54, 225, 200, 135, 103, 248, 244, 125, 112, 53, 133, 91, 42, 215, 236, 154, 199, 191,
			208, 110, 148, 223, 55, 92, 216, 250, 34
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 0,
			interior: X2(
				Parachain(1),
				AccountId32 {
					network: None,
					id: [0u8; 32]
				}
			),
		})
		.unwrap()
	);
	assert_eq!(
		[
			17, 142, 105, 253, 199, 34, 43, 136, 155, 48, 12, 137, 155, 219, 155, 110, 93, 181, 93, 252, 124, 60, 250,
			195, 229, 86, 31, 220, 121, 111, 254, 252
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 1,
			interior: X2(
				Parachain(1),
				AccountId32 {
					network: None,
					id: [0u8; 32]
				}
			),
		})
		.unwrap()
	);
	assert_eq!(
		[
			237, 65, 190, 49, 53, 182, 196, 183, 151, 24, 214, 23, 72, 244, 235, 87, 187, 67, 52, 122, 195, 192, 10,
			58, 253, 49, 0, 112, 175, 224, 125, 66
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 0,
			interior: X2(
				Parachain(1),
				AccountKey20 {
					network: None,
					key: [0u8; 20]
				}
			),
		})
		.unwrap()
	);
	assert_eq!(
		[
			226, 225, 225, 162, 254, 156, 113, 95, 68, 155, 160, 118, 126, 18, 166, 132, 144, 19, 8, 204, 228, 112,
			164, 189, 179, 124, 249, 1, 168, 110, 151, 50
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 1,
			interior: X2(
				Parachain(1),
				AccountKey20 {
					network: None,
					key: [0u8; 20]
				}
			),
		})
		.unwrap()
	);
	assert_eq!(
		[
			254, 186, 179, 229, 13, 24, 84, 36, 84, 35, 64, 95, 114, 136, 62, 69, 247, 74, 215, 104, 121, 114, 53, 6,
			124, 46, 42, 245, 121, 197, 12, 208
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 1,
			interior: X2(Parachain(2), PalletInstance(3)),
		})
		.unwrap()
	);
	assert_eq!(
		[
			217, 56, 0, 36, 228, 154, 250, 26, 200, 156, 1, 39, 254, 162, 16, 187, 107, 67, 27, 16, 218, 254, 250, 184,
			6, 27, 216, 138, 194, 93, 23, 165
		],
		Converter::<[u8; 32]>::convert(MultiLocation {
			parents: 1,
			interior: Here,
		})
		.unwrap()
	);
}
