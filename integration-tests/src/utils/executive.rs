use codec::Encode;
use primitives::{AccountId, Balance};
use sp_core::Pair;
use sp_runtime::traits::IdentifyAccount;

/// Returns transaction extra.
pub fn signed_extra(nonce: primitives::Index, extra_fee: Balance) -> hydradx_runtime::SignedExtra {
	(
		frame_system::CheckNonZeroSender::new(),
		frame_system::CheckSpecVersion::new(),
		frame_system::CheckTxVersion::new(),
		frame_system::CheckGenesis::new(),
		frame_system::CheckEra::from(sp_runtime::generic::Era::mortal(256, 0)),
		frame_system::CheckNonce::from(nonce),
		frame_system::CheckWeight::new(),
		pallet_transaction_payment::ChargeTransactionPayment::from(extra_fee),
		pallet_claims::ValidateClaim::<hydradx_runtime::Runtime>::new(),
		frame_metadata_hash_extension::CheckMetadataHash::<hydradx_runtime::Runtime>::new(false),
		cumulus_primitives_storage_weight_reclaim::StorageWeightReclaim::<hydradx_runtime::Runtime>::new(),
	)
		.into()
}

pub(crate) fn assert_executive_apply_signed_extrinsic<P: Pair>(call: hydradx_runtime::RuntimeCall, pair: P)
where
	sp_runtime::MultiSigner: From<<P as Pair>::Public>,
	sp_runtime::MultiSignature: From<<P as Pair>::Signature>,
{
	let who: AccountId = sp_runtime::MultiSigner::from(pair.public()).into_account();
	let extra = signed_extra(0, 0);
	let payload = sp_runtime::generic::SignedPayload::new(call.clone(), extra.clone()).unwrap();
	let ecdsa_sig = payload.using_encoded(|e| pair.sign(e));

	let ue = hydradx_runtime::UncheckedExtrinsic::new_signed(
		call,
		who.clone(),
		sp_runtime::MultiSignature::from(ecdsa_sig),
		extra,
	);

	let ae = hydradx_runtime::Executive::apply_extrinsic(ue);
	frame_support::assert_ok!(ae.unwrap());
}

pub(crate) fn assert_executive_apply_unsigned_extrinsic(call: hydradx_runtime::RuntimeCall)
{
	let ue = hydradx_runtime::UncheckedExtrinsic::new_unsigned(call);
	let ae = hydradx_runtime::Executive::apply_extrinsic(ue);
	frame_support::assert_ok!(ae);
}
