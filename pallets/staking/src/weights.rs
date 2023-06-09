use sp_std::marker::PhantomData;

pub trait WeightInfo {}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {}

impl WeightInfo for () {}
