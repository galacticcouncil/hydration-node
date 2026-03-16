//! Report-only submitter for standalone dry-run mode.

use crate::traits::*;

/// Logs what would be submitted without actually submitting anything.
pub struct ReportSubmitter;

impl TxSubmitter for ReportSubmitter {
	fn submit(&self, tx: &LiquidationTx, _block_hash: [u8; 32]) -> SubmitResult {
		log::info!(
			target: "pepl-worker",
			"[DRY-RUN] Would liquidate user {:?}\n  collateral_asset: {}, debt_asset: {}\n  debt_to_cover: {}, health_factor: {}\n  status: DRY_RUN (not submitted)",
			tx.user,
			tx.collateral_asset,
			tx.debt_asset,
			tx.debt_to_cover,
			tx.health_factor,
		);
		SubmitResult::DryRun
	}
}

/// No-op dry runner for standalone mode — always returns true.
/// In standalone mode we skip dry-run validation since we can't call the runtime directly.
pub struct StandaloneDryRunner;

impl DryRunner for StandaloneDryRunner {
	fn dry_run(&self, _tx: &LiquidationTx, _block_hash: [u8; 32]) -> bool {
		true
	}
}
