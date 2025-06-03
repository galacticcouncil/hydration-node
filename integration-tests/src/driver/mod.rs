mod example;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use frame_support::BoundedVec;
use hydradx_runtime::bifrost_account;
use hydradx_runtime::AssetLocation;
use hydradx_runtime::*;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::AggregatedPriceOracle;
use pallet_asset_registry::AssetType;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use primitives::constants::chain::{OMNIPOOL_SOURCE, STABLESWAP_SOURCE};
use primitives::{AccountId, AssetId};
use sp_runtime::{FixedU128, Permill};
use sp_std::cell::RefCell;
use xcm_emulator::TestExt;

type BoundedName = BoundedVec<u8, <hydradx_runtime::Runtime as pallet_asset_registry::Config>::StringLimit>;
pub(crate) struct HydrationTestDriver {
	omnipool_assets: Vec<AssetId>,
	stablepools: Vec<(AssetId, Vec<(AssetId, u8)>)>,
	ext: Option<RefCell<frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block>>>,
}

impl HydrationTestDriver {
	pub(crate) fn add_omnipool_assets(self, assets: Vec<AssetId>) -> Self {
		let mut driver = self;
		driver.omnipool_assets.extend(assets);
		driver
	}

	pub(crate) fn add_stablepools(self, pools: Vec<(AssetId, Vec<(AssetId, u8)>)>) -> Self {
		let mut driver = self;
		driver.stablepools.extend(pools);
		driver
	}
}

impl HydrationTestDriver {
	pub(crate) fn default() -> Self {
		TestNet::reset();
		HydrationTestDriver {
			omnipool_assets: vec![],
			stablepools: vec![],
			ext: None,
		}
	}

	pub(crate) fn with_snapshot(path: &str) -> Self {
		let ext = hydra_live_ext(path);
		let mut driver = Self::default();
		driver.ext = Some(RefCell::new(ext));
		driver
	}

	pub(crate) fn execute(&self, f: impl FnOnce()) -> &Self {
		if let Some(ref ext) = self.ext {
			ext.borrow_mut().execute_with(|| {
				f();
			});
		} else {
			Hydra::ext_wrapper(|| {
				f();
			});
		}
		self
	}

	pub(crate) fn execute_with_driver(&self, f: impl FnOnce(&Self)) -> &Self {
		if let Some(ref ext) = self.ext {
			ext.borrow_mut().execute_with(|| {
				f(&self);
			});
		} else {
			Hydra::ext_wrapper(|| {
				f(&self);
			});
		}
		self
	}

	pub(crate) fn setup_hydration(self) -> Self {
		self.setup_omnipool()
			.setup_stableswap()
			.add_stablepools_to_omnipool()
			.populate_oracle()
	}

	pub fn endow_account(&self, account: AccountId, asset_id: AssetId, amount: Balance) -> &Self {
		self.execute(|| {
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				account,
				asset_id,
				amount,
				0
			));
		});
		self
	}

	pub fn register_asset(
		self,
		asset_id: AssetId,
		name: &[u8],
		decimals: u8,
		location: Option<polkadot_xcm::v4::Location>,
	) -> Self {
		self.execute(|| {
			let location = location.map(|location| AssetLocation::try_from(location).unwrap());
			assert_ok!(AssetRegistry::register(
				RawOrigin::Root.into(),
				Some(asset_id),
				Some(BoundedName::truncate_from(name.to_vec())),
				AssetType::Token,
				Some(1000u128),
				Some(BoundedName::truncate_from(name.to_vec())),
				Some(decimals),
				location,
				None,
				true
			));
		});
		self
	}

	pub fn update_bifrost_oracle(
		&self,
		asset_a: Box<polkadot_xcm::VersionedLocation>,
		asset_b: Box<polkadot_xcm::VersionedLocation>,
		price: (Balance, Balance),
	) -> &Self {
		self.execute(|| {
			assert_ok!(EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(bifrost_account()),
				asset_a,
				asset_b,
				price,
			));
		});
		self
	}

	pub(crate) fn setup_omnipool(self) -> Self {
		self.execute(|| {
			let acc = hydradx_runtime::Omnipool::protocol_account();
			let native_price = FixedU128::from_rational(29903049701668757, 73927734532192294158);
			let dot_price = FixedU128::from_rational(103158291366950047, 4566210555614178);

			let dot_amount: primitives::Balance = 4566210555614178u128;
			let native_amount: primitives::Balance = 73927734532192294158u128;
			let weth_amount: primitives::Balance = 1074271742496220564487u128;
			let weth_price = FixedU128::from_rational(67852651072676287, 1074271742496220564487);

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				acc.clone(),
				DOT,
				dot_amount,
				0
			));
			Balances::set_balance(&acc, native_amount);
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				acc.clone(),
				WETH,
				weth_amount,
				0
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				HDX,
				native_price,
				Permill::from_percent(60),
				AccountId::from(ALICE),
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				dot_price,
				Permill::from_percent(60),
				AccountId::from(ALICE),
			));
			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				WETH,
				weth_price,
				Permill::from_percent(60),
				AccountId::from(ALICE),
			));
		});

		self.add_omnipool_assets(vec![HDX, DOT, WETH])
	}

	pub(crate) fn setup_stableswap(self) -> Self {
		let mut stable_pool_id = 0;
		let mut stable_assets = vec![];
		self.execute(|| {
			let possible_decimals: Vec<u8> = vec![6u8, 10u8, 12u8, 12u8, 18u8];
			let initial_liquidity = 1000u128;
			let mut asset_ids: Vec<(AssetId, u8)> = Vec::new();
			let mut initial: Vec<AssetAmount<AssetId>> = vec![];

			let asset_offset = 555u32;

			for idx in 0u32..MAX_ASSETS_IN_POOL {
				let name: Vec<u8> = idx.to_ne_bytes().to_vec();
				let decimals = possible_decimals[idx as usize % possible_decimals.len()];
				let result = AssetRegistry::register(
					RawOrigin::Root.into(),
					Some(asset_offset + idx),
					Some(name.clone().try_into().unwrap()),
					AssetType::Token,
					Some(1000u128),
					Some(name.try_into().unwrap()),
					Some(decimals),
					None,
					None,
					true,
				);
				assert_ok!(result);
				let asset_id = asset_offset + idx;
				asset_ids.push((asset_id, decimals));

				let liquidity = initial_liquidity * 10u128.pow(decimals as u32);

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					AccountId::from(BOB),
					asset_id,
					liquidity as i128,
				));
				initial.push(AssetAmount::new(asset_id, liquidity));
			}

			let pool_id = 222_222u32;
			let result = AssetRegistry::register(
				RawOrigin::Root.into(),
				Some(pool_id),
				Some(b"pool".to_vec().try_into().unwrap()),
				AssetType::StableSwap,
				Some(1u128),
				None,
				None,
				None,
				None,
				true,
			);
			assert_ok!(result);
			let amplification = 100u16;
			let fee = Permill::from_percent(1);

			assert_ok!(Stableswap::create_pool(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				BoundedVec::truncate_from(asset_ids.iter().map(|(id, _)| *id).collect()),
				amplification,
				fee,
			));

			assert_ok!(Stableswap::add_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
				pool_id,
				BoundedVec::truncate_from(initial)
			));
			stable_pool_id = pool_id;
			stable_assets = asset_ids;
		});

		self.add_stablepools(vec![(stable_pool_id, stable_assets)])
	}

	pub(crate) fn add_stablepools_to_omnipool(self) -> Self {
		self.execute(|| {
			let omnipool_acc = hydradx_runtime::Omnipool::protocol_account();
			for (pool_id, _) in self.stablepools.iter() {
				let pool_id_issuance = Tokens::total_issuance(pool_id);
				assert_ok!(hydradx_runtime::Currencies::transfer(
					hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
					omnipool_acc.clone(),
					*pool_id,
					pool_id_issuance,
				));
				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					*pool_id,
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));
			}
		});
		let stableids = self.stablepools.iter().map(|(pool_id, _)| *pool_id).collect();
		self.add_omnipool_assets(stableids)
	}

	fn populate_oracle(self) -> Self {
		//we need to make trades for each asset in omnipool
		//for at least 10 block to ensure SHORT oracle is updated too
		for _ in 1..=10 {
			self.new_block();
			let assets = self.omnipool_assets.clone();
			let stablepools = self.stablepools.clone();
			self.execute(|| {
				for asset_id in assets {
					let amount_to_sell = 1_000_000_000_000;
					assert_ok!(Tokens::set_balance(
						RawOrigin::Root.into(),
						CHARLIE.into(),
						crate::polkadot_test_net::LRNA,
						amount_to_sell,
						0,
					));

					assert_ok!(Omnipool::sell(
						RuntimeOrigin::signed(CHARLIE.into()),
						crate::polkadot_test_net::LRNA,
						asset_id,
						amount_to_sell,
						1
					));
				}

				for (pool_id, assets) in stablepools {
					for two_assets in assets.windows(2) {
						let asset_a = two_assets[0];
						let asset_b = two_assets[1];
						let amount = 10u128.pow(asset_a.1 as u32);
						assert_ok!(Tokens::set_balance(
							RawOrigin::Root.into(),
							CHARLIE.into(),
							asset_a.0,
							amount,
							0,
						));

						assert_ok!(Stableswap::sell(
							RuntimeOrigin::signed(CHARLIE.into()),
							pool_id,
							asset_a.0,
							asset_b.0,
							amount,
							1
						));
					}
				}
			});
		}

		self
	}

	pub fn new_block(&self) -> &Self {
		self.execute(|| {
			hydradx_run_to_next_block();
		});
		self
	}
}

#[test]
fn test_hydration_setup() {
	HydrationTestDriver::default()
		.setup_hydration()
		.execute_with_driver(|driver| {
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				CHARLIE.into(),
				DOT,
				20_000_000_000_000_000_000_000_000,
				0,
			));

			assert_ok!(Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
				DOT,
				HDX,
				1_000_000_000_000,
				0u128,
			));

			assert_eq!(driver.omnipool_assets, vec![HDX, DOT, WETH, 222_222]);
			assert!(!driver.stablepools.is_empty());

			let stablepool_1 = driver.stablepools[0].clone();
			let first_asset_id = stablepool_1.1[0].0;
			let pool_id = stablepool_1.0;

			// assert oracle initial values
			for supported_period in crate::oracle::SUPPORTED_PERIODS {
				assert!(
					EmaOracle::get_price(HDX, crate::polkadot_test_net::LRNA, *supported_period, OMNIPOOL_SOURCE)
						.is_ok()
				);
				assert!(
					EmaOracle::get_price(DOT, crate::polkadot_test_net::LRNA, *supported_period, OMNIPOOL_SOURCE)
						.is_ok()
				);
				assert!(EmaOracle::get_price(first_asset_id, pool_id, *supported_period, STABLESWAP_SOURCE).is_ok());
			}
		});
}
