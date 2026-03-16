//! Liquidation worker RPC API.

use super::LiquidationTaskData;
use jsonrpsee::{
	core::{async_trait, RpcResult},
	proc_macros::rpc,
	types::error::ErrorObject,
};
use liquidation_worker_support::Borrower;
use std::sync::Arc;

#[rpc(client, server)]
pub trait LiquidationWorkerApi {
	#[method(name = "liquidation_getBorrowers")]
	async fn get_borrowers(&self) -> RpcResult<Vec<Borrower>>;

	#[method(name = "liquidation_isRunning")]
	async fn is_running(&self) -> RpcResult<bool>;

	#[method(name = "liquidation_maxTransactionsPerBlock")]
	async fn max_transactions_per_block(&self) -> RpcResult<usize>;
}

/// Error type of this RPC api.
pub enum Error {
	/// Getting the lock failed.
	LockError,
}

impl From<Error> for i32 {
	fn from(e: Error) -> i32 {
		match e {
			Error::LockError => 1,
		}
	}
}

/// Provides RPC methods.
pub struct LiquidationWorker {
	pub liquidation_task_data: Arc<LiquidationTaskData>,
}

impl LiquidationWorker {
	pub fn new(liquidation_task_data: Arc<LiquidationTaskData>) -> Self {
		Self {
			liquidation_task_data,
		}
	}
}

#[async_trait]
impl LiquidationWorkerApiServer for LiquidationWorker {
	async fn get_borrowers(&self) -> RpcResult<Vec<Borrower>> {
		if let Ok(borrowers) = self.liquidation_task_data.borrowers_list.lock() {
			Ok(borrowers.clone())
		} else {
			Ok(Vec::new())
		}
	}

	async fn is_running(&self) -> RpcResult<bool> {
		if let Ok(thread_pool) = self.liquidation_task_data.clone().thread_pool.lock() {
			if thread_pool.active_count() > 0 {
				return Ok(true);
			}
		}

		Ok(false)
	}

	async fn max_transactions_per_block(&self) -> RpcResult<usize> {
		if let Ok(max_transactions) = self.liquidation_task_data.max_transactions.lock() {
			Ok(*max_transactions)
		} else {
			Err(ErrorObject::owned(
				Error::LockError.into(),
				"Unable to acquire the max_transactions lock. PEPL probably not running.",
				None::<String>,
			))
		}
	}
}
