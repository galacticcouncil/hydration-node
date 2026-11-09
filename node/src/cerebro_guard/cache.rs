//! Slow-changing chain context the guard needs on every candidate: protocol accounts and the
//! ERC20 / aToken registry maps. Read from the client at an imported block, never from the block
//! being built, so it costs nothing in the proof.

use cerebro_core::accounts::ProtocolAccounts;
use cerebro_core::AssetId;
use cerebro_hydration::{accounts, balance, Transport};
use sp_core::{H160, H256};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct Caches {
	pub protocol: ProtocolAccounts,
	pub contracts: HashMap<H160, AssetId>,
	pub atoken_of: BTreeMap<AssetId, AssetId>,
	pub loaded_at: u32,
}

pub fn load<T: Transport>(t: &T, at: H256, number: u32) -> Result<Caches, String> {
	let protocol = accounts::fetch(t, at)?;
	let contracts = balance::build_contract_map(t, at);
	let atoken_of = balance::build_underlying_to_atoken(t, &contracts, at);
	Ok(Caches {
		protocol,
		contracts,
		atoken_of,
		loaded_at: number,
	})
}

/// The current caches. A failed refresh keeps the previous ones; with none at all the guard is
/// not ready.
#[derive(Debug, Default)]
pub struct CacheSlot {
	current: RwLock<Option<Arc<Caches>>>,
}

impl CacheSlot {
	pub fn get(&self) -> Option<Arc<Caches>> {
		self.current.read().unwrap_or_else(|e| e.into_inner()).clone()
	}

	/// Store a refresh result. Returns the error so the caller can count it.
	pub fn apply(&self, result: Result<Caches, String>) -> Result<(), String> {
		let caches = result?;
		*self.current.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(caches));
		Ok(())
	}

	/// Whether a refresh is due at `number`.
	pub fn due(&self, number: u32, every: u32) -> bool {
		self.get()
			.is_none_or(|c| number.saturating_sub(c.loaded_at) >= every.max(1))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn caches(loaded_at: u32) -> Caches {
		Caches {
			protocol: ProtocolAccounts::default(),
			contracts: HashMap::new(),
			atoken_of: BTreeMap::new(),
			loaded_at,
		}
	}

	#[test]
	fn guard_should_not_be_ready_when_first_cache_refresh_fails() {
		let slot = CacheSlot::default();

		assert_eq!(slot.apply(Err("rpc down".into())), Err("rpc down".into()));

		assert!(slot.get().is_none());
		assert!(slot.due(1, 300));
	}

	#[test]
	fn guard_should_keep_previous_cache_when_later_refresh_fails() {
		let slot = CacheSlot::default();
		slot.apply(Ok(caches(100))).unwrap();

		assert!(slot.apply(Err("decode".into())).is_err());

		assert_eq!(slot.get().map(|c| c.loaded_at), Some(100));
	}

	#[test]
	fn due_should_wait_refresh_interval_when_cache_is_loaded() {
		let slot = CacheSlot::default();
		slot.apply(Ok(caches(100))).unwrap();

		assert!(!slot.due(399, 300));
		assert!(slot.due(400, 300));
	}
}
