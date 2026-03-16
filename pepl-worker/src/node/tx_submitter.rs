//! Node-mode TxSubmitter: wraps transaction_pool.submit_one().

use crate::traits::*;
use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use fp_self_contained::UncheckedExtrinsic;
use frame_support::BoundedVec;
use hydradx_runtime::RuntimeCall;
use sc_service::SpawnTaskHandle;
use sc_transaction_pool_api::TransactionPool;
use sp_runtime::transaction_validity::TransactionSource;
use std::sync::Arc;

/// Submits liquidation transactions to the node's transaction pool.
pub struct NodeTxSubmitter<Block: BlockT, P: TransactionPool<Block = Block>> {
	transaction_pool: Arc<P>,
	spawner: SpawnTaskHandle,
	_phantom: std::marker::PhantomData<Block>,
}

impl<Block: BlockT, P: TransactionPool<Block = Block>> NodeTxSubmitter<Block, P> {
	pub fn new(transaction_pool: Arc<P>, spawner: SpawnTaskHandle) -> Self {
		Self {
			transaction_pool,
			spawner,
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<Block, P> TxSubmitter for NodeTxSubmitter<Block, P>
where
	Block: BlockT,
	P: TransactionPool<Block = Block> + 'static,
	Block::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
	Block::Hash: From<sp_core::H256>,
{
	fn submit(&self, tx: &LiquidationTx, block_hash: [u8; 32]) -> SubmitResult {
		let hash: Block::Hash = sp_core::H256::from(block_hash).into();

		let liquidation_call = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
			collateral_asset: tx.collateral_asset,
			debt_asset: tx.debt_asset,
			user: tx.user,
			debt_to_cover: tx.debt_to_cover,
			route: BoundedVec::new(),
		});

		let encoded_tx: UncheckedExtrinsic<
			hydradx_runtime::Address,
			RuntimeCall,
			hydradx_runtime::Signature,
			hydradx_runtime::SignedExtra,
		> = UncheckedExtrinsic::new_bare(liquidation_call);
		let encoded = encoded_tx.encode();
		let opaque_tx = sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..])
			.expect("Encoded extrinsic is always valid");

		let tx_pool = self.transaction_pool.clone();
		let user = tx.user;

		self.spawner.spawn(
			"liquidation-worker-on-submit",
			Some("liquidation-worker"),
			async move {
				let submit_result = tx_pool
					.submit_one(hash, TransactionSource::Local, opaque_tx.into())
					.await;
				log::info!(
					target: "pepl-worker",
					"submit result for user {:?}: {:?}",
					user,
					submit_result
				);
			},
		);

		SubmitResult::Submitted
	}
}

/// Dry-run only submitter that logs what would be submitted.
pub struct ReportOnlySubmitter;

impl TxSubmitter for ReportOnlySubmitter {
	fn submit(&self, tx: &LiquidationTx, _block_hash: [u8; 32]) -> SubmitResult {
		log::info!(
			target: "pepl-worker",
			"[DRY-RUN] Would submit: {}",
			tx
		);
		SubmitResult::DryRun
	}
}
