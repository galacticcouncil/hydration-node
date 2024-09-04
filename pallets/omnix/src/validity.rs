use crate::{Call, Config, Pallet};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::IsSubType;
use orml_traits::GetByKey;
use sp_runtime::traits::{DispatchInfoOf, SignedExtension};
use sp_runtime::transaction_validity::{
	InvalidTransaction, TransactionPriority, TransactionValidity, TransactionValidityError, ValidTransaction,
};
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

impl<T: Config + Send + Sync> SignedExtension for ValidateIceSolution<T>
where
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
{
	const IDENTIFIER: &'static str = "ValidateIceSolution";
	type AccountId = T::AccountId;
	type Call = <T as frame_system::Config>::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();

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
			Some(Call::submit_solution { score, block, .. }) => {
				let (valid, previous_score) = Pallet::<T>::validate_submission(who, *score, *block);
				if !valid {
					log::info!(
							target: "omnix_ext::validate",
							"invalid solution");
					Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(1))) //TODO: custom error?!
				} else {
					ValidTransaction::with_tag_prefix("IceSolutionProposal")
						.and_provides(("solution", *score))
						.priority(*score)
						.longevity(1)
						.propagate(true)
						.build()
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
		self.validate(who, call, info, len).map(|_| ())
	}
}
