//! The chain-read seam every reader goes through. `RemoteApi` implements it over JSON-RPC; a node
//! embedding implements it over its own client or a block under construction.

use cerebro_core::BlockHash;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

pub trait Transport {
	/// Raw values for `keys` at `at`, one entry per key.
	fn storage_many(&self, keys: &[Vec<u8>], at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String>;

	fn storage(&self, key: &[u8], at: BlockHash) -> Result<Option<Vec<u8>>, String> {
		Ok(self.storage_many(&[key.to_vec()], at)?.pop().flatten())
	}

	/// Read-only EVM call (`eth_call` semantics): the call's output bytes, `Err` unless it succeeded.
	fn evm_view(&self, to: [u8; 20], data: &[u8], at: BlockHash) -> Result<Vec<u8>, String>;

	fn storage_keys(&self, prefix: &[u8], at: BlockHash) -> Result<Vec<Vec<u8>>, String>;

	/// Number of the block whose state `at` names (stableswap amplification is interpolated by it).
	fn block_number(&self, at: BlockHash) -> Result<u32, String>;

	/// Whether a read result may go into a process-wide cache. A transport that answers some reads
	/// with placeholders (see [`Prefetch`]) must say no.
	fn cache_reads(&self) -> bool {
		true
	}
}

type EvmCall = ([u8; 20], Vec<u8>);

/// Runs a reader closure repeatedly against one point of state, fetching everything it asked for
/// in one batched round per dependency level.
///
/// A key the cache doesn't have yet reads as absent and is queued; an EVM call reads as an error.
/// Readers keep going past an absent value where they can, so independent reads surface in the
/// same round and only reads that need an earlier value wait for the next one. The closure's
/// result is final once a round queues nothing.
pub struct Prefetch<'a, T: Transport> {
	inner: &'a T,
	at: BlockHash,
	storage: RefCell<BTreeMap<Vec<u8>, Option<Vec<u8>>>>,
	evm: RefCell<BTreeMap<EvmCall, Result<Vec<u8>, String>>>,
	pending_keys: RefCell<BTreeSet<Vec<u8>>>,
	pending_evm: RefCell<BTreeSet<EvmCall>>,
	pub rounds: u32,
}

pub const MAX_PREFETCH_ROUNDS: u32 = 8;

impl<'a, T: Transport> Prefetch<'a, T> {
	pub fn new(inner: &'a T, at: BlockHash) -> Self {
		Self {
			inner,
			at,
			storage: Default::default(),
			evm: Default::default(),
			pending_keys: Default::default(),
			pending_evm: Default::default(),
			rounds: 0,
		}
	}

	pub fn settle<R>(&mut self, mut f: impl FnMut(&Self) -> R) -> Result<R, String> {
		loop {
			let r = f(self);
			let keys: Vec<Vec<u8>> = std::mem::take(&mut *self.pending_keys.borrow_mut())
				.into_iter()
				.collect();
			let calls: Vec<EvmCall> = std::mem::take(&mut *self.pending_evm.borrow_mut())
				.into_iter()
				.collect();
			if keys.is_empty() && calls.is_empty() {
				return Ok(r);
			}
			if self.rounds == MAX_PREFETCH_ROUNDS {
				return Err(format!("reads did not settle in {MAX_PREFETCH_ROUNDS} rounds"));
			}
			self.rounds += 1;
			if !keys.is_empty() {
				let values = self.inner.storage_many(&keys, self.at)?;
				if values.len() != keys.len() {
					return Err(format!("storage_many: {} values for {} keys", values.len(), keys.len()));
				}
				self.storage.borrow_mut().extend(keys.into_iter().zip(values));
			}
			for (to, data) in calls {
				let out = self.inner.evm_view(to, &data, self.at);
				self.evm.borrow_mut().insert((to, data), out);
			}
		}
	}
}

impl<T: Transport> Transport for Prefetch<'_, T> {
	fn storage_many(&self, keys: &[Vec<u8>], _at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		let cache = self.storage.borrow();
		Ok(keys
			.iter()
			.map(|k| match cache.get(k) {
				Some(v) => v.clone(),
				None => {
					self.pending_keys.borrow_mut().insert(k.clone());
					None
				}
			})
			.collect())
	}

	fn evm_view(&self, to: [u8; 20], data: &[u8], _at: BlockHash) -> Result<Vec<u8>, String> {
		let call = (to, data.to_vec());
		if let Some(r) = self.evm.borrow().get(&call) {
			return r.clone();
		}
		self.pending_evm.borrow_mut().insert(call);
		Err("pending".into())
	}

	fn storage_keys(&self, prefix: &[u8], at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
		self.inner.storage_keys(prefix, at)
	}

	fn block_number(&self, at: BlockHash) -> Result<u32, String> {
		self.inner.block_number(at)
	}

	fn cache_reads(&self) -> bool {
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::cell::Cell;

	struct Counting {
		state: BTreeMap<Vec<u8>, Vec<u8>>,
		batches: Cell<u32>,
	}

	impl Transport for Counting {
		fn storage_many(&self, keys: &[Vec<u8>], _at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
			self.batches.set(self.batches.get() + 1);
			Ok(keys.iter().map(|k| self.state.get(k).cloned()).collect())
		}
		fn evm_view(&self, _to: [u8; 20], data: &[u8], _at: BlockHash) -> Result<Vec<u8>, String> {
			Ok(data.to_vec())
		}
		fn storage_keys(&self, _prefix: &[u8], _at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
			Ok(vec![])
		}
		fn block_number(&self, _at: BlockHash) -> Result<u32, String> {
			Ok(7)
		}
	}

	#[test]
	fn settle_should_batch_one_round_per_dependency_level_when_reads_chain() {
		let state = BTreeMap::from([
			(b"a".to_vec(), b"x".to_vec()),
			(b"b".to_vec(), b"y".to_vec()),
			(b"x".to_vec(), b"1".to_vec()),
			(b"y".to_vec(), b"2".to_vec()),
		]);
		let inner = Counting {
			state,
			batches: Cell::new(0),
		};
		let mut pf = Prefetch::new(&inner, [0; 32]);

		let out = pf
			.settle(|t| {
				let firsts = [t.storage(b"a", [0; 32]).unwrap(), t.storage(b"b", [0; 32]).unwrap()];
				firsts
					.iter()
					.map(|f| f.as_ref().and_then(|k| t.storage(k, [0; 32]).unwrap()))
					.collect::<Vec<_>>()
			})
			.unwrap();

		assert_eq!(out, vec![Some(b"1".to_vec()), Some(b"2".to_vec())]);
		assert_eq!(inner.batches.get(), 2);
		assert_eq!(pf.rounds, 2);
	}

	#[test]
	fn settle_should_serve_evm_calls_when_second_round_runs() {
		let inner = Counting {
			state: BTreeMap::new(),
			batches: Cell::new(0),
		};
		let mut pf = Prefetch::new(&inner, [0; 32]);

		let out = pf.settle(|t| t.evm_view([1; 20], b"call", [0; 32])).unwrap();

		assert_eq!(out, Ok(b"call".to_vec()));
		assert_eq!(pf.rounds, 1);
		assert_eq!(inner.batches.get(), 0);
	}
}
