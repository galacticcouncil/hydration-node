use super::*;

/// Hold info about each transfer which has to be made to resolve a direct trade.
pub struct Transfer<'a, T: Trait> {
	pub from: &'a T::AccountId,
	pub to: &'a T::AccountId,
	pub asset: AssetId,
	pub amount: Balance,
	pub fee_transfer: bool,
}

/// Hold info about a direct trade between two intentions.
/// After a direct trade is prepared - ```transfers``` contains all necessary transfers to complete the trade.
pub struct DirectTradeData<'a, T: Trait> {
	pub intention_a: &'a ExchangeIntention<T::AccountId, AssetId, Balance>,
	pub intention_b: &'a ExchangeIntention<T::AccountId, AssetId, Balance>,
	pub amount_from_a: Balance,
	pub amount_from_b: Balance,
	pub transfers: Vec<Transfer<'a, T>>,
}

/// Direct trading implementaton
impl<'a, T: Trait> DirectTradeData<'a, T> {
	/// Prepare direct trade
	/// 1. Validate balances
	/// 2. Calculate fees
	/// 3. Reserve amounts for each transfer ( including fee transfers )
	pub fn prepare(&mut self, pool_account: &'a T::AccountId) -> bool {
		if T::Currency::free_balance(self.intention_a.asset_sell, &self.intention_a.who) < self.amount_from_a {
			Self::send_insufficient_balance_event(self.intention_a, self.intention_a.asset_sell);
			return false;
		}
		if T::Currency::free_balance(self.intention_a.asset_buy, &self.intention_b.who) < self.amount_from_b {
			Self::send_insufficient_balance_event(self.intention_b, self.intention_a.asset_buy);
			return false;
		}

		if !Self::reserve_if_can(self.intention_a.asset_sell, &self.intention_a.who, self.amount_from_a) {
			return false;
		}
		if !Self::reserve_if_can(self.intention_a.asset_buy, &self.intention_b.who, self.amount_from_b) {
			return false;
		}

		let transfer = Transfer::<T> {
			from: &self.intention_a.who,
			to: &self.intention_b.who,
			asset: self.intention_a.asset_sell,
			amount: self.amount_from_a,
			fee_transfer: false,
		};
		self.transfers.push(transfer);
		let transfer = Transfer::<T> {
			from: &self.intention_b.who,
			to: &self.intention_a.who,
			asset: self.intention_a.asset_buy,
			amount: self.amount_from_b,
			fee_transfer: false,
		};
		self.transfers.push(transfer);

		// Let's handle the fees now for registered transfers.

		let transfer_a_fee = fee::get_fee(self.amount_from_a).unwrap();
		let transfer_b_fee = fee::get_fee(self.amount_from_b).unwrap();

		// Work out where to a fee from.
		// There are multiple possible scenarios to consider
		// 1. SELL - SELL
		// 2. SELL - BUY
		// 3. BUY - SELL
		// 4. BUY - BUY
		// Each one is handled slightly different, hence the complicated match statement.
		match (&self.intention_a.sell_or_buy, &self.intention_b.sell_or_buy) {
			(IntentionType::SELL, IntentionType::SELL) => {
				if !Self::reserve_if_can(self.intention_a.asset_buy, &self.intention_a.who, transfer_b_fee) {
					return false;
				}
				if !Self::reserve_if_can(self.intention_b.asset_buy, &self.intention_b.who, transfer_a_fee) {
					return false;
				}

				let transfer = Transfer::<T> {
					from: &self.intention_a.who,
					to: pool_account,
					asset: self.intention_a.asset_buy,
					amount: transfer_b_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);

				let transfer = Transfer::<T> {
					from: &self.intention_b.who,
					to: pool_account,
					asset: self.intention_b.asset_buy,
					amount: transfer_a_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);
			}
			(IntentionType::BUY, IntentionType::BUY) => {
				if !Self::reserve_if_can(self.intention_a.asset_sell, &self.intention_a.who, transfer_a_fee) {
					return false;
				}
				if !Self::reserve_if_can(self.intention_b.asset_sell, &self.intention_b.who, transfer_b_fee) {
					return false;
				}

				let transfer = Transfer::<T> {
					from: &self.intention_a.who,
					to: pool_account,
					asset: self.intention_a.asset_sell,
					amount: transfer_a_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);

				let transfer = Transfer::<T> {
					from: &self.intention_b.who,
					to: pool_account,
					asset: self.intention_b.asset_sell,
					amount: transfer_b_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);
			}
			(IntentionType::BUY, IntentionType::SELL) => {
				if !Self::reserve_if_can(self.intention_a.asset_sell, &self.intention_a.who, transfer_a_fee) {
					return false;
				}
				if !Self::reserve_if_can(self.intention_b.asset_buy, &self.intention_b.who, transfer_b_fee) {
					return false;
				}

				let transfer = Transfer::<T> {
					from: &self.intention_a.who,
					to: pool_account,
					asset: self.intention_a.asset_sell,
					amount: transfer_a_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);

				let transfer = Transfer::<T> {
					from: &self.intention_b.who,
					to: pool_account,
					asset: self.intention_b.asset_buy,
					amount: transfer_b_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);
			}
			(IntentionType::SELL, IntentionType::BUY) => {
				if !Self::reserve_if_can(self.intention_a.asset_buy, &self.intention_a.who, transfer_a_fee) {
					return false;
				}
				if !Self::reserve_if_can(self.intention_b.asset_sell, &self.intention_b.who, transfer_b_fee) {
					return false;
				}

				let transfer = Transfer::<T> {
					from: &self.intention_a.who,
					to: pool_account,
					asset: self.intention_a.asset_buy,
					amount: transfer_a_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);

				let transfer = Transfer::<T> {
					from: &self.intention_b.who,
					to: pool_account,
					asset: self.intention_b.asset_sell,
					amount: transfer_b_fee,
					fee_transfer: true,
				};
				self.transfers.push(transfer);
			}
		}

		true
	}

	/// Execute direct trade.
	/// Trade must be prepared first. Execute all transfers.
	pub fn execute(&self) -> bool {
		self.send_direct_trade_resolve_event();
		for transfer in &self.transfers {
			//TODO: check method to just moved already reserved ( and not do unreserve -> transfer )

			T::Currency::unreserve(transfer.asset, transfer.from, transfer.amount);
			T::Currency::transfer(transfer.asset, transfer.from, transfer.to, transfer.amount)
				.expect("Cannot fail. Checks should have been done prior to this.");

			if transfer.fee_transfer {
				Self::send_trade_fee_event(transfer.from, transfer.to, transfer.asset, transfer.amount);
			}
		}
		true
	}

	/// Revert all reserverd amounts.
	/// This does NOT revert transfers, only reserved amounts. So it can be only called if a preparation fails.
	pub fn revert(&mut self) {
		for transfer in &self.transfers {
			T::Currency::unreserve(transfer.asset, transfer.from, transfer.amount);
		}
	}

	/// Send pallet event in case of insufficient balance.
	fn send_insufficient_balance_event(intention: &Intention<T>, asset: AssetId) {
		Module::<T>::deposit_event(RawEvent::InsufficientAssetBalanceEvent(
			intention.who.clone(),
			asset,
			intention.sell_or_buy.clone(),
			intention.intention_id,
			Error::<T>::InsufficientAssetBalance.into(),
		));
	}

	/// Send pallet event after a free is transferred.
	fn send_trade_fee_event(from: &T::AccountId, to: &T::AccountId, asset: AssetId, amount: Balance) {
		Module::<T>::deposit_event(RawEvent::IntentionResolvedDirectTradeFees(
			from.clone(),
			to.clone(),
			asset,
			amount,
		));
	}

	/// Send event after successful direct trade.
	fn send_direct_trade_resolve_event(&self) {
		Module::<T>::deposit_event(RawEvent::IntentionResolvedDirectTrade(
			self.intention_a.who.clone(),
			self.intention_b.who.clone(),
			self.intention_a.intention_id,
			self.intention_b.intention_id,
			self.amount_from_a,
			self.amount_from_b,
		));
	}

	/// Reserve amount.
	fn reserve_if_can(asset: AssetId, who: &T::AccountId, amount: Balance) -> bool {
		match T::Currency::reserve(asset, who, amount) {
			Ok(_) => true,
			Err(_) => false,
		}
	}
}
