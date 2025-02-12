use mock::{System, *};

pub(crate) mod mock;
#[allow(clippy::module_inception)]
mod tests;

pub fn has_event(event: mock::RuntimeEvent) -> bool {
	System::events().iter().any(|record| record.event == event)
}
