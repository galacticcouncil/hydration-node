use mock::System;

mod add_to_queue;
mod dispatch_top;
pub(crate) mod mock;
mod validate_unsigned;

pub fn has_event(event: mock::RuntimeEvent) -> bool {
	System::events().iter().any(|record| record.event == event)
}
