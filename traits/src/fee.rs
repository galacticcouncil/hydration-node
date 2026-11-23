pub trait GetDynamicFee<K> {
	type Fee;
	// Return a fee for a given key
	fn get(key: K) -> Self::Fee;
	// Return a fee for a given key and store it
	fn get_and_store(key: K) -> Self::Fee;
	// Return the base fee for a given key, without any dynamic increase
	fn get_base(key: K) -> Self::Fee {
		Self::get(key)
	}
}
