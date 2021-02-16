use super::*;

pub fn migrate_to_v2<T: Config>() -> frame_support::weights::Weight {
    if PalletVersion::get() == StorageVersion::V1EmptyBalances {
        frame_support::debug::info!(" >>> Adding claims to the storage");
        for (addr, amount) in claims_data::CLAIMS_DATA.iter() {
            Claims::<T>::insert(
                EthereumAddress(<[u8; 20]>::from_hex(&addr[2..]).unwrap_or_else(|addr| {
                    frame_support::debug::warn!("Error encountered while migrating Ethereum address: {}", addr);
                    EthereumAddress::default().0
                })),
                amount,
            );
        }
        PalletVersion::put(StorageVersion::V2AddClaimData);
        T::DbWeight::get().reads_writes(2, 3)
    } else {
        frame_support::debug::info!(" >>> Unused migration");
        0
    }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::Test;

	#[test]
	fn data_migration_should_work() {
		sp_io::TestExternalities::default().execute_with(|| {
			// we need testing data to prevent a false-positive test result
			assert!(claims_data::CLAIMS_DATA.len() >= 3);
			let (first_addr, first_balance) = claims_data::CLAIMS_DATA[0];
			let (second_addr, second_balance) = claims_data::CLAIMS_DATA[1];
			let (last_addr, last_balance) = claims_data::CLAIMS_DATA.last().copied().unwrap();

			let first_addr = EthereumAddress(<[u8; 20]>::from_hex(&first_addr[2..]).unwrap());
			let second_addr = EthereumAddress(<[u8; 20]>::from_hex(&second_addr[2..]).unwrap());
			let last_addr = EthereumAddress(<[u8; 20]>::from_hex(&last_addr[2..]).unwrap());
			assert_eq!(Claims::<Test>::get(first_addr), 0);
			assert_eq!(Claims::<Test>::get(second_addr), 0);
			assert_eq!(Claims::<Test>::get(last_addr), 0);

			assert_eq!(PalletVersion::get(), StorageVersion::V1EmptyBalances);
			migrate_to_v2::<Test>();
			assert_eq!(PalletVersion::get(), StorageVersion::V2AddClaimData);

			assert_eq!(Claims::<Test>::get(first_addr), first_balance);
			assert_eq!(Claims::<Test>::get(second_addr), second_balance);
			assert_eq!(Claims::<Test>::get(last_addr), last_balance);
		})
	}
}
