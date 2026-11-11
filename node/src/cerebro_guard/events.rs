//! Incremental reader of `System.Events` for the block under construction: per extrinsic it reads
//! only the records appended since the last committed one, so a full block costs O(n), not O(n²).

use cerebro_hydration::decode::{decode_record_stream, EventRecord};
use codec::{Compact, Decode};
use frame_system::Phase;
use sp_storage::well_known_keys::EXTRINSIC_INDEX;

/// `twox128("System") ++ twox128("Events")`.
pub const EVENTS_KEY: [u8; 32] = hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7");

/// A storage read: key, byte offset, maximum length.
pub type RawRead = (Vec<u8>, u32, u32);

pub trait RawStorage {
	fn read(&self, reads: Vec<RawRead>) -> Result<Vec<Option<Vec<u8>>>, String>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum EventsError {
	Read(String),
	Decode(String),
	/// The suffix did not start at a record of the current extrinsic, so the byte bookkeeping is
	/// off. The reader decodes everything from now on.
	Desync,
}

/// Where the reader stands once the extrinsic it read is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
	seen: u32,
	width: u32,
}

#[derive(Debug)]
pub struct Applied {
	pub ext_index: u32,
	pub records: Vec<EventRecord>,
	/// Everything else in the block so far, when the whole value was decoded.
	pub earlier: Option<Vec<EventRecord>>,
	pub mark: Mark,
}

#[derive(Debug)]
pub struct EventsReader {
	/// Bytes of records already accounted for, after the length prefix.
	seen: u32,
	/// Width of the compact length prefix when `seen` was taken.
	width: u32,
	/// `seen` is unknown (first extrinsic, or something was pushed without being read).
	unknown: bool,
	/// Bookkeeping failed once; decode the whole value for the rest of the proposal.
	resynced: bool,
	pub resyncs: u64,
}

impl Default for EventsReader {
	fn default() -> Self {
		Self {
			seen: 0,
			width: 0,
			unknown: true,
			resynced: false,
			resyncs: 0,
		}
	}
}

fn prefix(bytes: &[u8]) -> Result<(u32, u32), EventsError> {
	let mut input = bytes;
	let Compact(count) =
		Compact::<u32>::decode(&mut input).map_err(|e| EventsError::Decode(format!("events prefix: {e}")))?;
	Ok((count, (bytes.len() - input.len()) as u32))
}

/// `frame_system` advances `:extrinsic_index` after applying an extrinsic, so the extrinsic just
/// applied is one below the stored value.
fn applied_index(raw: Option<Vec<u8>>) -> Result<u32, EventsError> {
	let raw = raw.ok_or_else(|| EventsError::Read(":extrinsic_index is not set".into()))?;
	let next = u32::decode(&mut &raw[..]).map_err(|e| EventsError::Decode(format!(":extrinsic_index: {e}")))?;
	next.checked_sub(1)
		.ok_or_else(|| EventsError::Decode(":extrinsic_index is zero after an applied extrinsic".into()))
}

impl EventsReader {
	/// Read the events of the extrinsic that was just applied.
	pub fn read<S: RawStorage>(&mut self, s: &S) -> Result<Applied, EventsError> {
		if self.unknown || self.resynced {
			return self.read_full(s);
		}
		let mut out = s
			.read(vec![
				(EXTRINSIC_INDEX.to_vec(), 0, 4),
				(EVENTS_KEY.to_vec(), 0, 5),
				(EVENTS_KEY.to_vec(), self.width + self.seen, u32::MAX),
			])
			.map_err(EventsError::Read)?;
		if out.len() != 3 {
			return Err(EventsError::Read(format!("{} values for 3 reads", out.len())));
		}
		let ext_index = applied_index(out[0].take())?;
		let (_, width) = prefix(out[1].as_deref().unwrap_or_default())?;
		let mut suffix = out[2].take().unwrap_or_default();
		if width != self.width {
			suffix = s
				.read(vec![(EVENTS_KEY.to_vec(), width + self.seen, u32::MAX)])
				.map_err(EventsError::Read)?
				.pop()
				.flatten()
				.unwrap_or_default();
		}
		let Ok(records) = decode_record_stream(&suffix) else {
			return Err(self.desync());
		};
		if records.is_empty() || records.iter().any(|r| r.phase != Phase::ApplyExtrinsic(ext_index)) {
			return Err(self.desync());
		}
		Ok(Applied {
			ext_index,
			records,
			earlier: None,
			mark: Mark {
				seen: self.seen + suffix.len() as u32,
				width,
			},
		})
	}

	fn read_full<S: RawStorage>(&mut self, s: &S) -> Result<Applied, EventsError> {
		let mut out = s
			.read(vec![
				(EXTRINSIC_INDEX.to_vec(), 0, 4),
				(EVENTS_KEY.to_vec(), 0, u32::MAX),
			])
			.map_err(EventsError::Read)?;
		if out.len() != 2 {
			return Err(EventsError::Read(format!("{} values for 2 reads", out.len())));
		}
		let ext_index = applied_index(out[0].take())?;
		let value = out[1].take().unwrap_or_default();
		let (count, width) = prefix(&value)?;
		let records = decode_record_stream(&value[width as usize..]).map_err(EventsError::Decode)?;
		if records.len() != count as usize {
			return Err(EventsError::Decode(format!(
				"events prefix says {count} records, value holds {}",
				records.len()
			)));
		}
		let (records, earlier) = records
			.into_iter()
			.partition(|r| r.phase == Phase::ApplyExtrinsic(ext_index));
		Ok(Applied {
			ext_index,
			records,
			earlier: Some(earlier),
			mark: Mark {
				seen: value.len() as u32 - width,
				width,
			},
		})
	}

	fn desync(&mut self) -> EventsError {
		self.resynced = true;
		self.resyncs += 1;
		EventsError::Desync
	}

	/// The extrinsic whose events were read with `mark` is in the block.
	pub fn commit(&mut self, mark: Mark) {
		self.seen = mark.seen;
		self.width = mark.width;
		self.unknown = false;
	}

	/// Something was pushed without its events being read.
	pub fn invalidate(&mut self) {
		self.unknown = true;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use codec::Encode;
	use hydradx_runtime::RuntimeEvent;
	use sp_core::H256;
	use std::cell::RefCell;

	#[derive(Default)]
	struct Chain {
		records: RefCell<Vec<EventRecord>>,
		next_index: RefCell<u32>,
		reads: RefCell<u32>,
	}

	impl Chain {
		fn events(&self) -> Vec<u8> {
			self.records.borrow().encode()
		}

		/// Apply an extrinsic that deposits `n` events.
		fn apply(&self, n: usize) -> u32 {
			let index = *self.next_index.borrow();
			for i in 0..n {
				self.records
					.borrow_mut()
					.push(record(Phase::ApplyExtrinsic(index), i as u32));
			}
			*self.next_index.borrow_mut() = index + 1;
			index
		}

		/// Undo the last applied extrinsic (a rolled-back push).
		fn roll_back(&self, n: usize) {
			let mut records = self.records.borrow_mut();
			let len = records.len();
			records.truncate(len - n);
			*self.next_index.borrow_mut() -= 1;
		}
	}

	impl RawStorage for Chain {
		fn read(&self, reads: Vec<RawRead>) -> Result<Vec<Option<Vec<u8>>>, String> {
			*self.reads.borrow_mut() += 1;
			Ok(reads
				.into_iter()
				.map(|(key, offset, max_len)| {
					let value = if key == EXTRINSIC_INDEX {
						self.next_index.borrow().encode()
					} else {
						self.events()
					};
					let start = (offset as usize).min(value.len());
					let end = start.saturating_add(max_len as usize).min(value.len());
					Some(value[start..end].to_vec())
				})
				.collect())
		}
	}

	fn record(phase: Phase, nonce: u32) -> EventRecord {
		EventRecord {
			phase,
			event: RuntimeEvent::System(frame_system::Event::Remarked {
				sender: [nonce as u8; 32].into(),
				hash: H256::repeat_byte(nonce as u8),
			}),
			topics: vec![],
		}
	}

	fn apply_and_read(chain: &Chain, reader: &mut EventsReader, n: usize) -> Applied {
		let index = chain.apply(n);
		let applied = reader.read(chain).expect("read succeeds");
		assert_eq!(applied.ext_index, index);
		reader.commit(applied.mark);
		applied
	}

	fn chain_with_inherents() -> Chain {
		let chain = Chain::default();
		chain.records.borrow_mut().push(record(Phase::Initialization, 0));
		chain.apply(2);
		chain
	}

	#[test]
	fn events_reader_should_return_only_new_records_when_suffix_is_read() {
		let chain = chain_with_inherents();
		let mut reader = EventsReader::default();

		let first = apply_and_read(&chain, &mut reader, 3);
		let second = apply_and_read(&chain, &mut reader, 2);

		assert_eq!(first.records.len(), 3);
		assert_eq!(
			first.earlier,
			Some(vec![
				record(Phase::Initialization, 0),
				record(Phase::ApplyExtrinsic(0), 0),
				record(Phase::ApplyExtrinsic(0), 1)
			])
		);
		assert_eq!(second.earlier, None);
		assert_eq!(
			second.records,
			vec![record(Phase::ApplyExtrinsic(2), 0), record(Phase::ApplyExtrinsic(2), 1)]
		);
		assert_eq!(reader.resyncs, 0);
	}

	#[test]
	fn events_reader_should_not_advance_when_extrinsic_rolls_back() {
		let chain = chain_with_inherents();
		let mut reader = EventsReader::default();
		apply_and_read(&chain, &mut reader, 1);

		chain.apply(4);
		reader.read(&chain).expect("read succeeds");
		chain.roll_back(4);
		let next = apply_and_read(&chain, &mut reader, 2);

		assert_eq!(next.ext_index, 2);
		assert_eq!(next.records.len(), 2);
		assert_eq!(reader.resyncs, 0);
	}

	#[test]
	fn events_reader_should_fall_back_to_full_decode_when_suffix_phase_mismatches() {
		let chain = chain_with_inherents();
		let mut reader = EventsReader::default();
		apply_and_read(&chain, &mut reader, 2);
		let one_record = record(Phase::ApplyExtrinsic(0), 0).encoded_size() as u32;
		reader.seen -= one_record;

		chain.apply(3);
		let desynced = reader.read(&chain);

		assert_eq!(desynced.unwrap_err(), EventsError::Desync);
		assert_eq!(reader.resyncs, 1);
		assert!(reader.resynced);
		let recovered = reader.read(&chain).expect("full decode succeeds");
		assert_eq!(recovered.ext_index, 2);
		assert_eq!(recovered.records.len(), 3);
	}

	#[test]
	fn events_reader_should_reread_prefix_when_compact_length_width_changes() {
		for boundary in [64usize, 16_384] {
			let chain = Chain::default();
			let mut reader = EventsReader::default();
			apply_and_read(&chain, &mut reader, boundary - 2);
			let width_before = reader.width;

			let crossing = apply_and_read(&chain, &mut reader, 3);
			let after = apply_and_read(&chain, &mut reader, 1);

			assert_eq!(crossing.records.len(), 3, "boundary {boundary}");
			assert_eq!(
				after.records,
				vec![record(Phase::ApplyExtrinsic(2), 0)],
				"boundary {boundary}"
			);
			assert_eq!(reader.width, width_before * 2, "boundary {boundary}");
			assert_eq!(reader.resyncs, 0, "boundary {boundary}");
		}
	}

	#[test]
	fn events_reader_should_decode_everything_when_something_was_pushed_unread() {
		let chain = chain_with_inherents();
		let mut reader = EventsReader::default();
		apply_and_read(&chain, &mut reader, 1);

		chain.apply(2);
		reader.invalidate();
		let next = apply_and_read(&chain, &mut reader, 1);

		assert_eq!(next.ext_index, 3);
		assert_eq!(next.records.len(), 1);
		assert_eq!(reader.resyncs, 0);
	}
}
