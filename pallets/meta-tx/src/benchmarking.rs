// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

//! Measures the reject path: decode, nonce read and one signature verification.

use super::*;

use codec::Decode;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_std::boxed::Box;
use sp_std::vec;

benchmarks! {
	where_clause { where
		T: crate::Config,
	}

	dispatch_meta_tx {
		let relayer: T::AccountId = account("relayer", 0, 0);
		let signer: T::AccountId = account("signer", 0, 0);
		let call: <T as crate::Config>::RuntimeCall = frame_system::Call::remark { remark: vec![] }.into();
		let deadline = frame_system::Pallet::<T>::block_number();
		let signature = T::Signature::decode(&mut &[0u8; 65][..])
			.expect("signature type decodes from a 65 byte buffer; qed");
	}: {
		let _ = crate::Pallet::<T>::dispatch_meta_tx(
			RawOrigin::Signed(relayer).into(),
			signer,
			Box::new(call),
			0,
			deadline,
			signature,
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
