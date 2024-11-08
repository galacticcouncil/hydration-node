mod omni;

use futures::future::ready;
use futures::StreamExt;
use pallet_ice::api::ICEApi;
use primitives::{AccountId, AssetId};
use sc_client_api::{Backend, BlockchainEvents};
use sp_api::ProvideRuntimeApi;
use sp_core::offchain::storage::OffchainDb;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

const LOG_TARGET: &str = "ice-solver";

pub struct HydrationSolver<RA, B, BE>(PhantomData<(RA, B, BE)>);

impl<RA, Block, BE> HydrationSolver<RA, Block, BE>
where
	Block: sp_runtime::traits::Block,
	RA: ProvideRuntimeApi<Block>,
	RA::Api: pallet_ice::api::ICEApi<Block, AccountId, AssetId>,
	BE: Backend<Block> + 'static,
	RA: BlockchainEvents<Block> + 'static,
{
	//pub async fn run<BE: BlockchainEvents<Block>>(client: Arc<BE>) {
	pub async fn run(client: Arc<RA>) {
		tracing::debug!(
			target: LOG_TARGET,
			"starting solver runner",
		);
		let mut notification_st = client.import_notification_stream();

		while let Some(notification) = notification_st.next().await {
			tracing::debug!(target: LOG_TARGET, "notification: {:?}", notification);
			if notification.is_new_best {
				tracing::debug!(target: LOG_TARGET, "is best");
				let runtime = client.runtime_api();
				let intents = runtime.intents(notification.hash, &notification.header);
			}
		}
	}
}

#[macro_export]
macro_rules! rational_to_f64 {
	($x:expr, $y:expr) => {
		($x as f64) / ($y as f64)
		//FixedU128::from_rational($x, $y).to_float()
	};
}
#[macro_export]
macro_rules! to_f64_by_decimals {
	($x:expr, $y:expr) => {
		($x as f64) / (10u128.pow($y as u32) as f64)
		//FixedU128::from_rational($x, 10u128.pow($y as u32)).to_float()
	};
}
