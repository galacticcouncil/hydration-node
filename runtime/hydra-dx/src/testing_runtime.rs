use crate::{BaseFilter, Call, Filter};

impl Filter<Call> for BaseFilter {
	fn filter(_call: &Call) -> bool {
		true
	}
}
