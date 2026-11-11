//! JSON-lines output: one shadow record per held (or, in shadow mode, would-be held) extrinsic,
//! one audit record per hold action.

use cerebro_hydration::guard::Judgement;
use serde::Serialize;
use sp_core::H256;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

pub const SHADOW_VERSION: u32 = 1;
pub const AUDIT_VERSION: u32 = 1;

pub struct JsonlWriter {
	out: Mutex<BufWriter<File>>,
}

impl JsonlWriter {
	pub fn open(path: &Path) -> std::io::Result<Self> {
		if let Some(dir) = path.parent() {
			std::fs::create_dir_all(dir)?;
		}
		let file = OpenOptions::new().create(true).append(true).open(path)?;
		Ok(Self {
			out: Mutex::new(BufWriter::new(file)),
		})
	}

	pub fn write<T: Serialize>(&self, record: &T) {
		let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
		let written = serde_json::to_writer(&mut *out, record)
			.map_err(|e| e.to_string())
			.and_then(|()| out.write_all(b"\n").map_err(|e| e.to_string()))
			.and_then(|()| out.flush().map_err(|e| e.to_string()));
		if let Err(e) = written {
			log::warn!(target: super::LOG_TARGET, "jsonl write failed: {e}");
		}
	}
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PoolRecord {
	pub key: String,
	pub vps_bps: i64,
	pub loss_usd: f64,
	pub block_loss_usd: f64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PriceRecord {
	pub asset: u32,
	pub source: &'static str,
}

#[derive(Serialize, Debug, PartialEq, Default)]
pub struct StageMicros {
	pub pass1: u64,
	pub pre: u64,
	pub judge: u64,
	pub repush: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ShadowRecord {
	pub v: u32,
	pub parent: String,
	pub number: u32,
	pub index: u32,
	pub tx: String,
	pub signer: Option<String>,
	pub mode: &'static str,
	pub verdict: &'static str,
	pub reason: &'static str,
	pub candidate: bool,
	pub r1: bool,
	pub net_usd: f64,
	pub block_signer_usd: f64,
	pub pools: Vec<PoolRecord>,
	pub prices: Vec<PriceRecord>,
	pub stage_us: StageMicros,
	pub events_resync: bool,
	pub would_hold: bool,
}

pub fn hex(bytes: &[u8]) -> String {
	format!("0x{}", ::hex::encode(bytes))
}

pub struct RecordCtx<'a> {
	pub parent: H256,
	pub number: u32,
	pub index: u32,
	pub tx: &'a H256,
	pub mode: &'static str,
	pub candidate: bool,
	pub stage_us: StageMicros,
	pub events_resync: bool,
}

pub fn shadow_record(ctx: RecordCtx, j: &Judgement) -> ShadowRecord {
	ShadowRecord {
		v: SHADOW_VERSION,
		parent: hex(ctx.parent.as_bytes()),
		number: ctx.number,
		index: ctx.index,
		tx: hex(ctx.tx.as_bytes()),
		signer: j.signer.map(|s| hex(&s)),
		mode: ctx.mode,
		verdict: j.verdict.kind(),
		reason: j.verdict.reason(),
		candidate: ctx.candidate,
		r1: j.r1,
		net_usd: j.net.to_dollars(),
		block_signer_usd: j.block_signer.to_dollars(),
		pools: j
			.pools
			.iter()
			.map(|p| PoolRecord {
				key: p.pool.key(),
				vps_bps: p.vps_bps,
				loss_usd: p.loss.to_dollars(),
				block_loss_usd: p.block_loss.to_dollars(),
			})
			.collect(),
		prices: j
			.prices
			.iter()
			.map(|(asset, origin)| PriceRecord {
				asset: *asset,
				source: origin.as_str(),
			})
			.collect(),
		stage_us: ctx.stage_us,
		events_resync: ctx.events_resync,
		would_hold: j.verdict.blocks(),
	}
}

#[derive(Serialize, Debug, PartialEq)]
pub struct AuditRecord {
	pub v: u32,
	pub unix_ms: u64,
	pub action: &'static str,
	pub tx: String,
	pub block: u32,
	pub reason: String,
	pub detail: serde_json::Value,
}

pub fn unix_ms() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_millis() as u64)
		.unwrap_or_default()
}

pub fn audit(action: &'static str, tx: &H256, block: u32, reason: String, detail: serde_json::Value) -> AuditRecord {
	AuditRecord {
		v: AUDIT_VERSION,
		unix_ms: unix_ms(),
		action,
		tx: hex(tx.as_bytes()),
		block,
		reason,
		detail,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use cerebro_core::value::Usd6;
	use cerebro_hydration::guard::{PoolOutcome, PoolRef, PriceOrigin, Verdict};

	#[test]
	fn shadow_record_should_serialize_spec_fields_when_judgement_holds() {
		let j = Judgement {
			pools: vec![PoolOutcome {
				pool: PoolRef::Omnipool(5),
				vps_bps: -12,
				loss: Usd6(150_000_000),
				block_loss: Usd6(150_000_000),
			}],
			prices: vec![(5, PriceOrigin::ExecSelf)],
			net: Usd6(75_500_000),
			block_signer: Usd6(75_500_000),
			..Judgement::without_detection(Verdict::Hold, Some([2; 32]))
		};
		let ctx = RecordCtx {
			parent: H256::repeat_byte(1),
			number: 123,
			index: 4,
			tx: &H256::repeat_byte(3),
			mode: "shadow",
			candidate: true,
			stage_us: StageMicros {
				pass1: 10,
				pre: 20,
				judge: 30,
				repush: 40,
			},
			events_resync: false,
		};

		let line = serde_json::to_value(shadow_record(ctx, &j)).unwrap();

		assert_eq!(
			line,
			serde_json::json!({
				"v": 1,
				"parent": format!("0x{}", "01".repeat(32)),
				"number": 123,
				"index": 4,
				"tx": format!("0x{}", "03".repeat(32)),
				"signer": format!("0x{}", "02".repeat(32)),
				"mode": "shadow",
				"verdict": "hold",
				"reason": "finding",
				"candidate": true,
				"r1": false,
				"net_usd": 75.5,
				"block_signer_usd": 75.5,
				"pools": [{"key": "omnipool:5", "vps_bps": -12, "loss_usd": 150.0, "block_loss_usd": 150.0}],
				"prices": [{"asset": 5, "source": "exec_self"}],
				"stage_us": {"pass1": 10, "pre": 20, "judge": 30, "repush": 40},
				"events_resync": false,
				"would_hold": true,
			})
		);
	}
}
