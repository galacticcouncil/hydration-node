use codec::{Decode, Encode};

pub trait AMMState {
	/// Opaque state type - solver knows how to interpret it
	type State: Encode + Decode;

	/// Get current state of all relevant AMM pools
	fn get_state() -> Self::State;
}
