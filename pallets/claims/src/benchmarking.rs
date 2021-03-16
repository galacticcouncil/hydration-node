#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;

benchmarks! {
	claim {
		let alice_id = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"];
		let signature = hex!["bcae7d4f96f71cf974c173ae936a1a79083af7f76232efbf8a568b7f990eceed73c2465bba769de959b7f6ac5690162b61eb90949901464d0fa158a83022a0741c"];

		#[cfg(test)]
		let alice_id = hex!["2a00000000000000"];
		#[cfg(test)]
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		let caller = T::AccountId::decode(&mut &alice_id[..]).unwrap_or_default();
		let eth_address = EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]);
		Claims::<T>::insert(eth_address, T::CurrencyBalance::from(1_000_000_000_000_000_000_u128).into());
	}: _(RawOrigin::Signed(caller.clone()), EcdsaSignature(signature))
	verify {
		let expected_balance = T::CurrencyBalance::from(2_000_000_000_000_000_000_u128);

		#[cfg(test)]
		let expected_balance = T::CurrencyBalance::from(1_000_000_000_000_000_000_u128);

		assert_eq!(T::Currency::free_balance(&caller), expected_balance.into());
		assert_eq!(Claims::<T>::get(eth_address), T::CurrencyBalance::from(0u128).into());
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
