pub mod omni;

mod data;
mod problem;
#[cfg(test)]
mod tests;
pub mod v3;

use futures::future::ready;
use futures::StreamExt;
use pallet_ice::api::ICEApi;
use pallet_ice::traits::Solver;
use pallet_ice::types::{BoundedResolvedIntents, BoundedTrades};
use primitives::{AccountId, AssetId};
use sc_client_api::{Backend, BlockchainEvents, UsageProvider};
use sp_api::ProvideRuntimeApi;
use sp_core::offchain::storage::OffchainDb;
use sp_runtime::Permill;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use sp_api::__private::BlockT;
use sp_runtime::traits::{Hash, Header};
use sc_transaction_pool_api::{MaintainedTransactionPool, TransactionPool};
use sp_runtime::transaction_validity::TransactionSource;

const LOG_TARGET: &str = "ice-solver";

pub struct HydrationSolver<T, RA, B, BE, TP>(PhantomData<(T, RA, B, BE, TP)>);

impl<T, RA, Block, BE ,TP> HydrationSolver<T, RA, Block, BE, TP>
where
	Block: sp_runtime::traits::Block,
	RA: ProvideRuntimeApi<Block> + UsageProvider<Block>,
	RA::Api: ICEApi<Block, AccountId, AssetId>,
	BE: Backend<Block> + 'static,
	RA: BlockchainEvents<Block> + 'static,
	TP: MaintainedTransactionPool<Block = Block, Hash = <Block as BlockT>::Hash> + 'static,
	T: pallet_ice::Config
		+ pallet_omnipool::Config<AssetId = AssetId>
		+ pallet_asset_registry::Config<AssetId = AssetId>
		+ pallet_dynamic_fees::Config<Fee = Permill, AssetId = AssetId>,
{
	pub async fn run(client: Arc<RA>, transaction_pool: Arc<TP>) {
		tracing::debug!(
			target: LOG_TARGET,
			"starting solver runner",
		);

		let mut notification_st = client.import_notification_stream();

		while let Some(notification) = notification_st.next().await {
			println!("notification: {:?}", notification);
			let block_number: primitives::BlockNumber = 1; //TODO: get block number somehow
			if notification.is_new_best {
				//tracing::debug!(target: LOG_TARGET, "is best");
				println!("is best");
				let chain_info = client.usage_info().chain;
				let h = notification.header.hash();
				println!("chain info: {:?}", chain_info.best_number,);
				let runtime = client.runtime_api();
				if let Ok(intents) = runtime.intents(notification.hash, &notification.header) {
					// Compute solution using solver
					let Ok((resolved_intents, metadata)) =
						omni::OmniSolver::<AccountId, AssetId, hydradx_adapters::ice::OmnipoolDataProvider<T>>::solve(
							intents,
						)
					else {
						//TODO: log error
						return;
					};
					let (trades, score) =
						pallet_ice::Pallet::<T>::calculate_trades_and_score(&resolved_intents).unwrap();

					println!("found solution ,submit it pls");
					let call = pallet_ice::Call::propose_solution {
						intents: BoundedResolvedIntents::truncate_from(resolved_intents),
						trades: BoundedTrades::truncate_from(trades),
						score,
						block: block_number.saturating_add(1u32.into()).into(),
					};

					transaction_pool.submit_at(h, TransactionSource::Local, vec![call.into()]);


					/*
					let call = pallet_ice::Call::propose_solution {
						intents: BoundedResolvedIntents::truncate_from(resolved_intents),
						trades: BoundedTrades::truncate_from(trades),
						score,
						block: block_number.saturating_add(1u32.into()).into(),
					};
					let _ =
						frame_system::offchain::SubmitTransaction::<T, pallet_ice::Call<T>>::submit_unsigned_transaction(
							call.into(),
						);

					 */
				}
			}
		}
	}
}

#[macro_export]
macro_rules! rational_to_f64 {
	($x:expr, $y:expr) => {
		($x as f64) / ($y as f64)
	};
}
#[macro_export]
macro_rules! to_f64_by_decimals {
	($x:expr, $y:expr) => {
		($x as f64) / (10u128.pow($y as u32) as f64)
	};
}
