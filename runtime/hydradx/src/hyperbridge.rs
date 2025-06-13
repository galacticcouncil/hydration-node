use crate::origins::GeneralAdmin;
use crate::{
	AssetRegistry, Balances, Ismp, IsmpParachain, NativeAssetId, Runtime, RuntimeEvent, TechCommitteeSuperMajority,
	Timestamp, TreasuryAccount, HOLLAR,
};
use frame_support::parameter_types;
use frame_support::traits::fungible::ItemOf;
use frame_support::traits::EitherOf;
use frame_system::EnsureRoot;
use ismp::{host::StateMachine, module::IsmpModule, router::IsmpRouter};
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_currencies::BasicCurrencyAdapter;
use pallet_genesis_history::migration::Weight;
use primitives::{AccountId, Amount, Balance, BlockNumber};
use sp_core::ConstU8;

impl pallet_hyperbridge::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// pallet-ismp implements the IsmpHost
	type IsmpHost = Ismp;
}

parameter_types! {
	// The hyperbridge parachain on Polkadot
	pub const Coprocessor: Option<StateMachine> = Some(StateMachine::Polkadot(3367));
	// The hyperbridge parachain on Paseo
	// pub const Coprocessor: Option<StateMachine> = Some(StateMachine::Kusama(4009));

	// The host state machine of this pallet, your state machine id goes here
	pub const HostStateMachine: StateMachine = StateMachine::Polkadot(2034);
}

impl pallet_ismp::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// Modify the consensus client's permissions, for example, TechAdmin
	type AdminOrigin = EitherOf<EnsureRoot<Self::AccountId>, EitherOf<TechCommitteeSuperMajority, GeneralAdmin>>;
	type TimestampProvider = Timestamp;
	type Balance = Balance;
	// The token used to collect fees, only stablecoins are supported
	type Currency = ItemOf<FungibleCurrencies<Runtime>, HOLLAR, AccountId>;
	// The state machine identifier of the chain -- parachain id
	type HostStateMachine = HostStateMachine;
	// Co-processor
	type Coprocessor = Coprocessor;
	// The router provides the implementation for the IsmpModule as the module id.
	type Router = IsmpRouterStruct;
	// A tuple of types implementing the ConsensusClient interface, which defines all consensus algorithms supported by this protocol deployment
	type ConsensusClients = (ismp_parachain::ParachainConsensusClient<Runtime, IsmpParachain>,);
	type WeightProvider = ();
	type OffchainDB = ();
}

#[derive(Default)]
pub struct IsmpRouterStruct;

impl IsmpRouter for IsmpRouterStruct {
	fn module_for_id(&self, id: Vec<u8>) -> Result<Box<dyn IsmpModule>, anyhow::Error> {
		match id.as_slice() {
			pallet_hyperbridge::PALLET_HYPERBRIDGE_ID => Ok(Box::new(pallet_hyperbridge::Pallet::<Runtime>::default())),
			_ => Err(ismp::Error::ModuleNotFound(id))?,
		}
	}
}

impl ismp_parachain::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// pallet-ismp implements the IsmpHost
	type IsmpHost = Ismp;
	type WeightInfo = IsmpWeights;
}

pub struct IsmpWeights;
impl ismp_parachain::weights::WeightInfo for IsmpWeights {
	fn add_parachain(_n: u32) -> Weight {
		Weight::from_parts(10_000, 0u64)
	}

	fn remove_parachain(_n: u32) -> Weight {
		Weight::from_parts(10_000, 0u64)
	}

	fn update_parachain_consensus() -> Weight {
		Weight::from_parts(10_000, 0u64)
	}
}

impl pallet_token_gateway::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Dispatcher = Ismp;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
	type AssetAdmin = TreasuryAccount;
	type CreateOrigin = EitherOf<EnsureRoot<Self::AccountId>, EitherOf<TechCommitteeSuperMajority, GeneralAdmin>>;
	type Assets = FungibleCurrencies<Runtime>;
	type NativeAssetId = NativeAssetId;
	type Decimals = ConstU8<12>;
	type EvmToSubstrate = ();
	type WeightInfo = ();
}
