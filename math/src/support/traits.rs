pub trait CheckedAddInto {
	type Output;
	fn checked_add_into(&self, other: &Self) -> Option<Self::Output>;
}

pub trait CheckedMulInto {
	type Output;
	fn checked_mul_into(&self, other: &Self) -> Option<Self::Output>;
}

pub trait CheckedAddInner: Sized {
	type Inner;
	fn checked_add_inner(&self, other: &Self::Inner) -> Option<Self>;
}

pub trait CheckedMulInner: Sized {
	type Inner;
	fn checked_mul_inner(&self, other: &Self::Inner) -> Option<Self>;
}

pub trait CheckedDivInner: Sized {
	type Inner;
	fn checked_div_inner(&self, other: &Self::Inner) -> Option<Self>;
}
