mod add_liquidity;
mod create_subpool;
mod migrate_asset;
pub(crate) mod mock;

use mock::*;

use frame_support::{assert_err, assert_noop, assert_ok};
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128, Permill};

use hydradx_traits::AccountIdFor;
use orml_traits::MultiCurrency;

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
// - ensure tradable state is respected - only if add liquidity is allowed for asset ( should be handled by correspoding pallet) but good to test via this interface too
// -- test wieght cap - should not allow when it is over weigth cap
// - add liquidity to omnipool asset only
// - add liquidity to subpool
//      - ensure that LP does not have any shares in account ( because add liqudity first deposits shares to LP account and then move them to omnipool)
//      - ensure NFT
//      - assert correct liquiduity in subpool and in omnipool of share asset

// add liquidity with choise : TODO: still to be added
// - when adding liquidity to subpool, user will have a choice to keep the share or deposits it to omnipool and get NFT instead ( previous case )
// - in this case, need to test if he gets shares only and not NFT

// convert position: TODO: missing implemenation
// - it will be possible to convert selected position
// - this scenario happens when LP adds liquidity of asset and only after that, asset is migrated to subpool

// remove liquidity:
// - ensure tradable state is respected - only if remove liquidity is allowed for asset ( should be handled by correspoding pallet) but good to test via this interface too
// - ensure the position data is update correctly - mainly when position has to be migrated within remove liquidity - it should change asset id to share asset id and data recalculated

//Mutation testing
// - execute mutation testing on all extrinsics and based on result, adding additional tests

//Property based testing:
// - See notion: https://www.notion.so/Convert-Omnipool-position-to-Stableswap-Subpool-position-b18dabaa55bf433fa96f4ebf67cecec4

//Integration tests
// - add complex integration tests for creating pool, adding liq, and trading in it
