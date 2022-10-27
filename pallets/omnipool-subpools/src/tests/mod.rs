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
// - ensure only origin can create
// - assets must exists in omnipool
// - ensure liquidity is transfered
// - assert correct values in migrated assets
// - ensure assets are removed from ommnipool
// - assert correct state os share asset in omnipool
// - tradable state must be preserved ( TODO: this is still something to be added to stableswap)
// - assert share token cap ( TODO: missing implementation )
// - asset event ( TODO: missing implementation )


// migrate asset:
// - ensure origin
// - asset must exists in omnipool
// - ensure liquidity has been moved from omnipool to subpool - note this account of subpool can change after asset is added )
// - ensure that all previous of all tokens are in correct account ( due to possible change of account id after token is added )
// - same tests as per create subpool here.
// - ensure list of assets is sorted in stableswap pool ( this must be done in stableswap pallet) - few new tests are needed there to test the add asset

// add liquidity:
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
// -
