use super::{AssetId, *};

use codec::Decode;
use cumulus_primitives_core::ParaId;
use frame_support::{
	traits::{Everything, Nothing},
	PalletId,
};
use hydradx_adapters::{MultiCurrencyTrader, ToFeeReceiver};
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key};
pub use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use polkadot_xcm::latest::prelude::*;
use primitives::Price;
use sp_runtime::traits::{AccountIdConversion, Convert};
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	EnsureXcmOrigin, FixedWeightBounds, LocationInverter, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative,
	SiblingParachainConvertsVia, SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation,
	TakeWeightCredit,
};
use xcm_executor::{Config, XcmExecutor};

use polkadot_xcm::latest::Weight;

pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

pub type Barrier = (
	TakeWeightCredit,
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Subscriptions for version tracking are OK.
	AllowSubscriptionsFrom<Everything>,
);

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;

	pub RelayChainOrigin: Origin = cumulus_pallet_xcm::Origin::Relay.into();

	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
}

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToCallOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, Origin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, Origin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, Origin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, Origin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<Origin>,
);

parameter_types! {
	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: Weight = 100_000_000;
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsForTransfer: usize = 2;
}

pub struct XcmConfig;
impl Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmRouter;

	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToCallOrigin;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;

	type IsTeleporter = (); // disabled
	type LocationInverter = LocationInverter<Ancestry>;

	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	// We calculate weight fees the same way as for regular extrinsics and use the prices and choice
	// of accepted currencies of the transaction payment pallet. Fees go to the same fee receiver as
	// configured in `MultiTransactionPayment`.
	type Trader = MultiCurrencyTrader<
		AssetId,
		Balance,
		Price,
		WeightToFee,
		MultiTransactionPayment,
		CurrencyIdConvert,
		ToFeeReceiver<
			AccountId,
			AssetId,
			Balance,
			Price,
			CurrencyIdConvert,
			DepositAll<Runtime>,
			MultiTransactionPayment,
		>,
	>;

	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = MoreThanHalfTechCommittee;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type WeightInfo = ();
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

parameter_type_with_key! {
	pub ParachainMinFee: |_location: MultiLocation| -> Option<u128> {
		None
	};
}

impl orml_xtokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type LocationInverter = LocationInverter<Ancestry>;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
	type MinXcmFee = ParachainMinFee;
}

impl orml_unknown_tokens::Config for Runtime {
	type Event = Event;
}

impl orml_xcm::Config for Runtime {
	type Event = Event;
	type SovereignOrigin = MoreThanHalfCouncil;
}

impl pallet_xcm::Config for Runtime {
	type Event = Event;
	type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmExecuteFilter = Everything;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
	type LocationInverter = LocationInverter<Ancestry>;
	type Origin = Origin;
	type Call = Call;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

pub struct CurrencyIdConvert;
use primitives::constants::chain::CORE_ASSET_ID;

impl Convert<AssetId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: AssetId) -> Option<MultiLocation> {
		match id {
			CORE_ASSET_ID => Some(MultiLocation::new(
				1,
				X2(Parachain(ParachainInfo::get().into()), GeneralIndex(id.into())),
			)),
			_ => {
				if let Some(loc) = AssetRegistry::asset_to_location(id) {
					Some(loc.0)
				} else {
					None
				}
			}
		}
	}
}

impl Convert<MultiLocation, Option<AssetId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<AssetId> {
		match location {
			MultiLocation {
				parents,
				interior: X2(Parachain(id), GeneralKey(key)),
			} if parents == 1 && ParaId::from(id) == ParachainInfo::get() => {
				// Handling native asset for this parachain
				if let Ok(currency_id) = AssetId::decode(&mut &key[..]) {
					// we currently have only one native asset
					match currency_id {
						CORE_ASSET_ID => Some(currency_id),
						_ => None,
					}
				} else {
					None
				}
			}
			// handle reanchor canonical location: https://github.com/paritytech/polkadot/pull/4470
			MultiLocation {
				parents: 0,
				interior: X1(GeneralKey(key)),
			} => {
				if let Ok(currency_id) = AssetId::decode(&mut &key[..]) {
					// we currently have only one native asset
					match currency_id {
						CORE_ASSET_ID => Some(currency_id),
						_ => None,
					}
				} else {
					None
				}
			}
			// delegate to asset-registry
			_ => AssetRegistry::location_to_asset(AssetLocation(location)),
		}
	}
}

impl Convert<MultiAsset, Option<AssetId>> for CurrencyIdConvert {
	fn convert(asset: MultiAsset) -> Option<AssetId> {
		if let MultiAsset {
			id: Concrete(location), ..
		} = asset
		{
			Self::convert(location)
		} else {
			None
		}
	}
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(AccountId32 {
			network: Any,
			id: account.into(),
		})
		.into()
	}
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, ()>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
);

parameter_types! {
	// The account which receives multi-currency tokens from failed attempts to deposit them
	pub Alternative: AccountId = PalletId(*b"xcm/alte").into_account_truncating();
}

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsNativeConcrete<AssetId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	AssetId,
	CurrencyIdConvert,
	DepositToAlternative<Alternative, Currencies, AssetId, AccountId, Balance>,
>;
