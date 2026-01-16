use super::*;

use crate::origins::GeneralAdmin;
use sp_std::marker::PhantomData;

use codec::MaxEncodedLen;
use hydradx_adapters::{MultiCurrencyTrader, ReroutingMultiCurrencyAdapter, ToFeeReceiver};
use pallet_transaction_multi_payment::DepositAll;
use primitives::{AssetId, Price};

use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
	parameter_types,
	sp_runtime::traits::{AccountIdConversion, Convert},
	traits::{ConstU32, Contains, ContainsPair, EitherOf, Everything, Get, Nothing, TransformOrigin},
	PalletId,
};
use frame_system::unique;
use frame_system::EnsureRoot;
use hydradx_adapters::xcm_exchange::XcmAssetExchanger;
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key};
use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiNativeAsset};
use pallet_evm::AddressMapping;
pub use pallet_xcm::GenesisConfig as XcmGenesisConfig;
use pallet_xcm::XcmPassthrough;
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use polkadot_parachain::primitives::Sibling;
use polkadot_xcm::v3::MultiLocation;
use polkadot_xcm::v4::{prelude::*, Asset, InteriorLocation, Weight as XcmWeight};
use scale_info::TypeInfo;
use sp_runtime::{traits::MaybeEquivalence, Perbill};
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	DescribeAllTerminal, DescribeFamily, EnsureXcmOrigin, GlobalConsensusConvertsFor, HashedDescription,
	ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId,
	WeightInfoBounds, WithComputedOrigin, WithUniqueTopic,
};
use xcm_executor::{Config, XcmExecutor};

#[derive(Debug, Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AssetLocation(pub polkadot_xcm::v3::Location);

impl From<AssetLocation> for Option<Location> {
	fn from(location: AssetLocation) -> Option<Location> {
		xcm_builder::WithLatestLocationConverter::convert_back(&location.0)
	}
}

impl From<AssetLocation> for MultiLocation {
	fn from(location: AssetLocation) -> Self {
		location.0
	}
}

impl TryFrom<Location> for AssetLocation {
	type Error = ();

	fn try_from(value: Location) -> Result<Self, Self::Error> {
		let loc: MultiLocation = value.try_into()?;
		Ok(AssetLocation(loc))
	}
}

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

pub type Barrier = TrailingSetTopicAsId<(
	TakeWeightCredit,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Evaluate the barriers with the effective origin
	WithComputedOrigin<
		(
			remove_when_updating_to_stable2412::AllowTopLevelPaidExecutionFrom<Everything>,
			// Subscriptions for version tracking are OK.
			AllowSubscriptionsFrom<Everything>,
		),
		UniversalLocation,
		ConstU32<8>,
	>,
)>;

parameter_types! {
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

use sp_std::sync::Arc;
parameter_types! {
	pub SelfLocation: Location = Location::new(1, cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(ParachainInfo::get().into());1])));
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;

	pub const PolkadotLocation: Location = Location::parent();


	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();

	pub Ancestry: Location = Parachain(ParachainInfo::parachain_id().into()).into();
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

	pub const MaxAssetsIntoHolding: u32 = 64;

	pub TempAccountForXcmAssetExchange: AccountId = [42; 32].into();
	pub const MaxXcmDepth: u16 = 5;
	pub const MaxNumberOfInstructions: u16 = 100;

	pub UniversalLocation: InteriorLocation = [GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into())].into();
	pub AssetHubLocation: Location = (Parent, Parachain(ASSET_HUB_PARA_ID)).into();
}

/// Matches foreign assets from a given origin.
/// Foreign assets are assets bridged from other consensus systems. i.e parents > 1.
pub struct IsForeignNativeAssetFrom<Origin>(PhantomData<Origin>);
impl<Origin> ContainsPair<Asset, Location> for IsForeignNativeAssetFrom<Origin>
where
	Origin: Get<Location>,
{
	fn contains(asset: &Asset, origin: &Location) -> bool {
		let loc = Origin::get();
		&loc == origin
			&& matches!(
				asset,
				Asset {
					id: AssetId(Location { parents: 2, .. }),
					fun: Fungible(_),
				},
			)
	}
}

pub struct IsDotFrom<Origin>(PhantomData<Origin>);
impl<Origin> ContainsPair<Asset, Location> for IsDotFrom<Origin>
where
	Origin: Get<Location>,
{
	fn contains(asset: &Asset, origin: &Location) -> bool {
		let loc = Origin::get();
		&loc == origin
			&& matches!(
				asset,
				Asset {
					id: AssetId(Location {
						parents: 1,
						interior: Here
					}),
					fun: Fungible(_),
				},
			)
	}
}

pub type Reserves = (
	IsDotFrom<AssetHubLocation>,
	IsForeignNativeAssetFrom<AssetHubLocation>,
	MultiNativeAsset<AbsoluteReserveProvider>,
);

pub type DynamicWeigher<RuntimeCall> =
	WeightInfoBounds<crate::weights::xcm::HydraXcmWeight<RuntimeCall>, RuntimeCall, MaxInstructions>;

// Types that exist in `xcm_builder` from `stable2412` onwards.
mod remove_when_updating_to_stable2412 {
	use core::marker::PhantomData;
	use frame_support::{
		ensure,
		traits::{Contains, ContainsPair, Get, ProcessMessageError},
	};
	use polkadot_xcm::prelude::*;
	use xcm_builder::{CreateMatcher, MatchXcm};
	use xcm_executor::traits::{Properties, ShouldExecute};

	/// Alias a descendant location of the original origin.
	pub struct AliasChildLocation;
	impl ContainsPair<Location, Location> for AliasChildLocation {
		fn contains(origin: &Location, target: &Location) -> bool {
			return target.starts_with(origin);
		}
	}

	/// Alias a location if it passes `Filter` and the original origin is root of `Origin`.
	///
	/// This can be used to allow (trusted) system chains root to alias into other locations.
	/// **Warning**: do not use with untrusted `Origin` chains.
	pub struct AliasOriginRootUsingFilter<Origin, Filter>(PhantomData<(Origin, Filter)>);
	impl<Origin, Filter> ContainsPair<Location, Location> for AliasOriginRootUsingFilter<Origin, Filter>
	where
		Origin: Get<Location>,
		Filter: Contains<Location>,
	{
		fn contains(origin: &Location, target: &Location) -> bool {
			// check that `origin` is a root location
			match origin.unpack() {
				(1, [Parachain(_)]) | (2, [GlobalConsensus(_)]) | (2, [GlobalConsensus(_), Parachain(_)]) => (),
				_ => return false,
			};
			// check that `origin` matches `Origin` and `target` matches `Filter`
			return Origin::get().eq(origin) && Filter::contains(target);
		}
	}

	const MAX_ASSETS_FOR_BUY_EXECUTION: usize = 2;

	/// Allows execution from `origin` if it is contained in `T` (i.e. `T::Contains(origin)`) taking
	/// payments into account.
	///
	/// Only allows for `WithdrawAsset`, `ReceiveTeleportedAsset`, `ReserveAssetDeposited` and
	/// `ClaimAsset` XCMs because they are the only ones that place assets in the Holding Register to
	/// pay for execution.
	pub struct AllowTopLevelPaidExecutionFrom<T>(PhantomData<T>);
	impl<T: Contains<Location>> ShouldExecute for AllowTopLevelPaidExecutionFrom<T> {
		fn should_execute<RuntimeCall>(
			origin: &Location,
			instructions: &mut [Instruction<RuntimeCall>],
			max_weight: Weight,
			properties: &mut Properties,
		) -> Result<(), ProcessMessageError> {
			log::trace!(
				target: "xcm::barriers",
				"AllowTopLevelPaidExecutionFrom. origin: {:?}, instructions: {:?}, max_weight: {:?}, properties: {:?}",
				origin,
				instructions,
				max_weight,
				properties,
			);

			ensure!(T::contains(origin), ProcessMessageError::Unsupported);
			// We will read up to 5 instructions. This allows up to 3 `ClearOrigin` instructions. We
			// allow for more than one since anything beyond the first is a no-op and it's conceivable
			// that composition of operations might result in more than one being appended.
			let end = instructions.len().min(5);
			instructions[..end]
				.matcher()
				.match_next_inst(|inst| match inst {
					WithdrawAsset(ref assets)
					| ReceiveTeleportedAsset(ref assets)
					| ReserveAssetDeposited(ref assets)
					| ClaimAsset { ref assets, .. } => {
						if assets.len() <= MAX_ASSETS_FOR_BUY_EXECUTION {
							Ok(())
						} else {
							Err(ProcessMessageError::BadFormat)
						}
					}
					_ => Err(ProcessMessageError::BadFormat),
				})?
				.skip_inst_while(|inst| {
					matches!(inst, ClearOrigin | AliasOrigin(..))
						|| matches!(inst, DescendOrigin(child) if *child != Here)
				})?
				.match_next_inst(|inst| match inst {
					BuyExecution {
						weight_limit: Limited(ref mut weight),
						..
					} if weight.all_gte(max_weight) => {
						*weight = max_weight;
						Ok(())
					}
					BuyExecution {
						ref mut weight_limit, ..
					} if weight_limit == &Unlimited => {
						*weight_limit = Limited(max_weight);
						Ok(())
					}
					_ => Err(ProcessMessageError::Overweight(max_weight)),
				})?;
			Ok(())
		}
	}
}

pub struct RestrictedAssetHubAliases;
impl Contains<Location> for RestrictedAssetHubAliases {
	fn contains(target: &Location) -> bool {
		match target.unpack() {
			// Allow system parachains under the Polkadot relay
			(1, [Parachain(id)]) if id < &2000 => true,

			// Allow Kusama relay itself
			(2, [GlobalConsensus(Kusama)]) => true,

			// Allow Kusama Asset Hub and all its descendants
			(2, [GlobalConsensus(Kusama), Parachain(1000), ..]) => true,

			// Allow Ethereum consensus and all its descendants
			(2, [GlobalConsensus(NetworkId::Ethereum { .. }), ..]) => true,

			// Everything else disallowed
			_ => false,
		}
	}
}

/// Rules for allowing the usage of `AliasOrigin`.
pub type Aliasers = (
	// Anyone can alias an interior location, same as `DescendOrigin`.
	remove_when_updating_to_stable2412::AliasChildLocation,
	// Asset Hub root can alias system chains, Ethereum and Kusama
	remove_when_updating_to_stable2412::AliasOriginRootUsingFilter<AssetHubLocation, RestrictedAssetHubAliases>,
);

pub struct XcmConfig;
impl Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;

	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToCallOrigin;
	type IsReserve = Reserves;

	type IsTeleporter = (); // disabled
	type UniversalLocation = UniversalLocation;

	type Barrier = Barrier;
	type Weigher = DynamicWeigher<RuntimeCall>;
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
	type AssetExchanger = XcmAssetExchanger<Runtime, TempAccountForXcmAssetExchange, CurrencyIdConvert, Currencies>;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
	type FeeManager = ();
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = Everything;
	type Aliasers = Aliasers;
	type TransactionalProcessor = xcm_builder::FrameTransactionalProcessor;
	type HrmpNewChannelOpenRequestHandler = ();
	type HrmpChannelClosingHandler = ();
	type HrmpChannelAcceptedHandler = ();
	type XcmRecorder = PolkadotXcm;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = WithUnifiedEventSupport<XcmExecutor<XcmConfig>>;
}

pub struct WithUnifiedEventSupport<Inner>(PhantomData<Inner>);

impl<Inner: ExecuteXcm<<XcmConfig as Config>::RuntimeCall>> ExecuteXcm<<XcmConfig as Config>::RuntimeCall>
	for WithUnifiedEventSupport<Inner>
{
	type Prepared = <Inner as cumulus_primitives_core::ExecuteXcm<RuntimeCall>>::Prepared;

	fn prepare(
		message: Xcm<<XcmConfig as Config>::RuntimeCall>,
	) -> Result<Self::Prepared, Xcm<<XcmConfig as Config>::RuntimeCall>> {
		//We populate the context in `prepare` as we have the xcm message at this point so we can get the unique topic id
		let unique_id = if let Some(SetTopic(id)) = message.last() {
			*id
		} else {
			unique(&message)
		};
		if pallet_broadcast::Pallet::<Runtime>::add_to_context(|event_id| ExecutionType::Xcm(unique_id, event_id))
			.is_err()
		{
			log::error!(target: "xcm-executor", "Failed to add to broadcast context.");
			return Err(message.clone());
		}

		let prepare_result = Inner::prepare(message.clone());

		//In case of error we need to clean context as xcm execution won't happen
		if prepare_result.is_err() && pallet_broadcast::Pallet::<Runtime>::remove_from_context().is_err() {
			log::error!(target: "xcm-executor", "Failed to remove from broadcast context.");
			return Err(message);
		}

		prepare_result
	}

	fn execute(
		origin: impl Into<Location>,
		pre: Self::Prepared,
		id: &mut XcmHash,
		weight_credit: XcmWeight,
	) -> Outcome {
		let outcome = Inner::execute(origin, pre, id, weight_credit);

		// Context was added to the stack in `prepare` call.
		if pallet_broadcast::Pallet::<Runtime>::remove_from_context().is_err() {
			return Outcome::Error {
				error: XcmError::FailedToTransactAsset("Unexpected error at modifying broadcast execution stack"),
			};
		};

		outcome
	}

	fn charge_fees(location: impl Into<Location>, fees: Assets) -> XcmResult {
		Inner::charge_fees(location, fees)
	}
}

impl<Inner: ExecuteXcm<<XcmConfig as Config>::RuntimeCall>> XcmAssetTransfers for WithUnifiedEventSupport<Inner> {
	type IsReserve = <XcmConfig as Config>::IsReserve;
	type IsTeleporter = <XcmConfig as Config>::IsTeleporter;
	type AssetTransactor = <XcmConfig as Config>::AssetTransactor;
}

parameter_types! {
	pub const MaxInboundSuspended: u32 = 1_000;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxInboundSuspended = MaxInboundSuspended;
	type MaxActiveOutboundChannels = ConstU32<128>;
	type MaxPageSize = ConstU32<{ 128 * 1024 }>;
	type ControllerOrigin = EitherOf<EnsureRoot<Self::AccountId>, EitherOf<TechCommitteeSuperMajority, GeneralAdmin>>;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type PriceForSiblingDelivery = polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery<ParaId>;
	type WeightInfo = weights::cumulus_pallet_xcmp_queue::HydraWeight<Runtime>;
}

const ASSET_HUB_PARA_ID: u32 = 1000;

parameter_type_with_key! {
	pub ParachainMinFee: |location: Location| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(ASSET_HUB_PARA_ID))) => Some(150_000_000),
			_ => None,
		}
	};
}

impl orml_xtokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = WithUnifiedEventSupport<XcmExecutor<XcmConfig>>;
	type Weigher = DynamicWeigher<RuntimeCall>;
	type BaseXcmWeight = BaseXcmWeight;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type LocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
	type MinXcmFee = ParachainMinFee;
	type UniversalLocation = UniversalLocation;
	type RateLimiter = (); // do not use rate limiter
	type RateLimiterId = ();
}

impl orml_unknown_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = EnsureRoot<Self::AccountId>;
}

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = circuit_breaker::XcmLockdownFilter;
	type XcmExecutor = WithUnifiedEventSupport<XcmExecutor<XcmConfig>>;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = circuit_breaker::CircuitBreakerReserveTransferFilter<Everything>;
	type Weigher = DynamicWeigher<RuntimeCall>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type TrustedLockers = ();
	type SovereignAccountOf = ();
	type MaxLockers = ConstU32<8>;
	type WeightInfo = weights::pallet_xcm::HydraWeight<Runtime>;
	type AdminOrigin = EitherOf<EnsureRoot<Self::AccountId>, EitherOf<TechCommitteeSuperMajority, GeneralAdmin>>;
	type MaxRemoteLockConsumers = ConstU32<0>;
	type RemoteLockConsumerIdentifier = ();
}

parameter_types! {
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(20) * BlockWeights::get().max_block;
	pub const MessageQueueMaxStale: u32 = 8;
	pub const MessageQueueHeapSize: u32 = 128 * 1048;
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_message_queue::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor =
		pallet_message_queue::mock_helpers::NoopMessageProcessor<cumulus_primitives_core::AggregateMessageOrigin>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		WithUnifiedEventSupport<XcmExecutor<XcmConfig>>,
		RuntimeCall,
	>;
	type Size = u32;
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = MessageQueueHeapSize;
	type MaxStale = MessageQueueMaxStale;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = ();
}

pub struct CurrencyIdConvert;
use crate::evm::ExtendedAddressMapping;
use primitives::constants::chain::{CORE_ASSET_ID, HOLLAR_ASSET_ID};

impl Convert<AssetId, Option<Location>> for CurrencyIdConvert {
	fn convert(id: AssetId) -> Option<Location> {
		match id {
			CORE_ASSET_ID => Some(Location {
				parents: 1,
				interior: [Parachain(ParachainInfo::get().into()), GeneralIndex(id.into())].into(),
			}),
			HOLLAR_ASSET_ID => Some(Location {
				parents: 1,
				interior: [Parachain(ParachainInfo::get().into()), GeneralIndex(id.into())].into(),
			}),
			_ => {
				let loc = AssetRegistry::asset_to_location(id);
				if let Some(location) = loc {
					location.into()
				} else {
					None
				}
			}
		}
	}
}

impl Convert<Location, Option<AssetId>> for CurrencyIdConvert {
	fn convert(location: Location) -> Option<AssetId> {
		let Location { parents, interior } = location.clone();

		match interior {
			Junctions::X2(a)
				if parents == 1
					&& a.contains(&GeneralIndex(CORE_ASSET_ID.into()))
					&& a.contains(&Parachain(ParachainInfo::get().into())) =>
			{
				Some(CORE_ASSET_ID)
			}
			Junctions::X2(a)
				if parents == 1
					&& a.contains(&GeneralIndex(HOLLAR_ASSET_ID.into()))
					&& a.contains(&Parachain(ParachainInfo::get().into())) =>
			{
				Some(HOLLAR_ASSET_ID)
			}
			Junctions::X1(a) if parents == 0 && a.contains(&GeneralIndex(CORE_ASSET_ID.into())) => Some(CORE_ASSET_ID),
			Junctions::X1(a) if parents == 0 && a.contains(&GeneralIndex(HOLLAR_ASSET_ID.into())) => {
				Some(HOLLAR_ASSET_ID)
			}
			_ => {
				let location: Option<AssetLocation> = location.try_into().ok();
				if let Some(location) = location {
					AssetRegistry::location_to_asset(location)
				} else {
					None
				}
			}
		}
	}
}

impl Convert<Asset, Option<AssetId>> for CurrencyIdConvert {
	fn convert(asset: Asset) -> Option<AssetId> {
		Self::convert(asset.id.0)
	}
}

impl Convert<VersionedLocation, Option<AssetId>> for CurrencyIdConvert {
	fn convert(versioned_location: VersionedLocation) -> Option<AssetId> {
		let location = Location::try_from(versioned_location).ok()?;

		Self::convert(location)
	}
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, Location> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> Location {
		[AccountId32 {
			network: None,
			id: account.into(),
		}]
		.into()
	}
}

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = WithUniqueTopic<(
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
)>;

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
	// Generate remote accounts according to polkadot standards
	HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
	// Convert ETH to local substrate account
	EvmAddressConversion<RelayNetwork>,
	// Converts a location which is a top-level relay chain (which provides its own consensus) into a
	// 32-byte `AccountId`.
	GlobalConsensusConvertsFor<UniversalLocation, AccountId>,
);
use pallet_broadcast::types::ExecutionType;
use xcm_executor::traits::{ConvertLocation, XcmAssetTransfers};

/// Converts Account20 (ethereum) addresses to AccountId32 (substrate) addresses.
pub struct EvmAddressConversion<Network>(PhantomData<Network>);
impl<Network: Get<Option<NetworkId>>> ConvertLocation<AccountId> for EvmAddressConversion<Network> {
	fn convert_location(location: &Location) -> Option<AccountId> {
		let Location { parents, interior } = location;
		match interior {
			Junctions::X1(a) if *parents == 0 => {
				let j = a.as_ref()[0];
				match j {
					AccountKey20 { network: _, key } => {
						let account_32 = ExtendedAddressMapping::into_account_id(H160::from(key));
						Some(account_32)
					}
					_ => None,
				}
			}
			_ => None,
		}
	}
}

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
