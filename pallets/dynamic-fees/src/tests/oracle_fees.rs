use crate::tests::mock::*;
use crate::types::FeeEntry;
use sp_runtime::traits::{One, Zero};
use sp_runtime::FixedU128;
use std::cmp::Ordering;

#[test]
pub fn asset_fee_should_be_update_correctly_when_volume_is_increasing() {
	ExtBuilder::default()
		.with_asset_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.with_protocol_fee_params(
			Fee::from_percent(1),
			Fee::from_percent(40),
			FixedU128::zero(),
			FixedU128::one(),
		)
		.build()
		.execute_with(|| {
			crate::AssetFee::<Test>::insert(
				HDX,
				FeeEntry {
					asset_fee: Fee::from_float(0.03),
					protocol_fee: Fee::from_float(0.03),
					timestamp: 0,
				},
			);

			for block in (1..=200).step_by(1) {
				let previous_entry = crate::AssetFee::<Test>::get(HDX).unwrap();
				let current_block = BLOCK.with(|v| *v.borrow());
				let asset_volume = get_oracle_entry(HDX, current_block as u64);
				let (asset_fee, protocol_fee) = retrieve_fee_entry(HDX);

				match asset_volume.amount_out.cmp(&asset_volume.amount_in) {
					Ordering::Less => {
						assert!(previous_entry.asset_fee >= asset_fee);
						assert!(previous_entry.protocol_fee <= protocol_fee);
					}
					Ordering::Equal => {
						assert_eq!(previous_entry.asset_fee, asset_fee);
						assert_eq!(previous_entry.protocol_fee, protocol_fee);
					}
					Ordering::Greater => {
						assert!(previous_entry.asset_fee <= asset_fee);
						assert!(previous_entry.protocol_fee >= protocol_fee);
					}
				}

				System::set_block_number(block);
				BLOCK.with(|v| *v.borrow_mut() = block as usize);
			}
		})
}
