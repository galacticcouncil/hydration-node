mod amplification;
mod invariants;
mod multi_assets;
mod prices;
mod two_assets;

use crate::types::Balance;

pub(crate) const ONE: Balance = 1_000_000_000_000;

pub(crate) fn default_pegs(ct: usize) -> Vec<(Balance, Balance)> {
	vec![(1, 1); ct]
}
