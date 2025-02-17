use crate::{Call, Config, Pallet};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::IsSubType;
use sp_runtime::traits::{DispatchInfoOf, PostDispatchInfoOf, SignedExtension};
use sp_runtime::transaction_validity::{
	InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
};
use sp_runtime::DispatchResult;
use sp_std::marker::PhantomData;

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ValidateIceSolution<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for ValidateIceSolution<T> {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "ValidateIceSolution")
	}
}

impl<T: Config + Send + Sync> ValidateIceSolution<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T: Config + Send + Sync> Default for ValidateIceSolution<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Config + Send + Sync> SignedExtension for ValidateIceSolution<T>
where
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
	const IDENTIFIER: &'static str = "ValidateIceSolution";
	type AccountId = T::AccountId;
	type Call = <T as frame_system::Config>::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = Option<T::AccountId>;

	fn additional_signed(&self) -> Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		match call.is_sub_type() {
			Some(Call::submit_solution {
				intents, score, block, ..
			}) => {
				if !Pallet::<T>::ensure_proposal_bond(who) {
					return Err(TransactionValidityError::Invalid(InvalidTransaction::Payment));
					//TODO: custom error?
				}
				let valid = Pallet::<T>::validate_submission(who, intents, *score, *block);
				if valid {
					ValidTransaction::with_tag_prefix("IceSolutionProposal")
						.and_provides(("solution", *score))
						.priority(*score)
						.longevity(1)
						.propagate(true)
						.build()
				} else {
					Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(1))) //TODO: custom error?!
				}
			}
			_ => Ok(Default::default()),
		}
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		match call.is_sub_type() {
			Some(Call::submit_solution { .. }) => self.validate(who, call, info, len).map(|_| Some(who.clone())),
			_ => Ok(None),
		}
	}

	fn post_dispatch(
		pre: Option<Self::Pre>,
		_info: &DispatchInfoOf<Self::Call>,
		_post_info: &PostDispatchInfoOf<Self::Call>,
		_len: usize,
		result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		// if the result is ok, nothing to do
		if result.is_ok() {
			return Ok(());
		}
		// if pre contains None, we dont need to do anything as it was not a submit_solution call
		let Some(maybe_who) = pre else {
			// we cant return error from post dispatch
			return Ok(());
		};
		let Some(who) = maybe_who else {
			// we cant return error from post dispatch
			return Ok(());
		};
		// Now we know that the call was submit_solution and it failed, we can slash the proposer
		let r = Pallet::<T>::slash_bond(&who);

		// if the slashing failed, we cant return error from post dispatch as it would cause block to be invalid
		if r.is_err() {
			log::error!("Failed to slash proposer: {:?}", r.err());
		}
		Ok(())
	}
}
