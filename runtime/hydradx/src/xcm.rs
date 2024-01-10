use super::*;

use codec::MaxEncodedLen;
use hydradx_adapters::{
	MultiCurrencyTrader, RelayChainBlockNumberProvider, ReroutingMultiCurrencyAdapter, ToFeeReceiver,
};
use pallet_transaction_multi_payment::DepositAll;
use primitives::AssetId; // shadow glob import of polkadot_xcm::v3::prelude::AssetId

use cumulus_primitives_core::ParaId;
use frame_support::{
	parameter_types,
	sp_runtime::traits::{AccountIdConversion, Convert},
	traits::{ConstU32, Contains, Everything, Get, Nothing},
	PalletId,
};
use frame_system::EnsureRoot;
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key};
use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiNativeAsset};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::{RelayChainBlockNumber, Sibling};
use polkadot_xcm::v3::{prelude::*, Weight as XcmWeight};
use primitives::Price;
use scale_info::TypeInfo;
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	EnsureXcmOrigin, FixedWeightBounds, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative,
	SiblingParachainConvertsVia, SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation,
	TakeWeightCredit, WithComputedOrigin,
};
use xcm_executor::{Config, XcmExecutor};

#[derive(Debug, Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AssetLocation(pub polkadot_xcm::v3::MultiLocation);

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

pub type Barrier = (
	TakeWeightCredit,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Evaluate the barriers with the effective origin
	WithComputedOrigin<
		(
			AllowTopLevelPaidExecutionFrom<Everything>,
			// Subscriptions for version tracking are OK.
			AllowSubscriptionsFrom<Everything>,
		),
		UniversalLocation,
		ConstU32<8>,
	>,
);

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;

	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();

	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
}

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToCallOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_parts(100_000_000, 0);
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsForTransfer: usize = 2;

	pub UniversalLocation: InteriorMultiLocation = X2(GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into()));
}

pub struct XcmConfig;
impl Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;

	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToCallOrigin;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;

	type IsTeleporter = (); // disabled
	type UniversalLocation = UniversalLocation;

	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
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
		ToFeeReceiver<AccountId, AssetId, Balance, Price, CurrencyIdConvert, DepositAll<Runtime>, TreasuryAccount>,
	>;

	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = ConstU32<64>;
	type FeeManager = ();
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = SafeCallFilter;
	type Aliasers = Nothing;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

parameter_types! {
	pub const MaxDeferredMessages: u32 = 20;
	pub const MaxDeferredBuckets: u32 = 1_000;
	pub const MaxBucketsProcessed: u32 = 3;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = MoreThanHalfTechCommittee;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type PriceForSiblingDelivery = ();
	type WeightInfo = weights::xcmp_queue::HydraWeight<Runtime>;
	type ExecuteDeferredOrigin = EnsureRoot<AccountId>;
	type MaxDeferredMessages = MaxDeferredMessages;
	type MaxDeferredBuckets = MaxDeferredBuckets;
	type MaxBucketsProcessed = MaxBucketsProcessed;
	type RelayChainBlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
	type XcmDeferFilter = XcmRateLimiter;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

const ASSET_HUB_PARA_ID: u32 = 1000;

parameter_type_with_key! {
	pub ParachainMinFee: |location: MultiLocation| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(ASSET_HUB_PARA_ID))) => Some(50_000_000),
			_ => None,
		}
	};
}

impl orml_xtokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
	type MinXcmFee = ParachainMinFee;
	type UniversalLocation = UniversalLocation;
}

impl orml_unknown_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = MoreThanHalfCouncil;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type TrustedLockers = ();
	type SovereignAccountOf = ();
	type MaxLockers = ConstU32<8>;
	type WeightInfo = weights::xcm::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
	type AdminOrigin = MajorityOfCouncil;
	type MaxRemoteLockConsumers = ConstU32<0>;
	type RemoteLockConsumerIdentifier = ();
}

#[test]
fn defer_duration_configuration() {
	use sp_runtime::{traits::One, FixedPointNumber, FixedU128};
	/// Calculate the configuration value for the defer duration based on the desired defer duration and
	/// the threshold percentage when to start deferring.
	/// - `defer_by`: the desired defer duration when reaching the rate limit
	/// - `a``: the fraction of the rate limit where we start deferring, e.g. 0.9
	fn defer_duration(defer_by: u32, a: FixedU128) -> u32 {
		assert!(a < FixedU128::one());
		// defer_by * a / (1 - a)
		(FixedU128::one() / (FixedU128::one() - a)).saturating_mul_int(a.saturating_mul_int(defer_by))
	}
	assert_eq!(
		defer_duration(600 * 4, FixedU128::from_rational(9, 10)),
		DeferDuration::get()
	);
}
parameter_types! {
	pub DeferDuration: RelayChainBlockNumber = 600 * 36; // 36 hours
	pub MaxDeferDuration: RelayChainBlockNumber = 600 * 24 * 10; // 10 days
}

impl pallet_xcm_rate_limiter::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type DeferDuration = DeferDuration;
	type MaxDeferDuration = MaxDeferDuration;
	type RelayBlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
	type CurrencyIdConvert = CurrencyIdConvert;
	type RateLimitFor = pallet_asset_registry::XcmRateLimitsInRegistry<Runtime>;
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
			_ => AssetRegistry::asset_to_location(id).map(|loc| loc.0),
		}
	}
}

impl Convert<MultiLocation, Option<AssetId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<AssetId> {
		match location {
			MultiLocation {
				parents,
				interior: X2(Parachain(id), GeneralIndex(index)),
			} if parents == 1 && ParaId::from(id) == ParachainInfo::get() && (index as u32) == CORE_ASSET_ID => {
				// Handling native asset for this parachain
				Some(CORE_ASSET_ID)
			}
			// handle reanchor canonical location: https://github.com/paritytech/polkadot/pull/4470
			MultiLocation {
				parents: 0,
				interior: X1(GeneralIndex(index)),
			} if (index as u32) == CORE_ASSET_ID => Some(CORE_ASSET_ID),
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
			network: None,
			id: account.into(),
		})
		.into()
	}
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
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

pub struct OmnipoolProtocolAccount;
impl Contains<(AssetId, AccountId)> for OmnipoolProtocolAccount {
	fn contains((c, account_id): &(AssetId, AccountId)) -> bool {
		&Omnipool::protocol_account() == account_id && Omnipool::exists(*c)
	}
}

/// We use `orml::Currencies` for asset transacting. Transfers to active Omnipool accounts are rerouted to the treasury.
pub type LocalAssetTransactor = ReroutingMultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsNativeConcrete<AssetId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	AssetId,
	CurrencyIdConvert,
	DepositToAlternative<Alternative, Currencies, AssetId, AccountId, Balance>,
	OmnipoolProtocolAccount,
	TreasuryAccount,
>;

/// A call filter for the XCM Transact instruction. This is a temporary measure until we properly
/// account for proof size weights.
///
/// Calls that are allowed through this filter must:
/// 1. Have a fixed weight;
/// 2. Cannot lead to another call being made;
/// 3. Have a defined proof size weight, e.g. no unbounded vecs in call parameters.
pub struct SafeCallFilter;
impl Contains<RuntimeCall> for SafeCallFilter {
	fn contains(call: &RuntimeCall) -> bool {
		#[cfg(feature = "runtime-benchmarks")]
		{
			if matches!(call, RuntimeCall::System(frame_system::Call::remark_with_event { .. })) {
				return true;
			}
		}

		// check the runtime call filter
		if !CallFilter::contains(call) {
			return false;
		}

		matches!(
			call,
			RuntimeCall::System(frame_system::Call::kill_prefix { .. } | frame_system::Call::set_heap_pages { .. })
				| RuntimeCall::Timestamp(..)
				| RuntimeCall::Balances(..)
				| RuntimeCall::Treasury(..)
				| RuntimeCall::Utility(pallet_utility::Call::as_derivative { .. })
				| RuntimeCall::Vesting(..)
				| RuntimeCall::Proxy(..)
				| RuntimeCall::CollatorSelection(
					pallet_collator_selection::Call::set_desired_candidates { .. }
						| pallet_collator_selection::Call::set_candidacy_bond { .. }
						| pallet_collator_selection::Call::register_as_candidate { .. }
						| pallet_collator_selection::Call::leave_intent { .. },
				) | RuntimeCall::Session(pallet_session::Call::purge_keys { .. })
				| RuntimeCall::Uniques(
					pallet_uniques::Call::create { .. }
						| pallet_uniques::Call::force_create { .. }
						| pallet_uniques::Call::mint { .. }
						| pallet_uniques::Call::burn { .. }
						| pallet_uniques::Call::transfer { .. }
						| pallet_uniques::Call::freeze { .. }
						| pallet_uniques::Call::thaw { .. }
						| pallet_uniques::Call::freeze_collection { .. }
						| pallet_uniques::Call::thaw_collection { .. }
						| pallet_uniques::Call::transfer_ownership { .. }
						| pallet_uniques::Call::set_team { .. }
						| pallet_uniques::Call::approve_transfer { .. }
						| pallet_uniques::Call::cancel_approval { .. }
						| pallet_uniques::Call::force_item_status { .. }
						| pallet_uniques::Call::set_attribute { .. }
						| pallet_uniques::Call::clear_attribute { .. }
						| pallet_uniques::Call::set_metadata { .. }
						| pallet_uniques::Call::clear_metadata { .. }
						| pallet_uniques::Call::set_collection_metadata { .. }
						| pallet_uniques::Call::clear_collection_metadata { .. }
						| pallet_uniques::Call::set_accept_ownership { .. }
						| pallet_uniques::Call::set_price { .. }
						| pallet_uniques::Call::buy_item { .. },
				) | RuntimeCall::Identity(
				pallet_identity::Call::add_registrar { .. }
					| pallet_identity::Call::set_identity { .. }
					| pallet_identity::Call::clear_identity { .. }
					| pallet_identity::Call::request_judgement { .. }
					| pallet_identity::Call::cancel_request { .. }
					| pallet_identity::Call::set_fee { .. }
					| pallet_identity::Call::set_account_id { .. }
					| pallet_identity::Call::set_fields { .. }
					| pallet_identity::Call::provide_judgement { .. }
					| pallet_identity::Call::kill_identity { .. }
					| pallet_identity::Call::add_sub { .. }
					| pallet_identity::Call::rename_sub { .. }
					| pallet_identity::Call::remove_sub { .. }
					| pallet_identity::Call::quit_sub { .. },
			) | RuntimeCall::Omnipool(..)
				| RuntimeCall::OmnipoolLiquidityMining(..)
				| RuntimeCall::OTC(..)
				| RuntimeCall::CircuitBreaker(..)
				| RuntimeCall::Router(..)
				| RuntimeCall::DCA(..)
				| RuntimeCall::MultiTransactionPayment(..)
				| RuntimeCall::Currencies(..)
				| RuntimeCall::Tokens(..)
				| RuntimeCall::OrmlXcm(..)
		)
	}
}
