//! The mapper: decode SCALE `System.Events` into cerebro-core's normalized `ChainEvent` stream.
//! This is the ONLY place that matches on `hydradx_runtime::RuntimeEvent`. Everything downstream
//! (core) is runtime-agnostic.

use cerebro_core::model::{ChainEvent, DebtLeg, Notable, Phase, Trade, TransferLeg};
use cerebro_core::{AssetId, EvmAddress, NATIVE_ASSET};
use codec::Decode;
use hydradx_runtime::RuntimeEvent;
use primitives::AccountId;
use sp_core::{H160, H256};

pub type EventRecord = frame_system::EventRecord<RuntimeEvent, H256>;

/// A decoded event: its phase, the normalized event and the raw Aave leg awaiting reserve resolution.
pub type Decoded = (Phase, ChainEvent, Option<RawDebtLeg>);

/// A raw Aave debt leg before reserve→asset resolution (which needs chain access).
pub struct RawDebtLeg {
	pub who: EvmAddress,
	pub reserve: H160,
	pub amount: i128,
	/// Supply/withdraw (readable aToken counter-leg) vs borrow/repay (unreadable debt token).
	pub collateral: bool,
}

/// Decode a block's events into `(Phase, ChainEvent)` in event order, plus the raw debt legs whose
/// reserve→asset resolution the caller finishes with chain access. Debt legs are emitted as
/// `ChainEvent::AavePosition` with `asset` filled by the caller (see `map_block`).
pub fn decode_events(events_raw: &[u8]) -> Result<Vec<Decoded>, String> {
	let mut input = events_raw;
	let records: Vec<EventRecord> = Decode::decode(&mut input).map_err(|e| format!("decode System.Events: {e}"))?;
	Ok(map_records(&records))
}

/// Decode records stored back to back with no length prefix, e.g. a suffix of `System.Events`.
/// Every byte must belong to a whole record.
pub fn decode_record_stream(mut bytes: &[u8]) -> Result<Vec<EventRecord>, String> {
	let mut out = Vec::new();
	while !bytes.is_empty() {
		out.push(EventRecord::decode(&mut bytes).map_err(|e| format!("decode EventRecord: {e}"))?);
	}
	Ok(out)
}

/// [`decode_events`] over already decoded records.
pub fn map_records(records: &[EventRecord]) -> Vec<Decoded> {
	let mut out = Vec::new();
	for rec in records {
		let phase = match rec.phase {
			frame_system::Phase::ApplyExtrinsic(i) => Phase::ApplyExtrinsic(i),
			_ => Phase::Hook,
		};
		let mapped = chain_events(&rec.event);
		let debt = raw_debt_leg(&rec.event);
		// Every EventRecord must touch its extrinsic's per_ext entry so `n_ext` counts it (matching
		// the reference), even if it carried no economic signal (ExtrinsicSuccess, timestamp, …).
		if mapped.is_empty() && debt.is_none() {
			out.push((phase, ChainEvent::Inert, None));
		}
		for ev in mapped {
			out.push((phase, ev, None));
		}
		if let Some(raw) = debt {
			// The AavePosition ChainEvent's asset is resolved by the caller; carry the raw leg
			// alongside a placeholder event so ordering is preserved.
			out.push((
				phase,
				ChainEvent::AavePosition(DebtLeg {
					who: raw.who,
					asset: None,
					amount: raw.amount,
					collateral: raw.collateral,
				}),
				Some(raw),
			));
		}
	}
	out
}

/// Translate one runtime event into zero or more `ChainEvent`s (excluding Aave legs, handled
/// separately for reserve resolution).
fn chain_events(ev: &RuntimeEvent) -> Vec<ChainEvent> {
	// Fee / signer.
	if let RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {
		who,
		actual_fee,
		..
	}) = ev
	{
		return vec![ChainEvent::FeePaid {
			who: acc(who),
			amount: *actual_fee,
		}];
	}
	// Reward claims (earned yield, folded out of profit): staking pays native HDX; the two LM
	// pallets pay their farm's reward currency.
	if let RuntimeEvent::Staking(pallet_staking::Event::RewardsClaimed { who, paid_rewards, .. }) = ev {
		return vec![
			ChainEvent::RewardClaimed {
				who: acc(who),
				asset: NATIVE_ASSET,
				amount: *paid_rewards,
			},
			ChainEvent::Notable(Notable::Reward { venue: "staking" }),
		];
	}
	if let RuntimeEvent::OmnipoolLiquidityMining(pallet_omnipool_liquidity_mining::Event::RewardClaimed {
		who,
		claimed,
		reward_currency,
		..
	}) = ev
	{
		return vec![
			ChainEvent::RewardClaimed {
				who: acc(who),
				asset: *reward_currency,
				amount: *claimed,
			},
			ChainEvent::Notable(Notable::Reward { venue: "omnipool-lm" }),
		];
	}
	if let RuntimeEvent::XYKLiquidityMining(pallet_xyk_liquidity_mining::Event::RewardClaimed {
		who,
		claimed,
		reward_currency,
		..
	}) = ev
	{
		return vec![
			ChainEvent::RewardClaimed {
				who: acc(who),
				asset: *reward_currency,
				amount: *claimed,
			},
			ChainEvent::Notable(Notable::Reward { venue: "xyk-lm" }),
		];
	}
	// DCA lifecycle boundaries.
	if let RuntimeEvent::DCA(e) = ev {
		use pallet_dca::Event as E;
		match e {
			E::ExecutionStarted { id, .. } => return vec![ChainEvent::DcaStarted { schedule_id: *id }],
			E::TradeExecuted { id, who, .. }
			| E::TradeFailed { id, who, .. }
			| E::Terminated { id, who, .. }
			| E::Completed { id, who } => {
				return vec![ChainEvent::DcaClosed {
					schedule_id: *id,
					owner: acc(who),
				}]
			}
			_ => {}
		}
	}
	// Trades (router = whole route, direct = a single hop). Emit the Trade (for the price graph)
	// and a Notable (for labels).
	if let Some(t) = trade_of(ev) {
		let mut out = vec![ChainEvent::Trade(t.clone())];
		if let Some(n) = notable_of(ev) {
			out.push(ChainEvent::Notable(n));
		}
		return out;
	}
	// Liquidity: flag (gates R1) + Notable (label).
	if is_lp_event(ev) {
		let mut out = vec![ChainEvent::Liquidity];
		if let Some(n) = notable_of(ev) {
			out.push(ChainEvent::Notable(n));
		}
		return out;
	}
	// OTC actions carry no balance leg of their own — Notable only.
	if let Some(n) = notable_of(ev) {
		return vec![ChainEvent::Notable(n)];
	}
	// Transfer legs (may be several).
	legs_of(ev).into_iter().map(ChainEvent::Transfer).collect()
}

/// A block-log-worthy action (for human labels) — mirrors the reference `notable_event`.
fn notable_of(ev: &RuntimeEvent) -> Option<Notable> {
	Some(match ev {
		RuntimeEvent::OTC(pallet_otc::Event::Placed { .. }) => Notable::Otc { kind: "place-order" },
		RuntimeEvent::OTC(pallet_otc::Event::Filled { .. } | pallet_otc::Event::PartiallyFilled { .. }) => {
			Notable::Otc { kind: "settle/fill" }
		}
		RuntimeEvent::Router(pallet_route_executor::Event::Executed {
			asset_in, asset_out, ..
		}) => Notable::Trade {
			venue: "router",
			asset_in: *asset_in,
			asset_out: *asset_out,
		},
		RuntimeEvent::Omnipool(
			pallet_omnipool::Event::SellExecuted {
				asset_in, asset_out, ..
			}
			| pallet_omnipool::Event::BuyExecuted {
				asset_in, asset_out, ..
			},
		) => Notable::Trade {
			venue: "omnipool",
			asset_in: *asset_in,
			asset_out: *asset_out,
		},
		RuntimeEvent::Stableswap(
			pallet_stableswap::Event::SellExecuted {
				asset_in, asset_out, ..
			}
			| pallet_stableswap::Event::BuyExecuted {
				asset_in, asset_out, ..
			},
		) => Notable::Trade {
			venue: "stableswap",
			asset_in: *asset_in,
			asset_out: *asset_out,
		},
		RuntimeEvent::Omnipool(pallet_omnipool::Event::LiquidityAdded { .. }) => Notable::Liquidity {
			venue: "omnipool",
			added: true,
		},
		RuntimeEvent::Omnipool(pallet_omnipool::Event::LiquidityRemoved { .. }) => Notable::Liquidity {
			venue: "omnipool",
			added: false,
		},
		RuntimeEvent::Stableswap(pallet_stableswap::Event::LiquidityAdded { .. }) => Notable::Liquidity {
			venue: "stableswap",
			added: true,
		},
		RuntimeEvent::Stableswap(pallet_stableswap::Event::LiquidityRemoved { .. }) => Notable::Liquidity {
			venue: "stableswap",
			added: false,
		},
		RuntimeEvent::XYK(pallet_xyk::Event::LiquidityAdded { .. }) => Notable::Liquidity {
			venue: "xyk",
			added: true,
		},
		RuntimeEvent::XYK(pallet_xyk::Event::LiquidityRemoved { .. }) => Notable::Liquidity {
			venue: "xyk",
			added: false,
		},
		_ => return None,
	})
}

fn trade_of(ev: &RuntimeEvent) -> Option<Trade> {
	let (router, asset_in, amount_in, asset_out, amount_out) = match ev {
		RuntimeEvent::Router(pallet_route_executor::Event::Executed {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			..
		}) => (true, *asset_in, *amount_in, *asset_out, *amount_out),
		RuntimeEvent::Omnipool(
			pallet_omnipool::Event::SellExecuted {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				..
			}
			| pallet_omnipool::Event::BuyExecuted {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				..
			},
		) => (false, *asset_in, *amount_in, *asset_out, *amount_out),
		RuntimeEvent::Stableswap(
			pallet_stableswap::Event::SellExecuted {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				..
			}
			| pallet_stableswap::Event::BuyExecuted {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				..
			},
		) => (false, *asset_in, *amount_in, *asset_out, *amount_out),
		_ => return None,
	};
	Some(Trade {
		asset_in,
		amount_in,
		asset_out,
		amount_out,
		router,
	})
}

fn is_lp_event(ev: &RuntimeEvent) -> bool {
	matches!(
		ev,
		RuntimeEvent::Omnipool(
			pallet_omnipool::Event::LiquidityAdded { .. } | pallet_omnipool::Event::LiquidityRemoved { .. }
		) | RuntimeEvent::Stableswap(
			pallet_stableswap::Event::LiquidityAdded { .. } | pallet_stableswap::Event::LiquidityRemoved { .. }
		) | RuntimeEvent::XYK(pallet_xyk::Event::LiquidityAdded { .. } | pallet_xyk::Event::LiquidityRemoved { .. })
	)
}

fn legs_of(ev: &RuntimeEvent) -> Vec<TransferLeg> {
	let leg = |asset: AssetId, from: Option<AccountId>, to: Option<AccountId>, amount: u128| TransferLeg {
		asset,
		from: from.map(|a| a.into()),
		to: to.map(|a| a.into()),
		amount,
	};
	match ev {
		RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
			currency_id,
			from,
			to,
			amount,
		}) => {
			vec![leg(*currency_id, Some(from.clone()), Some(to.clone()), *amount)]
		}
		RuntimeEvent::Tokens(orml_tokens::Event::Deposited {
			currency_id,
			who,
			amount,
		}) => {
			vec![leg(*currency_id, None, Some(who.clone()), *amount)]
		}
		RuntimeEvent::Tokens(orml_tokens::Event::Withdrawn {
			currency_id,
			who,
			amount,
		}) => {
			vec![leg(*currency_id, Some(who.clone()), None, *amount)]
		}
		RuntimeEvent::Balances(pallet_balances::Event::Transfer { from, to, amount }) => {
			vec![leg(NATIVE_ASSET, Some(from.clone()), Some(to.clone()), *amount)]
		}
		RuntimeEvent::Balances(pallet_balances::Event::Deposit { who, amount }) => {
			vec![leg(NATIVE_ASSET, None, Some(who.clone()), *amount)]
		}
		RuntimeEvent::Balances(pallet_balances::Event::Withdraw { who, amount }) => {
			vec![leg(NATIVE_ASSET, Some(who.clone()), None, *amount)]
		}
		// pallet-currencies: the ONLY substrate signal for an Erc20 asset move (HOLLAR, aUSDC), and
		// ALSO emitted alongside tokens.* for orml moves routed through Currencies — core's reducer
		// dedups the (asset,from,to,amount) duplicate, so both sources are safe to emit.
		RuntimeEvent::Currencies(pallet_currencies::Event::Transferred {
			currency_id,
			from,
			to,
			amount,
		}) => {
			vec![leg(*currency_id, Some(from.clone()), Some(to.clone()), *amount)]
		}
		RuntimeEvent::Currencies(pallet_currencies::Event::Deposited {
			currency_id,
			who,
			amount,
		}) => {
			vec![leg(*currency_id, None, Some(who.clone()), *amount)]
		}
		RuntimeEvent::Currencies(pallet_currencies::Event::Withdrawn {
			currency_id,
			who,
			amount,
		}) => {
			vec![leg(*currency_id, Some(who.clone()), None, *amount)]
		}
		_ => Vec::new(),
	}
}

// ---- Aave EVM-log parsing (topic0 = keccak of the event signature) ----

const AAVE_BORROW_SIG: [u8; 32] = hex_literal(b"b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0");
const AAVE_REPAY_SIG: [u8; 32] = hex_literal(b"a534c8dbe71f871f9f3530e97a74601fea17b426cae02e1c5aee42c96c784051");
const AAVE_WITHDRAW_SIG: [u8; 32] = hex_literal(b"3115d1449a7b732c986cba18244e897a450f61e1bb8d589cd2e69e6c8924f9f7");
const AAVE_SUPPLY_SIG: [u8; 32] = hex_literal(b"2b627736bca15cd5381dcf80b0bf11fd197d01a037c52b927a881a10fb73ba61");

fn raw_debt_leg(ev: &RuntimeEvent) -> Option<RawDebtLeg> {
	let RuntimeEvent::EVM(pallet_evm::Event::Log { log }) = ev else {
		return None;
	};
	if log.topics.len() < 3 {
		return None;
	}
	let sig = log.topics.first()?.as_bytes();
	let (who, reserve, amount, collateral) = if sig == AAVE_BORROW_SIG {
		(
			h160_from_word(&log.topics[2]),
			h160_from_word(&log.topics[1]),
			u128_word(&log.data, 1) as i128,
			false,
		)
	} else if sig == AAVE_REPAY_SIG {
		(
			h160_from_word(&log.topics[2]),
			h160_from_word(&log.topics[1]),
			-(u128_word(&log.data, 0) as i128),
			false,
		)
	} else if sig == AAVE_WITHDRAW_SIG {
		(
			h160_from_word(&log.topics[2]),
			h160_from_word(&log.topics[1]),
			u128_word(&log.data, 0) as i128,
			true,
		)
	} else if sig == AAVE_SUPPLY_SIG {
		(
			h160_from_word(&log.topics[2]),
			h160_from_word(&log.topics[1]),
			-(u128_word(&log.data, 1) as i128),
			true,
		)
	} else {
		return None;
	};
	Some(RawDebtLeg {
		who: who.0,
		reserve,
		amount,
		collateral,
	})
}

const fn hex_literal(s: &[u8; 64]) -> [u8; 32] {
	let mut out = [0u8; 32];
	let mut i = 0;
	while i < 32 {
		out[i] = (hex_nibble(s[i * 2]) << 4) | hex_nibble(s[i * 2 + 1]);
		i += 1;
	}
	out
}
const fn hex_nibble(c: u8) -> u8 {
	match c {
		b'0'..=b'9' => c - b'0',
		b'a'..=b'f' => c - b'a' + 10,
		_ => 0,
	}
}
fn h160_from_word(word: &H256) -> H160 {
	H160::from_slice(&word.as_bytes()[12..32])
}
fn u128_word(data: &[u8], word_index: usize) -> u128 {
	let end = (word_index + 1) * 32;
	if data.len() < end {
		return 0;
	}
	let mut buf = [0u8; 16];
	buf.copy_from_slice(&data[end - 16..end]);
	u128::from_be_bytes(buf)
}

/// `primitives::AccountId` (AccountId32) → core's raw 32-byte id.
fn acc(a: &AccountId) -> cerebro_core::AccountId {
	a.clone().into()
}
