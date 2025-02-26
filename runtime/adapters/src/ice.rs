use frame_support::defensive;
use frame_support::traits::OriginTrait;
use hydradx_traits::ice::{AmmInfo, AmmState, OmnipoolAsset, OmnipoolState};
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT, TradeExecution};
use hydradx_traits::Inspect;
use pallet_ice::types::Balance;
use primitives::AssetId;
use sp_runtime::{DispatchError, Permill};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

/// Provides state of all AMM pools in Hydration.
pub struct GlobalAmmState<T>(sp_std::marker::PhantomData<T>);

impl<T> AmmState<AssetId> for GlobalAmmState<T>
where
	T: pallet_omnipool::Config<AssetId = AssetId>
		+ pallet_asset_registry::Config<AssetId = AssetId>
		+ pallet_dynamic_fees::Config<Fee = Permill, AssetId = AssetId>,
	<T as pallet_omnipool::Config>::AssetId: From<AssetId>,
	<T as pallet_asset_registry::Config>::AssetId: From<<T as pallet_omnipool::Config>::AssetId> + From<AssetId>,
	AssetId: From<<T as pallet_omnipool::Config>::AssetId>,
{
	fn state<F: Fn(&AssetId) -> bool>(retain: F) -> Vec<AmmInfo<AssetId>> {
		// Get state of omnipool
		let mut assets = vec![];
		for (asset_id, state) in pallet_omnipool::Pallet::<T>::omnipool_state() {
			if !retain(&asset_id) {
				continue;
			}
			let decimals = pallet_asset_registry::Pallet::<T>::decimals(asset_id.into()).unwrap();
			let (asset_fee, hub_fee) = pallet_dynamic_fees::Pallet::<T>::get_fee(asset_id);
			assets.push(OmnipoolAsset {
				asset_id: asset_id.into(),
				reserve: state.reserve,
				hub_reserve: state.hub_reserve,
				decimals,
				fee: asset_fee,
				hub_fee,
			});
		}
		vec![AmmInfo::Omnipool(OmnipoolState { assets })]
		// TODO: add state of all stableswap pools
	}
}

pub struct IceTrader<R, Routing>(sp_std::marker::PhantomData<(R, Routing)>);

impl<R, Routing> pallet_ice::traits::Trader<R::AccountId> for IceTrader<R, Routing>
where
	Routing: RouterT<
			R::RuntimeOrigin,
			AssetId,
			Balance,
			hydradx_traits::router::Trade<AssetId>,
			hydradx_traits::router::AmountInAndOut<Balance>,
		> + RouteProvider<AssetId>,
	R: pallet_ice::Config,
{
	type Outcome = ();

	fn trade(
		account: R::AccountId,
		assets: Vec<(pallet_ice::types::AssetId, (Balance, Balance))>,
	) -> Result<Self::Outcome, DispatchError> {
		let mut delta_in: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut delta_out: BTreeMap<AssetId, Balance> = BTreeMap::new();

		// Calculate deltas to trade
		for (asset_id, (amount_in, amount_out)) in assets.into_iter() {
			if amount_out == amount_in {
				// nothing to trade here, all matched
			} else if amount_out > amount_in {
				// there is something left to buy
				delta_out.insert(asset_id, amount_out - amount_in);
			} else {
				// there is something left to sell
				delta_in.insert(asset_id, amount_in - amount_out);
			}
		}

		loop {
			let Some((asset_out, mut amount_out)) = delta_out.pop_first() else {
				break;
			};
			for (asset_in, amount_in) in delta_in.iter_mut() {
				if *amount_in == 0u128 {
					continue;
				}
				let route = Routing::get_route(AssetPair::new(*asset_in, asset_out));

				// Calculate the amount we can buy with the amount in we have
				let possible_out_amount = Routing::calculate_sell_trade_amounts(&route, *amount_in)?;
				let possible_out_amount = possible_out_amount.last().unwrap().amount_out;

				if possible_out_amount >= amount_out {
					// do exact buy
					let a_in = Routing::calculate_buy_trade_amounts(&route, amount_out)?;
					let a_in = a_in.last().unwrap().amount_in;

					if a_in > *amount_in {
						// this is a bug!
						defensive!("Trading - amount in is less than expected. Bug!");
						return Err(pallet_ice::Error::<R>::MissingPrice.into());
					}

					let origin = R::RuntimeOrigin::signed(account.clone());
					Routing::buy(origin, *asset_in, asset_out, amount_out, a_in, route.to_vec())?;

					*amount_in -= a_in; // this is safe, because of the condition
					amount_out = 0u128;
					//after this, we sorted the asset_out, we can move one
					break;
				} else {
					// do max sell
					let origin = R::RuntimeOrigin::signed(account.clone());
					Routing::sell(
						origin,
						*asset_in,
						asset_out,
						*amount_in,
						possible_out_amount,
						route.to_vec(),
					)?;

					*amount_in = 0u128;
					amount_out -= possible_out_amount; //this is safe, because of the condition
					                    //after this, we need another asset_in to buy the rest
				}
			}
		}
		Ok(())
	}
}
