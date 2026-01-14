use codec::Decode;
use codec::Encode;
use codec::MaxEncodedLen;
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;

pub type Identificator = u128;
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Source {
	ICE(Identificator),
}

pub trait Mutate<AccountId> {
	type Error;
	type BoundedCall;

	// Function queue `call` to be lazylly executed as `origin`
	fn queue(src: Source, origin: AccountId, call: Self::BoundedCall) -> Result<(), Self::Error>;
}
