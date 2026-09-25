//! TEMPORARY node-side patch — delete (with its call sites) once the runtime ships the
//! `solver_intents` pre-filter fix.
//!
//! Runtimes up to spec 447 withhold any DCA whose hard limit exceeds `oracle × (1 − slippage)`,
//! although settlement enforces `max(hard_limit, oracle_floor)` and accepts it. For every DCA
//! withheld that way the hard limit *is* the enforced floor, so it is re-admitted as a swap bound
//! by its own `amount_out` — no `min_amount_out` entry needed.

use crate::{AssetId, Balance, Intent, IntentData, IntentId};
use sp_std::cmp::Reverse;
use sp_std::vec::Vec;

/// Appends eligible stored DCAs missing from `intents`; returns how many were added.
///
/// `stored` is `pallet_intent::Intents` at the block `intents` was built at, and `current_block`
/// that block's number. A DCA whose assets are absent from `existential_deposits` is skipped:
/// the solver could not size it.
pub fn readmit_withheld_dcas(
	intents: &mut Vec<Intent>,
	existential_deposits: &[(AssetId, Balance)],
	stored: impl IntoIterator<Item = (IntentId, IntentData)>,
	current_block: u32,
) -> usize {
	let before = intents.len();
	let has_ed = |asset| existential_deposits.iter().any(|(a, _)| *a == asset);

	for (id, data) in stored {
		let IntentData::Dca(dca) = data else { continue };
		if current_block < dca.last_execution_block.saturating_add(dca.period)
			|| dca.remaining_budget < dca.amount_in
			|| !has_ed(dca.asset_in)
			|| !has_ed(dca.asset_out)
			|| intents.iter().any(|i| i.id == id)
		{
			continue;
		}
		intents.push(Intent {
			id,
			data: IntentData::Swap(dca.to_swap_data(dca.amount_out)),
		});
	}

	// Same order `solver_intents` hands out, so the input matches the fixed runtime's.
	intents.sort_by_key(|i| Reverse(i.id));
	intents.len() - before
}
