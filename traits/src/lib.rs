// This file is part of hydradx-traits.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::upper_case_acronyms)]

pub mod liquidity_mining;
pub mod nft;
pub mod pools;
pub mod registry;
pub mod router;

pub use registry::*;
pub mod oracle;
pub mod price;

pub use oracle::*;

use codec::{Decode, Encode};
use frame_support::dispatch::{self};
use frame_support::sp_runtime::{traits::Zero, DispatchError, RuntimeDebug};
use frame_support::traits::LockIdentifier;
use frame_support::weights::Weight;
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

/// Hold information to perform amm transfer
/// Contains also exact amount which will be sold/bought
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, Default)]
pub struct AMMTransfer<AccountId, AssetId, AssetPair, Balance> {
	pub origin: AccountId,
	pub assets: AssetPair,
	pub amount: Balance,
	pub amount_b: Balance,
	pub discount: bool,
	pub discount_amount: Balance,
	pub fee: (AssetId, Balance),
}

/// Traits for handling AMM Pool trades.
pub trait AMM<AccountId, AssetId, AssetPair, Amount: Zero> {
	/// Check if both assets exist in a pool.
	fn exists(assets: AssetPair) -> bool;

	/// Return pair account.
	fn get_pair_id(assets: AssetPair) -> AccountId;

	/// Return share token for assets.
	fn get_share_token(assets: AssetPair) -> AssetId;

	/// Return list of active assets in a given pool.
	fn get_pool_assets(pool_account_id: &AccountId) -> Option<Vec<AssetId>>;

	/// Calculate spot price for asset a and b.
	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Amount) -> Amount;

	/// Sell trade validation
	/// Perform all necessary checks to validate an intended sale.
	fn validate_sell(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		min_bought: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetId, AssetPair, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_sell(transfer: &AMMTransfer<AccountId, AssetId, AssetPair, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	/// Call execute following the validation.
	fn sell(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		min_bought: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_sell(&Self::validate_sell(origin, assets, amount, min_bought, discount)?)?;
		Ok(())
	}

	/// Buy trade validation
	/// Perform all necessary checks to validate an intended buy.
	fn validate_buy(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		max_limit: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetId, AssetPair, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_buy(transfer: &AMMTransfer<AccountId, AssetId, AssetPair, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	fn buy(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		max_limit: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_buy(&Self::validate_buy(origin, assets, amount, max_limit, discount)?)?;
		Ok(())
	}

	fn get_min_trading_limit() -> Amount;

	fn get_min_pool_liquidity() -> Amount;

	fn get_max_in_ratio() -> u128;

	fn get_max_out_ratio() -> u128;

	fn get_fee(pool_account_id: &AccountId) -> (u32, u32);
}

pub trait Resolver<AccountId, Intention, E> {
	/// Resolve an intention directl via AMM pool.
	fn resolve_single_intention(intention: &Intention);

	/// Resolve intentions by either directly trading with each other or via AMM pool.
	/// Intention ```intention``` must be validated prior to call this function.
	fn resolve_matched_intentions(pair_account: &AccountId, intention: &Intention, matched: &[&Intention]);
}

/// Handler used by AMM pools to perform some tasks when a new pool is created.
pub trait OnCreatePoolHandler<AssetId> {
	/// Register an asset to be handled by price-oracle pallet.
	/// If an asset is not registered, calling `on_trade` results in populating the price buffer in the price oracle pallet,
	/// but the entries are ignored and the average price for the asset is not calculated.
	fn on_create_pool(asset_a: AssetId, asset_b: AssetId) -> dispatch::DispatchResult;
}

impl<AssetId> OnCreatePoolHandler<AssetId> for () {
	fn on_create_pool(_asset_a: AssetId, _asset_b: AssetId) -> dispatch::DispatchResult {
		Ok(())
	}
}

/// Handler used by AMM pools to perform some tasks when a trade is executed.
pub trait OnTradeHandler<AssetId, Balance, Price> {
	/// Include a trade in the average price calculation of the price-oracle pallet.
	#[allow(clippy::too_many_arguments)]
	fn on_trade(
		source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		amount_a: Balance,
		amount_b: Balance,
		liquidity_a: Balance,
		liquidity_b: Balance,
		price: Price,
	) -> Result<Weight, (Weight, DispatchError)>;
	/// Known overhead for a trade in `on_initialize/on_finalize`.
	/// Needs to be specified here if we don't want to make AMM pools tightly coupled with the price oracle pallet, otherwise we can't access the weight.
	/// Add this weight to an extrinsic from which you call `on_trade`.
	fn on_trade_weight() -> Weight;
}

impl<AssetId, Balance, Price> OnTradeHandler<AssetId, Balance, Price> for () {
	fn on_trade(
		_source: Source,
		_asset_a: AssetId,
		_asset_b: AssetId,
		_amount_a: Balance,
		_amount_b: Balance,
		_liquidity_a: Balance,
		_liquidity_b: Balance,
		_price: Price,
	) -> Result<Weight, (Weight, DispatchError)> {
		Ok(Weight::zero())
	}
	fn on_trade_weight() -> Weight {
		Weight::zero()
	}
}

pub trait CanCreatePool<AssetId> {
	fn can_create(asset_a: AssetId, asset_b: AssetId) -> bool;
}

pub trait LockedBalance<AssetId, AccountId, Balance> {
	fn get_by_lock(lock_id: LockIdentifier, currency_id: AssetId, who: AccountId) -> Balance;
}

/// Handler used by AMM pools to perform some tasks when liquidity changes outside of trades.
pub trait OnLiquidityChangedHandler<AssetId, Balance, Price> {
	/// Notify that the liquidity for a pair of assets has changed.
	#[allow(clippy::too_many_arguments)]
	fn on_liquidity_changed(
		source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		amount_a: Balance,
		amount_b: Balance,
		liquidity_a: Balance,
		liquidity_b: Balance,
		price: Price,
	) -> Result<Weight, (Weight, DispatchError)>;
	/// Known overhead for a liquidity change in `on_initialize/on_finalize`.
	/// Needs to be specified here if we don't want to make AMM pools tightly coupled with the price oracle pallet, otherwise we can't access the weight.
	/// Add this weight to an extrinsic from which you call `on_liquidity_changed`.
	fn on_liquidity_changed_weight() -> Weight;
}

impl<AssetId, Balance, Price> OnLiquidityChangedHandler<AssetId, Balance, Price> for () {
	fn on_liquidity_changed(
		_source: Source,
		_a: AssetId,
		_b: AssetId,
		_amount_a: Balance,
		_amount_b: Balance,
		_liq_a: Balance,
		_liq_b: Balance,
		_price: Price,
	) -> Result<Weight, (Weight, DispatchError)> {
		Ok(Weight::zero())
	}

	fn on_liquidity_changed_weight() -> Weight {
		Weight::zero()
	}
}

/// Implementers of this trait provides information about user's position in the AMM pool.
pub trait AMMPosition<AssetId, Balance> {
	type Error;

	/// This function calculates amount of assets behind the `share_token`s.
	fn get_liquidity_behind_shares(
		asset_a: AssetId,
		asset_b: AssetId,
		shares_amount: Balance,
	) -> Result<(Balance, Balance), Self::Error>;
}
