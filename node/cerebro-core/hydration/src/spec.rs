//! Runtime spec-version guard. The mapper decodes events with the compiled-in `hydradx_runtime`
//! types; if mainnet has upgraded past the pinned rev, the on-chain SCALE layout can diverge and
//! decode silently produces garbage. Compare versions at startup and refuse to run on a mismatch
//! (a loud error the operator resolves by bumping the pinned rev), unless explicitly overridden.

use crate::provider::RemoteApi;
use sp_core::H256;

/// The spec_version this binary was compiled against.
pub fn compiled_spec_version() -> u32 {
	hydradx_runtime::VERSION.spec_version
}

/// The node's live spec_version at `at` (via `state_getRuntimeVersion`).
pub fn chain_spec_version(api: &RemoteApi, at: H256) -> Result<u32, String> {
	api.runtime_spec_version(at)
}

/// Compare compiled vs chain spec_version. `Ok(())` on match; `Err(msg)` describing the drift
/// otherwise. The caller decides whether to abort or warn (`--allow-spec-drift`).
pub fn check(api: &RemoteApi, at: H256) -> Result<(), String> {
	let compiled = compiled_spec_version();
	let chain = chain_spec_version(api, at)?;
	if compiled == chain {
		Ok(())
	} else {
		Err(format!(
			"spec_version mismatch: binary compiled against {compiled}, chain is {chain}. \
			 Decode may be wrong — bump the pinned hydration-node rev and rebuild, or pass \
			 --allow-spec-drift to run anyway."
		))
	}
}
