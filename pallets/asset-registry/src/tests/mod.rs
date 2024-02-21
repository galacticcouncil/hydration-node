use mock::*;

use crate::*;
use frame_support::{assert_noop, assert_ok};

mod create_trait;
mod inspect_trait;
pub(crate) mod mock;
mod mutate_trait;
mod register;
#[allow(clippy::module_inception)]
mod tests;
mod update;

#[macro_export]
macro_rules! assert_last_event {
	( $x:expr ) => {{
		pretty_assertions::assert_eq!(System::events().last().expect("events expected").event, $x);
	}};
}

pub fn has_event(event: mock::RuntimeEvent) -> bool {
	System::events().iter().any(|record| record.event == event)
}
