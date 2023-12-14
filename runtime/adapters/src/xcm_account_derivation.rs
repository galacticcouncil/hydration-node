use super::*;
use codec::{Compact, Encode};
use sp_io::hashing::blake2_256;
use sp_std::{borrow::Borrow, marker::PhantomData, vec::Vec};
use xcm_executor::traits::Convert;

/// NOTE: Copied from <https://github.com/moonbeam-foundation/polkadot/blob/d83bb6cc7d7c93ead2fd3cafce0e268fd3f6b9bc/xcm/xcm-builder/src/location_conversion.rs#L25C1-L68C2>
///
/// temporary struct that mimics the behavior of the upstream type that we
/// will move to once we update this repository to Polkadot 0.9.43+.
pub struct HashedDescriptionDescribeFamilyAllTerminal<AccountId>(PhantomData<AccountId>);
impl<AccountId: From<[u8; 32]> + Clone> HashedDescriptionDescribeFamilyAllTerminal<AccountId> {
	fn describe_location_suffix(l: &MultiLocation) -> Result<Vec<u8>, ()> {
		match (l.parents, &l.interior) {
			(0, Here) => Ok(Vec::new()),
			(0, X1(PalletInstance(i))) => Ok((b"Pallet", Compact::<u32>::from(*i as u32)).encode()),
			(0, X1(AccountId32 { id, .. })) => Ok((b"AccountId32", id).encode()),
			(0, X1(AccountKey20 { key, .. })) => Ok((b"AccountKey20", key).encode()),
			_ => Err(()),
		}
	}
}

impl<AccountId: From<[u8; 32]> + Clone> Convert<MultiLocation, AccountId>
	for HashedDescriptionDescribeFamilyAllTerminal<AccountId>
{
	fn convert_ref(location: impl Borrow<MultiLocation>) -> Result<AccountId, ()> {
		let l = location.borrow();
		let to_hash = match (l.parents, l.interior.first()) {
			(0, Some(Parachain(index))) => {
				let tail = l.interior.split_first().0;
				let interior = Self::describe_location_suffix(&tail.into())?;
				(b"ChildChain", Compact::<u32>::from(*index), interior).encode()
			}
			(1, Some(Parachain(index))) => {
				let tail = l.interior.split_first().0;
				let interior = Self::describe_location_suffix(&tail.into())?;
				(b"SiblingChain", Compact::<u32>::from(*index), interior).encode()
			}
			(1, _) => {
				let tail = l.interior.into();
				let interior = Self::describe_location_suffix(&tail)?;
				(b"ParentChain", interior).encode()
			}
			_ => return Err(()),
		};
		Ok(blake2_256(&to_hash).into())
	}

	fn reverse_ref(_: impl Borrow<AccountId>) -> Result<MultiLocation, ()> {
		Err(())
	}
}

#[test]
fn test_hashed_family_all_terminal_converter() {
	use super::*;

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
