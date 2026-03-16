//! Oracle transaction parsing.
//!
//! Extracts oracle update data from DIA oracle Ethereum transactions.
//! Ported from `node/src/liquidation_worker.rs`.

use crate::traits::{AssetAddress, AssetSymbol, Price};
use codec::Encode;
use ethabi::ethereum_types::U256;
use liquidation_worker_support::Function;
use sp_core::RuntimeDebug;
use std::collections::HashMap;

/// Parsed oracle update data from a DIA oracle transaction.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct OracleUpdateData {
	pub base_asset_name: AssetSymbol,
	pub quote_asset: AssetSymbol,
	pub price: Price,
	pub timestamp: U256,
}

impl OracleUpdateData {
	pub fn new(base_asset_name: AssetSymbol, quote_asset: AssetSymbol, price: Price, timestamp: U256) -> Self {
		Self {
			base_asset_name,
			quote_asset,
			price,
			timestamp,
		}
	}
}

/// Parse a DIA oracle update from raw EVM transaction input bytes.
/// Returns a list of `OracleUpdateData` entries.
pub fn parse_oracle_input(transaction_input: &[u8]) -> Option<Vec<OracleUpdateData>> {
	if transaction_input.len() < 4 {
		return None;
	}

	let mut dia_oracle_data = Vec::new();
	let fn_selector = &transaction_input[0..4];

	if fn_selector == Into::<u32>::into(Function::SetValue).to_be_bytes() {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::String,
				ethabi::ParamType::Uint(16),
				ethabi::ParamType::Uint(16),
			],
			&transaction_input[4..],
		)
		.ok()?;

		dia_oracle_data.push((
			decoded[0].clone().into_string()?,
			decoded[1].clone().into_uint()?,
			decoded[2].clone().into_uint()?,
		));
	} else if fn_selector == Into::<u32>::into(Function::SetMultipleValues).to_be_bytes() {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
			],
			&transaction_input[4..],
		)
		.ok()?;

		if decoded.len() == 2 {
			let keys = decoded[0].clone().into_array()?;
			let values = decoded[1].clone().into_array()?;
			for (asset_str, price_and_timestamp) in keys.iter().zip(values.iter()) {
				let price_and_timestamp = price_and_timestamp.clone().into_uint()?;
				let encoded = price_and_timestamp.encode();
				let price = Price::from_little_endian(&encoded[16..32]);
				let timestamp = U256::from_little_endian(&encoded[0..16]);
				dia_oracle_data.push((asset_str.clone().into_string()?, price, timestamp));
			}
		};
	}

	let mut result = Vec::new();
	for (asset_str, price, timestamp) in dia_oracle_data.iter() {
		let mut assets = asset_str
			.split("/")
			.map(|s| s.as_bytes().to_vec())
			.collect::<Vec<AssetSymbol>>();
		if assets.len() != 2 {
			continue;
		};

		// remove null terminator from the second asset string
		if assets[1].last().cloned() == Some(0) {
			let quote_asset_len = assets[1].len().saturating_sub(1);
			assets[1].truncate(quote_asset_len);
		}

		result.push(OracleUpdateData::new(
			assets[0].clone(),
			assets[1].clone(),
			*price,
			*timestamp,
		));
	}

	Some(result)
}

/// Match oracle update data against known reserves and return (asset_address, maybe_price) pairs.
/// Only returns entries for assets that are in the money market.
pub fn match_oracle_to_reserves(
	oracle_data: &[OracleUpdateData],
	reserves: &HashMap<AssetAddress, AssetSymbol>,
) -> Vec<(AssetAddress, Option<Price>)> {
	oracle_data
		.iter()
		.filter_map(|update| {
			let base_str = String::from_utf8(update.base_asset_name.to_ascii_lowercase()).ok()?;
			let matched: Vec<(AssetAddress, Option<Price>)> = reserves
				.iter()
				.filter(|(_addr, symbol)| {
					String::from_utf8(symbol.to_ascii_lowercase().to_vec())
						.map(|s| s.contains(&base_str))
						.unwrap_or(false)
				})
				.map(|(&addr, symbol)| {
					if *symbol == update.base_asset_name {
						(addr, Some(update.price))
					} else {
						(addr, None)
					}
				})
				.collect();
			if matched.is_empty() {
				None
			} else {
				Some(matched)
			}
		})
		.flatten()
		.collect()
}
