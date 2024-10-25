use hydra_dx_math::ratio::Ratio;
use hydradx_traits::Inspect;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT, Trade};
use pallet_ice::traits::{OmnipoolAssetInfo, OmnipoolInfo, Routing};
use primitives::{AssetId, Balance};

pub struct OmnipoolDataProvider<T>(sp_std::marker::PhantomData<T>);

impl<T> OmnipoolInfo<AssetId> for OmnipoolDataProvider<T>
where T: pallet_omnipool::Config + pallet_asset_registry::Config + pallet_dynamic_fees::Config,
    <T as pallet_omnipool::Config>::AssetId: From<AssetId>
   {
    fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>> {
        if let Some(filter_assets) = filter {
            let mut assets = vec![];

            for asset_id in filter_assets {
                let state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_id.into()).unwrap();
                let decimals = pallet_asset_registry::Pallet::<T>::decimals(asset_id).unwrap();
                let details = pallet_asset_registry::Pallet::<T>::assets(asset_id).unwrap(); //TODO: unwraps?!
                let fees = pallet_dynamic_fees::Pallet::<T>::current_fees(asset_id).unwrap();
                assets.push(OmnipoolAssetInfo {
                    asset_id,
                    reserve: state.reserve,
                    hub_reserve: state.hub_reserve,
                    decimals,
                    fee: fees.asset_fee,
                    hub_fee: fees.protocol_fee,
                });
            }
            assets
        } else {
            let mut assets = vec![];
            for (asset_id, state) in pallet_omnipool::Pallet::<T>::omnipool_state() {
                let decimals = pallet_asset_registry::Pallet::<T>::decimals(asset_id).unwrap();
                let details = pallet_asset_registry::Pallet::<T>::assets(asset_id).unwrap();
                let fees = pallet_dynamic_fees::Pallet::<T>::current_fees(asset_id).unwrap();
                assets.push(OmnipoolAssetInfo {
                    asset_id,
                    reserve: state.reserve,
                    hub_reserve: state.hub_reserve,
                    decimals,
                    fee: fees.asset_fee,
                    hub_fee: fees.protocol_fee,
                });
            }
            assets
        }
    }
}

pub struct IceRoutingSupport<R, RP, PP, Origin>(sp_std::marker::PhantomData<(R, RP, PP, Origin)>);

impl<R, RP, PP, Origin> Routing<AssetId> for IceRoutingSupport<R, RP, PP, Origin>
where
    R: RouterT<
        Origin,
        AssetId,
        Balance,
        hydradx_traits::router::Trade<AssetId>,
        hydradx_traits::router::AmountInAndOut<Balance>,
    >,
    RP: RouteProvider<AssetId>,
    PP: PriceProvider<AssetId, Price = Ratio>,
{
    fn get_route(asset_a: AssetId, asset_b: AssetId) -> Vec<Trade<AssetId>> {
        RP::get_route(AssetPair::<AssetId>::new(asset_a, asset_b))
    }
    fn calculate_amount_out(route: &[Trade<AssetId>], amount_in: Balance) -> Result<Balance, ()> {
        let sold = R::calculate_sell_trade_amounts(&route, amount_in).unwrap();
        Ok(sold.last().unwrap().amount_out)
    }
    fn calculate_amount_in(route: &[Trade<AssetId>], amount_out: Balance) -> Result<Balance, ()> {
        let r = R::calculate_buy_trade_amounts(&route, amount_out).unwrap();
        Ok(r.last().unwrap().amount_in)
    }
    // should return price Hub/Asset
    fn hub_asset_price(asset_id: AssetId) -> Result<Ratio, ()> {
        PP::get_price(1u32.into(), asset_id).ok_or(())
    }
}