mod add_liquidity;
mod add_liquidity_stable;
mod buy;
mod create_subpool;
mod migrate_asset;
pub(crate) mod mock;
mod remove_liquidity;
mod sell;
use mock::*;
mod buy_invariants;
mod create_subpool_invariants;
mod migrate_asset_invariants;
mod sell_invariants;
mod verification;

use frame_support::{assert_err, assert_noop, assert_ok};
use sp_runtime::{FixedU128, Permill};

use hydradx_traits::AccountIdFor;
use orml_traits::MultiCurrency;
use proptest::prelude::Strategy;

pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000; // * 1_000 * 1_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);

fn asset_reserve() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..5000 * ONE
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

fn percent() -> impl Strategy<Value = Permill> {
	(1..100u32).prop_map(Permill::from_percent)
}

fn amplification() -> impl Strategy<Value = u16> {
	2..10_000u16
}

fn pool_token(asset_id: AssetId) -> impl Strategy<Value = PoolToken> {
	(asset_reserve(), price()).prop_map(move |(reserve, price)| PoolToken {
		asset_id,
		amount: reserve,
		price,
	})
}

#[derive(Debug)]
struct PoolToken {
	asset_id: AssetId,
	amount: Balance,
	price: FixedU128,
}

// Tests:
// create subpool:
// X ensure only origin can create
// X assets must exists in omnipool
// X ensure liquidity is transfered
// - assert correct values in migrated assets ( price ,shares, reserve ...  )
// X ensure assets are removed from ommnipool
// x assert correct state os share asset in omnipool
// - tradable state must be preserved ( TODO: this is still something to be added to stableswap)
// - assert share token cap ( TODO: missing implementation )
// - asset event ( TODO: missing implementation )

// migrate asset:
// X ensure origin
// X asset must exists in omnipool
// X ensure liquidity has been moved from omnipool to subpool - note this account of subpool can change after asset is added )
// X ensure that all previous of all tokens are in correct account ( due to possible change of account id after token is added )
// - same tests as per create subpool here.
// - (replace to stablesawp tests as done in omnipool atm) ensure list of assets is sorted in stableswap pool ( this must be done in stableswap pallet) - few new tests are needed there to test the add asset
//
// add liquidity:
// X ensure tradable state is respected - only if add liquidity is allowed for asset ( should be handled by correspoding pallet) but good to test via this interface too
// X -- test wieght cap - should not allow when it is over weigth cap - check omnipool test
// X add liquidity to omnipool asset only
// X add liquidity to subpool
//      X ensure that LP does not have any shares in account ( because add liqudity first deposits shares to LP account and then move them to omnipool)
//      X ensure NFT
//      X assert correct liquiduity in subpool and in omnipool of share asset

// add liquidity with choise : TODO: still to be added
// X when adding liquidity to subpool, user will have a choice to keep the share or deposits it to omnipool and get NFT instead ( previous case )
// X in this case, need to test if he gets shares only and not NFT

// convert position: TODO: missing implemenation
// - it will be possible to convert selected position
// - this scenario happens when LP adds liquidity of asset and only after that, asset is migrated to subpool

// remove liquidity:
// X ensure tradable state is respected - only if remove liquidity is allowed for asset ( should be handled by correspoding pallet) but good to test via this interface too
// - ensure the position data is update correctly - mainly when position has to be migrated within remove liquidity - it should change asset id to share asset id and data recalculated

// buy and sell
// Discuss with Martin if we need more negative test cases, or mainly only just the happy path as the rest is up to the pallets

//Mutation testing
// - execute mutation testing on all extrinsics and based on result, adding additional tests

//Property based testing:
// - See notion: https://www.notion.so/Convert-Omnipool-position-to-Stableswap-Subpool-position-b18dabaa55bf433fa96f4ebf67cecec4

//Integration tests
// - add complex integration tests for creating pool, adding liq, and trading in it

//Questions:
//tradeable asset state - change in omnipool only? In stableswap there is no such thing

// Trades with subpool - ensure withdraw fee is applied

//Prop tests
