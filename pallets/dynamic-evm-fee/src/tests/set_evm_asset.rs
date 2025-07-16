use crate::tests::mock::DynamicEvmFee;
use crate::tests::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn cannot_be_set_by_normal_user() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			DynamicEvmFee::set_evm_asset(RuntimeOrigin::signed(ALICE), NEW_ETH_ASSET_ID,),
			BadOrigin
		);
	});
}

#[test]
fn can_be_set_by_root() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DynamicEvmFee::set_evm_asset(RuntimeOrigin::root(), NEW_ETH_ASSET_ID),);

		let evm_asset = DynamicEvmFee::evm_asset();
		assert_eq!(evm_asset, Some(NEW_ETH_ASSET_ID));

		expect_events(vec![crate::Event::EvmAssetSet {
			asset_id: NEW_ETH_ASSET_ID,
		}
		.into()]);
	});
}
