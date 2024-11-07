use hydra_dx_math::omnipool::types::AssetReserveState;
use pallet_ice::traits::{OmnipoolAssetInfo, OmnipoolInfo};
use pallet_ice::types::{Intent, Swap, SwapType};
use primitives::{AccountId, AssetId, Balance, Moment};
use rand::Rng;
use sp_core::crypto::AccountId32;
use sp_runtime::{FixedPointNumber, FixedU128};

fn to_asset_reserve_state(s: &OmnipoolAssetInfo<AssetId>) -> AssetReserveState<Balance> {
	AssetReserveState {
		reserve: s.reserve,
		hub_reserve: s.hub_reserve,
		..Default::default()
	}
}

pub(crate) fn generate_random_intents(
	c: u32,
	data: Vec<OmnipoolAssetInfo<AssetId>>,
	deadline: Moment,
) -> Vec<Intent<AccountId, AssetId>> {
	let random_pair = || {
		let mut rng = rand::thread_rng();
		loop {
			let idx_in = rng.gen_range(0..data.len());
			let idx_out = rng.gen_range(0..data.len());
			if idx_in == idx_out {
				continue;
			}
			let reserve_in = data[idx_in].reserve;
			let reserve_out = data[idx_out].reserve;
			let amount_in = rng.gen_range(1..reserve_in / 4);

			let s_in = to_asset_reserve_state(&data[idx_in]);
			let s_out = to_asset_reserve_state(&data[idx_out]);
			let r = hydra_dx_math::omnipool::calculate_sell_state_changes(
				&s_in,
				&s_out,
				amount_in,
				data[idx_out].fee,
				data[idx_in].hub_fee,
				0,
			);
			let amount_out = *r.unwrap().asset_out.delta_reserve;
			return (data[idx_in].asset_id, data[idx_out].asset_id, amount_in, amount_out);
		}
	};

	let mut intents = Vec::new();
	for i in 0..c {
		let who: [u8; 32] = [i as u8; 32];
		let (asset_in, asset_out, amount_in, amount_out) = random_pair();
		intents.push(Intent {
			who: who.into(),
			swap: Swap {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				swap_type: SwapType::ExactIn,
			},
			deadline,
			partial: true,
			on_success: None,
			on_failure: None,
		});
	}
	intents
}
