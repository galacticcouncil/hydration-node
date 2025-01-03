use crate::constants::transaction::MaxExtrinsicSize;
use codec::Codec;
use frame_support::BoundedVec;
use sp_weights::Weight;

sp_api::decl_runtime_apis! {
	pub trait FeeEstimationApi<AccountId, AssetId, Balance>
	where
		AccountId: Codec,
		AssetId: Codec,
		Balance: Codec,
	{
		fn estimate_fee_payment(weight: Weight, account_id: AccountId) -> (AssetId, Balance);

		fn estimate_fee_payment_for_extrinsic(
			uxt: BoundedVec<u8, MaxExtrinsicSize>,
			account_id: AccountId
		) -> (AssetId, Balance);
	}
}
