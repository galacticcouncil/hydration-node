//! Stub types for standalone mode.
//!
//! `MoneyMarketData` needs `PhantomData<(Block, OriginCaller, RuntimeCall, RuntimeEvent)>`.
//! In standalone mode we define minimal stubs since the actual dispatch types are not needed.

/// Stub block type for standalone mode.
pub type StandaloneBlock = sp_runtime::generic::Block<
	sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>,
	sp_runtime::OpaqueExtrinsic,
>;

/// Stub origin caller for standalone mode.
#[derive(Clone, Debug)]
pub struct StandaloneOriginCaller;

/// Stub runtime call for standalone mode.
#[derive(Clone, Debug)]
pub struct StandaloneRuntimeCall;

/// Stub runtime event for standalone mode.
#[derive(Clone, Debug)]
pub struct StandaloneRuntimeEvent;
