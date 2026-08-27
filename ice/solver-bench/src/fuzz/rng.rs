//! Deterministic SplitMix64 PRNG so every scenario is reproducible from a
//! single `u64` seed, with no external rng dependency and identical output
//! across machines.

pub struct Rng(u64);

impl Rng {
	pub fn new(seed: u64) -> Self {
		// Avoid the all-zero state degenerating the first few draws.
		Rng(seed ^ 0xA0761D6478BD642F)
	}

	pub fn next_u64(&mut self) -> u64 {
		self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
		let mut z = self.0;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
		z ^ (z >> 31)
	}

	/// Uniform in `[lo, hi)`. Returns `lo` if the range is empty.
	pub fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
		if hi <= lo + 1 {
			return lo;
		}
		lo + (self.next_u64() as usize) % (hi - lo)
	}

	/// True with probability `num / den`.
	pub fn chance(&mut self, num: u32, den: u32) -> bool {
		(self.next_u64() % den as u64) < num as u64
	}

	pub fn choose<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
		&xs[self.range_usize(0, xs.len())]
	}

	/// Log-uniform sample in `[lo, hi]` — biases toward smaller magnitudes so
	/// dust-near-ED and pool-moving whales are both well represented.
	pub fn log_amount(&mut self, lo: u128, hi: u128) -> u128 {
		if hi <= lo {
			return lo;
		}
		let lo_bits = 128 - lo.max(1).leading_zeros();
		let hi_bits = 128 - hi.leading_zeros();
		let bits = self.range_usize(lo_bits as usize, hi_bits as usize + 1) as u32;
		let base = 1u128 << bits.saturating_sub(1).min(126);
		let jitter = self.next_u64() as u128 % base.max(1);
		(base + jitter).clamp(lo, hi)
	}

	pub fn shuffle<T>(&mut self, xs: &mut [T]) {
		for i in (1..xs.len()).rev() {
			let j = self.range_usize(0, i + 1);
			xs.swap(i, j);
		}
	}
}

/// Mix an iteration index into the run seed so per-scenario seeds are
/// well-distributed and each is independently replayable.
pub fn scenario_seed(run_seed: u64, iter: u64) -> u64 {
	let mut z = run_seed
		.wrapping_add(iter.wrapping_mul(0x9E37_79B9_7F4A_7C15))
		.wrapping_add(0x2545_F491_4F6C_DD1D);
	z = (z ^ (z >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
	z = (z ^ (z >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
	z ^ (z >> 33)
}
