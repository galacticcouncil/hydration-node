use crate::{AccountId, AssetId, AssetRegistry, Balance, EmaOracle, Omnipool, Runtime, RuntimeOrigin, System};

use super::*;

use frame_benchmarking::account;
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

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(
		<<Runtime as pallet_omnipool::Config>::Currency as MultiCurrencyExtended<_>>::update_balance(
			currency_id,
			who,
			balance.saturated_into()
		)
	);
}

const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;

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

runtime_benchmarks! {
	{Runtime, pallet_omnipool}

	initialize_pool {
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

	}: { Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price, Permill::from_percent(100), Permill::from_percent(100))? }
	verify {
		assert!(Omnipool::assets(stable_id).is_some());
		assert!(Omnipool::assets(native_id).is_some());
	}

	add_token{
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		update_balance(stable_id, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, stable_id, 100_000_000_000_u128, 100_000_000_000_000_u128)?;

	}: {Omnipool::remove_liquidity(RawOrigin::Signed(lp_provider.clone()).into(), current_position_id, liquidity_added)? }
	verify {
		// Ensure NFT instance was burned
		assert!(Omnipool::positions(current_position_id).is_none());

		// Ensure lp provider received LRNA
		let hub_id = <Runtime as pallet_omnipool::Config>::HubAssetId::get();
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(hub_id, &lp_provider) > Balance::zero());
	}

	sell {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		update_balance(stable_id, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, stable_id, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_sell = 100_000_000_000_u128;
		let buy_min_amount = 10_000_000_000_u128;

	}: { Omnipool::sell(RawOrigin::Signed(seller.clone()).into(), token_id, stable_id, amount_sell, buy_min_amount)? }
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(stable_id, &seller) >= buy_min_amount);
	}

	buy {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		update_balance(stable_id, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, stable_id, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_buy = 1_000_000_000_000_u128;
		let sell_max_limit = 2_000_000_000_000_u128;

	}: { Omnipool::buy(RawOrigin::Signed(seller.clone()).into(), stable_id, token_id, amount_buy, sell_max_limit)? }
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(stable_id, &seller) >= Balance::zero());
	}

	set_asset_tradable_state {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price, Permill::from_percent(100), Permill::from_percent(100))?;

	}: { Omnipool::set_asset_tradable_state(RawOrigin::Root.into(), stable_id, Tradability::BUY)? }
	verify {
		let asset_state = Omnipool::assets(stable_id).unwrap();
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
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price, Permill::from_percent(100), Permill::from_percent(100))?;

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

	}: {Omnipool::sacrifice_position(RawOrigin::Signed(lp_provider).into(), current_position_id)? }
	verify {
		assert!(Omnipool::positions(current_position_id).is_none());
	}

	set_asset_weight_cap {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price, Permill::from_percent(100), Permill::from_percent(100))?;

	}: { Omnipool::set_asset_weight_cap(RawOrigin::Root.into(), stable_id, Permill::from_percent(10))? }
	verify {
		let asset_state = Omnipool::assets(stable_id).unwrap();
		assert!(asset_state.cap == 100_000_000_000_000_000u128);
	}

	router_execution_sell {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128 = FixedU128::from((1,2));
		let native_price: FixedU128 = FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		update_balance(stable_id, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, stable_id, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_sell = 100_000_000_000_u128;
		let buy_min_amount = 10_000_000_000_u128;

	}: {
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_sell(PoolType::Omnipool, token_id, stable_id, amount_sell).is_ok());
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_sell(RawOrigin::Signed(seller.clone()).into(), PoolType::Omnipool, token_id, stable_id, amount_sell, buy_min_amount).is_ok());
	}
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(stable_id, &seller) >= buy_min_amount);
	}

	router_execution_buy {
		// Initialize pool
		let stable_amount: Balance = 1_000_000_000_000_000u128;
		let native_amount: Balance = 1_000_000_000_000_000u128;
		let stable_price: FixedU128= FixedU128::from((1,2));
		let native_price: FixedU128= FixedU128::from(1);

		let acc = Omnipool::protocol_account();
		let native_id = <Runtime as pallet_omnipool::Config>::HdxAssetId::get();
		let stable_id = <Runtime as pallet_omnipool::Config>::StableCoinAssetId::get();

		Omnipool::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP)?;

		update_balance(stable_id, &acc, stable_amount);
		update_balance(native_id, &acc, native_amount);

		Omnipool::initialize_pool(RawOrigin::Root.into(), stable_price, native_price,Permill::from_percent(100), Permill::from_percent(100))?;

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
		update_balance(stable_id, &buyer, 500_000_000_000_000_u128);
		Omnipool::buy(RawOrigin::Signed(buyer).into(), token_id, stable_id, 30_000_000_000_000_u128, 100_000_000_000_000_u128)?;

		let seller: AccountId = account("seller", 3, 1);
		update_balance(token_id, &seller, 500_000_000_000_000_u128);

		let amount_buy = 1_000_000_000_000_u128;
		let sell_max_limit = 2_000_000_000_000_u128;

	}: {
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_buy(PoolType::Omnipool, token_id, stable_id, amount_buy).is_ok());
		assert!(<Omnipool as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_buy(RawOrigin::Signed(seller.clone()).into(), PoolType::Omnipool, token_id, stable_id, amount_buy, sell_max_limit).is_ok());
	}
	verify {
		assert!(<Runtime as pallet_omnipool::Config>::Currency::free_balance(stable_id, &seller) >= Balance::zero());
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use frame_support::traits::GenesisBuild;
	use orml_benchmarking::impl_benchmark_test_suite;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<crate::Runtime>()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(Some(1), Some(b"LRNA".to_vec()), 1_000u128, None, None, None, false),
				(Some(2), Some(b"DAI".to_vec()), 1_000u128, None, None, None, false),
			],
			native_asset_name: b"HDX".to_vec(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
