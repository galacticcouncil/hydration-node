//! Transactions this node keeps out of its own blocks, and the operator's releases. Memory only:
//! after a restart, pending transactions are judged again.

use sp_core::H256;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Held {
	pub signer: Option<[u8; 32]>,
	pub first_seen_block: u32,
	pub first_seen_unix_ms: u64,
	pub net_usd: f64,
	pub pool_leak_usd: f64,
	pub assets: Vec<u32>,
	pub pools: Vec<String>,
	pub summary: String,
}

/// A change to the book worth an audit line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Removal {
	/// A held transaction left the pool (included elsewhere, expired, dropped).
	Pruned(H256),
	/// A release ran out without the transaction being included.
	Expired(H256),
}

#[derive(Debug, Default)]
pub struct HoldBook {
	held: HashMap<H256, Held>,
	/// Released hash → last block (inclusive) the release covers.
	released: HashMap<H256, u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseError {
	NotHeld,
}

impl HoldBook {
	pub fn hold(&mut self, hash: H256, held: Held) {
		self.held.entry(hash).or_insert(held);
	}

	pub fn is_held(&self, hash: &H256) -> bool {
		self.held.contains_key(hash)
	}

	pub fn is_released(&self, hash: &H256, best: u32) -> bool {
		self.released.get(hash).is_some_and(|until| best <= *until)
	}

	pub fn held(&self) -> impl Iterator<Item = (&H256, &Held)> {
		self.held.iter()
	}

	pub fn held_count(&self) -> usize {
		self.held.len()
	}

	pub fn released_count(&self) -> usize {
		self.released.len()
	}

	/// Release `hash` until `best + ttl_blocks`. Releasing a released hash again extends it.
	pub fn release(&mut self, hash: H256, best: u32, ttl_blocks: u32) -> Result<u32, ReleaseError> {
		if self.held.remove(&hash).is_none() && !self.released.contains_key(&hash) {
			return Err(ReleaseError::NotHeld);
		}
		let until = best.saturating_add(ttl_blocks);
		self.released.insert(hash, until);
		Ok(until)
	}

	/// Forget `hash` entirely (it was dropped from the pool).
	pub fn forget(&mut self, hash: &H256) -> bool {
		let held = self.held.remove(hash).is_some();
		let released = self.released.remove(hash).is_some();
		held || released
	}

	/// Run on every new best block. A release that runs out puts its transaction back under the
	/// guard; nothing else about it is remembered.
	pub fn prune(&mut self, best: u32, in_pool: impl Fn(&H256) -> bool) -> Vec<Removal> {
		let mut out = Vec::new();
		self.held.retain(|hash, _| {
			let keep = in_pool(hash);
			if !keep {
				out.push(Removal::Pruned(*hash));
			}
			keep
		});
		self.released.retain(|hash, until| {
			if !in_pool(hash) {
				return false;
			}
			let keep = best <= *until;
			if !keep {
				out.push(Removal::Expired(*hash));
			}
			keep
		});
		out.sort_by_key(|r| match r {
			Removal::Pruned(h) | Removal::Expired(h) => *h,
		});
		out
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn held() -> Held {
		Held {
			signer: Some([7; 32]),
			first_seen_block: 10,
			first_seen_unix_ms: 1,
			net_usd: 120.5,
			pool_leak_usd: 0.0,
			assets: vec![5],
			pools: vec!["omnipool:5".into()],
			summary: "extraction".into(),
		}
	}

	#[test]
	fn release_should_survive_abandoned_proposal_when_ttl_not_expired() {
		let mut book = HoldBook::default();
		let hash = H256::repeat_byte(1);
		book.hold(hash, held());

		assert_eq!(book.release(hash, 100, 10), Ok(110));

		// A proposal that pushed it was abandoned: nothing consumes the release.
		assert!(book.is_released(&hash, 105));
		assert_eq!(book.prune(105, |_| true), vec![]);
		assert!(book.is_released(&hash, 110));
		assert!(!book.is_held(&hash));
	}

	#[test]
	fn release_should_expire_when_ttl_blocks_pass() {
		let mut book = HoldBook::default();
		let hash = H256::repeat_byte(1);
		book.hold(hash, held());
		book.release(hash, 100, 10).unwrap();

		assert_eq!(book.prune(111, |_| true), vec![Removal::Expired(hash)]);

		assert!(!book.is_released(&hash, 111));
		assert!(!book.is_held(&hash));
		assert_eq!(book.released_count(), 0);
	}

	#[test]
	fn release_should_extend_when_called_again() {
		let mut book = HoldBook::default();
		let hash = H256::repeat_byte(1);
		book.hold(hash, held());
		book.release(hash, 100, 10).unwrap();

		assert_eq!(book.release(hash, 108, 10), Ok(118));
		assert!(book.is_released(&hash, 118));
	}

	#[test]
	fn release_should_fail_when_hash_is_unknown() {
		let mut book = HoldBook::default();

		assert_eq!(book.release(H256::repeat_byte(9), 100, 10), Err(ReleaseError::NotHeld));
	}

	#[test]
	fn prune_should_remove_hold_when_tx_left_pool() {
		let mut book = HoldBook::default();
		let (gone, kept) = (H256::repeat_byte(1), H256::repeat_byte(2));
		book.hold(gone, held());
		book.hold(kept, held());

		assert_eq!(book.prune(50, |h| *h == kept), vec![Removal::Pruned(gone)]);
		assert!(book.is_held(&kept));
		assert!(!book.is_held(&gone));
	}

	#[test]
	fn prune_should_drop_release_silently_when_released_tx_was_included() {
		let mut book = HoldBook::default();
		let hash = H256::repeat_byte(1);
		book.hold(hash, held());
		book.release(hash, 100, 10).unwrap();

		assert_eq!(book.prune(101, |_| false), vec![]);
		assert_eq!(book.released_count(), 0);
	}

	#[test]
	fn hold_should_keep_first_finding_when_held_again() {
		let mut book = HoldBook::default();
		let hash = H256::repeat_byte(1);
		book.hold(hash, held());

		book.hold(
			hash,
			Held {
				first_seen_block: 99,
				..held()
			},
		);

		assert_eq!(book.held().next().map(|(_, h)| h.first_seen_block), Some(10));
	}
}
