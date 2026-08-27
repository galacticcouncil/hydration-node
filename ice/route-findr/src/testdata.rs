//! Hydration mainnet pool snapshot for integration tests.
//!
//! Source: SDK `PoolContextProvider.getPools()` — real on-chain state.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::types::{PoolEdge, PoolType};

/// Returns the full pool set from a Hydration mainnet snapshot.
pub fn mainnet_pools() -> Vec<PoolEdge> {
	vec![
		// ---------------------------------------------------------------
		// Aave pools (19)
		// ---------------------------------------------------------------
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![22, 1003],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![10, 1002],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![5, 1001],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![15, 1005],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![1000765, 1006],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![690, 69],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![4200, 420],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![34, 1007],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![103, 1008],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![110, 1110],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![111, 1111],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![112, 1112],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![113, 1113],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![39, 1039],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![43, 1043],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![90001, 9001],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![1000752, 1009],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![44, 1044],
		},
		PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![10044, 4444],
		},
		// ---------------------------------------------------------------
		// Omnipool (1)
		// ---------------------------------------------------------------
		PoolEdge {
			pool_type: PoolType::Omnipool,
			assets: vec![
				1000771, 222, 420, 0, 1001, 39, 38, 16, 14, 1000796, 19, 1000795, 35, 33, 15, 1000794, 1000753,
				1000624, 1000765, 9001, 9, 1000752, 1,
			],
		},
		// ---------------------------------------------------------------
		// Stableswap pools (15)
		// ---------------------------------------------------------------
		PoolEdge {
			pool_type: PoolType::Stableswap(100),
			assets: vec![10, 18, 21, 23, 100],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(110),
			assets: vec![222, 1003, 110],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(143),
			assets: vec![43, 222, 143],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(101),
			assets: vec![11, 19, 101],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(44),
			assets: vec![222, 1044, 10044],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(105),
			assets: vec![21, 23, 222, 105],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(103),
			assets: vec![1002, 1000766, 1000767, 103],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(111),
			assets: vec![222, 1002, 111],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(4200),
			assets: vec![1007, 1000809, 4200],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(104),
			assets: vec![20, 1007, 104],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(90001),
			assets: vec![40, 1009, 90001],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(102),
			assets: vec![10, 22, 102],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(690),
			assets: vec![15, 1001, 690],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(112),
			assets: vec![222, 1000745, 112],
		},
		PoolEdge {
			pool_type: PoolType::Stableswap(113),
			assets: vec![222, 1000625, 113],
		},
		// ---------------------------------------------------------------
		// HSM pools (4)
		// ---------------------------------------------------------------
		PoolEdge {
			pool_type: PoolType::HSM,
			assets: vec![222, 1002],
		},
		PoolEdge {
			pool_type: PoolType::HSM,
			assets: vec![222, 1000745],
		},
		PoolEdge {
			pool_type: PoolType::HSM,
			assets: vec![222, 1000625],
		},
		PoolEdge {
			pool_type: PoolType::HSM,
			assets: vec![222, 1003],
		},
		// ---------------------------------------------------------------
		// XYK pools (25)
		// ---------------------------------------------------------------
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![0, 5],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![0, 27],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![26, 5],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![10, 25],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 30],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![1000081, 34],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 25],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 1000081],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![0, 15],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 3370],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![21, 5],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![0, 10],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![1000085, 0],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 15],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 36],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![252525, 22],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 24],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![1000085, 5],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![39, 222],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![10, 32],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![5, 252525],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![1000081, 15],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![0, 17],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![25, 1000771],
		},
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![1000081, 22],
		},
	]
}

/// Total number of pools in the mainnet snapshot.
pub const POOL_COUNT: usize = 64;

/// Total unique asset IDs across all pools.
pub fn unique_asset_count() -> usize {
	let pools = mainnet_pools();
	let mut assets = alloc::collections::BTreeSet::new();
	for pool in &pools {
		for &a in &pool.assets {
			assets.insert(a);
		}
	}
	assets.len()
}
