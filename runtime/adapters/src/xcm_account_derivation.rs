use super::*;
use codec::{Compact, Encode};
use sp_io::hashing::blake2_256;
use sp_std::{borrow::Borrow, marker::PhantomData, vec::Vec};
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

impl<AccountId: From<[u8; 32]> + Clone> ConvertLocation<AccountId>
	for HashedDescriptionDescribeFamilyAllTerminal<AccountId>
{
	fn convert_location(location: &MultiLocation) -> Option<AccountId> {
		let l = location.borrow();
		let to_hash = match (l.parents, l.interior.first()) {
			(0, Some(Parachain(index))) => {
				let tail = l.interior.split_first().0;
				let interior = Self::describe_location_suffix(&tail.into()).ok()?;
				(b"ChildChain", Compact::<u32>::from(*index), interior).encode()
			}
			(1, Some(Parachain(index))) => {
				let tail = l.interior.split_first().0;
				let interior = Self::describe_location_suffix(&tail.into()).ok()?;
				(b"SiblingChain", Compact::<u32>::from(*index), interior).encode()
			}
			(1, _) => {
				let tail = l.interior.into();
				let interior = Self::describe_location_suffix(&tail).ok()?;
				(b"ParentChain", interior).encode()
			}
			_ => return None,
		};
		Some(blake2_256(&to_hash).into())
	}
}
