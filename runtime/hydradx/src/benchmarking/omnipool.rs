use crate::{
	AccountId, AssetId, AssetRegistry, Balance, EmaOracle, Omnipool, Referrals, Runtime, RuntimeOrigin, System,
};

use super::*;

use frame_benchmarking::account;
use frame_support::dispatch::DispatchResult;
use frame_support::{
	assert_ok,
	sp_runtime::{
		traits::{One, SaturatedConversion, Zero},
		FixedU128, Permill,
	},
	traits::{OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;
use hydradx_traits::{
	router::{PoolType, TradeExecution},
	Registry,
};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_omnipool::types::Tradability;
use pallet_referrals::ReferralCode;

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(
		<<Runtime as pallet_omnipool::Config>::Currency as MultiCurrencyExtended<_>>::update_balance(
			currency_id,
			who,
			balance.saturated_into()
		)
	);
}

fn run_to_block(to: u32) {
	while System::block_number() < to {
		let b = System::block_number();

		System::on_finalize(b);
		EmaOracle::on_finalize(b);

		System::on_initialize(b + 1_u32);
		EmaOracle::on_initialize(b + 1_u32);

		System::set_block_number(b + 1_u32);
	}
}

const HDX: AssetId = 0;
const DAI: AssetId = 2;

fn init() -> DispatchResult {
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);

	let acc = Omnipool::protocol_account();

	update_balance(DAI, &acc, stable_amount);
	update_balance(HDX, &acc, native_amount);

	Omnipool::add_token(
		RawOrigin::Root.into(),
		HDX,
		native_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;
	Omnipool::add_token(
		RawOrigin::Root.into(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		acc,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{Runtime, pallet_omnipool}

	add_token{
		init()?;

		let acc = Omnipool::protocol_account();

		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price: FixedU128= FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		update_balance(token_id, &acc, token_amount);

		let current_position_id = Omnipool::next_position_id();

	}: { Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)? }
	verify {
		assert!(Omnipool::positions(current_position_id).is_some());
		assert!(Omnipool::assets(token_id).is_some());
	}

	add_liquidity {
		init()?;
		let acc = Omnipool::protocol_account();
		//Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
	}: { Omnipool::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)? }
	verify {
		assert!(Omnipool::positions(current_position_id).is_some());
	}

	remove_liquidity {
		init()?;
		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;

		// to ensure worst case - Let's do a trade to make sure price changes, so LP provider receives some LRNA ( which does additional transfer)
		let buyer: AccountId = account("buyer", 2, 1);
		update_balance(DAI, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, DAI, 100_000_000_000_u128, 100_000_000_000_000_u128)?;

		let hub_id = <Runtime as pallet_omnipool::Config>::HubAssetId::get();
		let hub_issuance = <Runtime as pallet_omnipool::Config>::Currency::total_issuance(hub_id);
	}: {Omnipool::remove_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), current_position_id, liquidity_added)? }
	verify {
		// Ensure NFT instance was burned
		assert!(Omnipool::positions(current_position_id).is_none());

		// Ensure LRNA was burned
		let hub_issuance_after  = <Runtime as pallet_omnipool::Config>::Currency::total_issuance(hub_id);
		assert!(hub_issuance_after < hub_issuance);
	}

	sell {
		init()?;
		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);
		update_balance(0, &owner, 1_000_000_000_000_000_u128);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner.clone())?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: AccountId = account("buyer", 2, 1);
		update_balance(DAI, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, DAI, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_sell = 100_000_000_000_u128;
		let buy_min_amount = 10_000_000_000_u128;

		// Register and link referral code to account for the weight too
		let code = ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"MYCODE".to_vec());
		Referrals::register_code(RawOrigin::Signed(owner).into(), code.clone())?;
		Referrals::link_code(RawOrigin::Signed(seller.clone()).into(), code)?;
	}: { Omnipool::sell(RawOrigin::Signed(seller.clone()).into(), token_id, DAI, amount_sell, buy_min_amount)? }
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(DAI, &seller) >= buy_min_amount);
	}

	buy {
		init()?;
		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1_u128)?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);
		update_balance(0, &owner, 1_000_000_000_000_000_u128);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner.clone())?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: AccountId = account("buyer", 2, 1);
		update_balance(DAI, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, DAI, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_buy = 1_000_000_000_000_u128;
		let sell_max_limit = 2_000_000_000_000_u128;
		// Register and link referral code to account for the weight too
		let code = ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"MYCODE".to_vec());
		Referrals::register_code(RawOrigin::Signed(owner).into(), code.clone())?;
		Referrals::link_code(RawOrigin::Signed(seller.clone()).into(), code)?;
	}: { Omnipool::buy(RawOrigin::Signed(seller.clone()).into(), DAI, token_id, amount_buy, sell_max_limit)? }
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(DAI, &seller) >= Balance::zero());
	}

	set_asset_tradable_state {
		init()?;
	}: { Omnipool::set_asset_tradable_state(RawOrigin::Root.into(), DAI, Tradability::BUY)? }
	verify {
		let asset_state = Omnipool::assets(DAI).unwrap();
		assert!(asset_state.tradable == Tradability::BUY);
	}

	refund_refused_asset {
		let recipient: AccountId = account("recipient", 3, 1);

		let asset_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1_u128)?;
		let amount = 1_000_000_000_000_000_u128;

		update_balance(asset_id, &Omnipool::protocol_account(), amount);

	}: {Omnipool::refund_refused_asset(RawOrigin::Root.into(), asset_id, amount, recipient.clone())? }
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(asset_id, &recipient) == amount);
	}

	sacrifice_position {
		init()?;
		let acc = Omnipool::protocol_account();
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;

	}: {Omnipool::sacrifice_position(RawOrigin::Signed(lp_provider).into(), current_position_id)? }
	verify {
		assert!(Omnipool::positions(current_position_id).is_none());
	}

	set_asset_weight_cap {
		init()?;
	}: { Omnipool::set_asset_weight_cap(RawOrigin::Root.into(), DAI, Permill::from_percent(10))? }
	verify {
		let asset_state = Omnipool::assets(DAI).unwrap();
		assert!(asset_state.cap == 100_000_000_000_000_000u128);
	}

	withdraw_protocol_liquidity {
		init()?;
		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), token_id, liquidity_added)?;

		let position =  Omnipool::positions(current_position_id).unwrap();

		Omnipool::sacrifice_position(RawOrigin::Signed(lp_provider).into(), current_position_id)?;

		let beneficiary: AccountId = account("beneficiary", 1, 1);

	}: { Omnipool::withdraw_protocol_liquidity(RawOrigin::Root.into(), token_id, position.shares, position.price, beneficiary.clone())? }
	verify {
		assert_eq!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(token_id, &beneficiary), liquidity_added);
	}

	remove_token{
		init()?;
		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::one())?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		let current_position_id = Omnipool::next_position_id();
		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price,Permill::from_percent(100), owner.clone())?;

		// Create LP provider account with correct balance
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		run_to_block(10);
		Omnipool::sacrifice_position(RawOrigin::Signed(owner).into(), current_position_id)?;

		Omnipool::set_asset_tradable_state(RawOrigin::Root.into(), token_id, Tradability::FROZEN)?;

		let beneficiary: AccountId = account("beneficiary", 1, 1);
	}: { Omnipool::remove_token(RawOrigin::Root.into(), token_id, beneficiary.clone())? }
	verify {
		assert_eq!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(token_id, &beneficiary), token_amount);
	}

	router_execution_sell {
		let c in 1..2;
		let e in 0..1;	// if e == 1, execute_sell is executed
		init()?;

		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1u128)?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: AccountId = account("buyer", 2, 1);
		update_balance(DAI, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, DAI, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_sell = 100_000_000_000_u128;
		let buy_min_amount = 10_000_000_000_u128;

	}: {
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_sell(PoolType::Omnipool, token_id, DAI, amount_sell).is_ok());
		if e != 0 {
			assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_sell(RawOrigin::Signed(seller.clone()).into(), PoolType::Omnipool, token_id, DAI, amount_sell, buy_min_amount).is_ok());
		}
	}
	verify {
		if e != 0 {
			assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(DAI, &seller) >= buy_min_amount);
		}
	}

	router_execution_buy {
		let c in 1..2;	// number of times calculate_buy is executed
		let e in 0..1;	// if e == 1, execute_buy is executed
		init()?;

		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = AssetRegistry::create_asset(&b"FCK".to_vec(), 1_u128)?;

		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner)?;

		// Create LP provider account with correct balance aand add some liquidity
		let lp_provider: AccountId = account("provider", 1, 1);
		update_balance(token_id, &lp_provider, 500_000_000_000_000_u128);

		let liquidity_added = 1_000_000_000_000_u128;

		let current_position_id = Omnipool::next_position_id();

		run_to_block(10);
		Omnipool::add_liquidity(RawOrigin::Signed(lp_provider).into(), token_id, liquidity_added)?;

		let buyer: AccountId = account("buyer", 2, 1);
		update_balance(DAI, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, DAI, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_buy = 1_000_000_000_000_u128;
		let sell_max_limit = 2_000_000_000_000_u128;

	}: {
		for _ in 1..c {
			assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_buy(PoolType::Omnipool, token_id, DAI, amount_buy).is_ok());
		}
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_buy(RawOrigin::Signed(seller.clone()).into(), PoolType::Omnipool, token_id, DAI, amount_buy, sell_max_limit).is_ok());
	}
	verify {
		if e != 0 {
			assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(DAI, &seller) >= Balance::zero());
		}
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(b"LRNA".to_vec(), 1_000u128, Some(1)),
				(b"DAI".to_vec(), 1_000u128, Some(2)),
			],
			native_asset_name: b"HDX".to_vec(),
			native_existential_deposit: NativeExistentialDeposit::get(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
