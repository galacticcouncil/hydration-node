#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;

benchmarks! {
	_ {}

	claim {
		let alice_id = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"];
		let caller = T::AccountId::decode(&mut &alice_id[..]).unwrap_or_default();
		let eth_address = EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]);
		let signature = hex!["ef9816023122208983c11e596446874df3d400d2f9e380a831206d0e91bfb96d54db352fbd62d3cfa8d8674cf63e6a32052ef3cab038e1e7398eac3d048ed5181c"];
		Claims::<T>::insert(eth_address, T::CurrencyBalance::from(1_000_000_000_000_000_000_u128));
	}: _(RawOrigin::Signed(caller.clone()), EcdsaSignature(signature))
	verify {
		assert_eq!(T::Currency::free_balance(&caller), T::CurrencyBalance::from(2_152_921_504_606_846_975_u128).into());
	}
}

#[cfg(test)]
mod tests {
	use super::mock::Test;
	use super::*;
	use crate::tests::new_test_ext;
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_claim::<Test>());
		});
	}
}
