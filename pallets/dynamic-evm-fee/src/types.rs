use sp_runtime::FixedU128;

pub trait MultiplierProvider {
	fn next() -> FixedU128;
}
