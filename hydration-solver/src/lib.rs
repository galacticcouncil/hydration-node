//pub mod omni;

mod data;
mod problem;
#[cfg(test)]
mod tests;
pub mod traits;
mod types;
pub mod v3;

const LOG_TARGET: &str = "ice-solver";

/*
pub struct HydrationSolver<T, RA, B, BE, TP, SC>(PhantomData<(T, RA, B, BE, TP, SC)>);

impl<T, RA, Block, BE, TP, SC> HydrationSolver<T, RA, Block, BE, TP, SC>
where
	Block: sp_runtime::traits::Block,
	RA: ProvideRuntimeApi<Block> + UsageProvider<Block>,
	RA::Api: ICEApi<Block, AccountId, AssetId>,
	//BE: Backend<Block> + 'static,
	RA: BlockchainEvents<Block> + 'static,
	TP: MaintainedTransactionPool<Block = Block, Hash = <Block as BlockT>::Hash> + 'static,
	SC: hydradx_traits::ice::SolverSolution<u32>,
	T: pallet_ice::Config
		+ frame_system::Config<RuntimeCall = hydradx_runtime::RuntimeCall>
		+ pallet_omnipool::Config<AssetId = AssetId>
		+ pallet_asset_registry::Config<AssetId = AssetId>
		+ pallet_dynamic_fees::Config<Fee = Permill, AssetId = AssetId>,
	//pallet_ice::Call::<T> : Into<hydradx_runtime::RuntimeCall>
{
	pub async fn run(client: Arc<RA>, transaction_pool: Arc<TP>, sc: Arc<SC>) {
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

					println!("found solution ,submit it pls 2");
					println!("setting solution");
					sc.set_solution(222u32);
					println!("solution set ");
					//runtime.submit_solution(notification.hash, &notification.header, resolved_intents).unwrap();
					/*
					let call = hydradx_runtime::RuntimeCall::ICE(pallet_ice::Call::propose_solution {
						intents: BoundedResolvedIntents::truncate_from(resolved_intents),
						//trades: BoundedTrades::truncate_from(trades),
						trades: BoundedTrades::truncate_from(vec![]),
						score,
						block: block_number.saturating_add(1u32.into()).into(),
					});


					let uxt = Block::Extrinsic::new(call, None).unwrap();
					let r = transaction_pool.submit_at(h, TransactionSource::Local, vec![uxt]).await;
					println!("submit result: {:?}", r);
					 */

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

 */

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
