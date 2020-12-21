pub use pallet_grandpa::fg_primitives;
pub use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};

pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;

#[macro_export]
macro_rules! runtime_standalone {
    () => {

        pub const VERSION: RuntimeVersion = RuntimeVersion {
                spec_name: create_runtime_str!("hack-hydra-dx-standalone"),
                impl_name: create_runtime_str!("hack-hydra-dx-standalone"),
                authoring_version: 1,
                spec_version: 1,
                impl_version: 1,
                apis: RUNTIME_API_VERSIONS,
                transaction_version: 1,
            };

        impl pallet_aura::Config for Runtime {
            type AuthorityId = AuraId;
        }

        impl pallet_grandpa::Config for Runtime {
            type Event = Event;
            type Call = Call;

            type KeyOwnerProofSystem = ();

            type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

            type KeyOwnerIdentification =
                <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::IdentificationTuple;

            type HandleEquivocation = ();

            type WeightInfo = ();
        }

        impl pallet_timestamp::Config for Runtime {
        /// A timestamp: milliseconds since the unix epoch.
            type Moment = u64;
            type OnTimestampSet = Aura;
            type MinimumPeriod = MinimumPeriod;
            type WeightInfo = ();
        }

        construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = opaque::Block,
            UncheckedExtrinsic = UncheckedExtrinsic
        {
            System: frame_system::{Module, Call, Config, Storage, Event<T>},
            RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
            Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
            Aura: pallet_aura::{Module, Config<T>, Inherent},
            Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
            Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
            TransactionPayment: pallet_transaction_payment::{Module, Storage},
            Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>},

            // ORML related modules
            Tokens: orml_tokens::{Module, Storage, Call, Event<T>, Config<T>},
            Currencies: orml_currencies::{Module, Call, Event<T>},

            // HydraDX related modules
            AssetRegistry: pallet_asset_registry::{Module, Call, Storage, Config<T>},
            AMM: pallet_amm::{Module, Call, Storage, Event<T>},
            Exchange: pallet_exchange::{Module, Call, Storage, Event<T>},
            Faucet: pallet_faucet::{Module, Call, Storage, Event<T>},
            MultiTransactionPayment: pallet_transaction_multi_payment::{Module, Call, Storage, Event<T>},

            // Include the custom logic from the template pallet in the runtime.
            TemplateModule: pallet_template::{Module, Call, Storage, Event<T>},
            }
        );

    };
}
