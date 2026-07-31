// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! metadata-driven `System::Events` decoding.
//!
//! synth logs are translated client-side, so the node has to decode a block's events
//! with types it compiled in. that only works while the node's polkadot-sdk agrees with
//! the on-chain runtime's, and hydration ships node-only releases on purpose — v49.2.0
//! was a stable2603 node against a stable2506 runtime, and `pallet_balances::Event` had
//! moved underneath it.
//!
//! the chain describes its own encoding in its runtime metadata, which the node can read
//! at any block. this module compares that description against the node's own metadata
//! and transcodes each record from the chain's layout into the node's, keyed by pallet and
//! variant *name* rather than index. two properties follow:
//!
//! * a variant this node knows under a different index still decodes — no hand-written
//!   per-sdk layout to maintain;
//! * a record this node cannot represent at all is measured from the chain's metadata and
//!   stepped over exactly, so it can never consume the record that follows it. that is what
//!   turned one moved variant into a whole block of missing logs.
//!
//! what remains outside the guarantee is an event whose pallet or variant this node's
//! `RuntimeEvent` does not contain — a newer runtime's new event. the node has no type to
//! hold it and no synth translation for it either, so it is reported dropped, by name.

use codec::{Compact, Decode, Encode, Input};
use frame_metadata::{v14::StorageEntryType, RuntimeMetadata, RuntimeMetadataPrefixed, META_RESERVED};
use frame_system::EventRecord;
use hydradx_runtime::{evm::event_logs::SYNTH_EVENTS, RuntimeEvent};
use scale_info::{form::PortableForm, Field, MetaType, PortableRegistry, Registry, TypeDef, TypeDefPrimitive};
use sp_core::H256;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

type NodeRecord = EventRecord<RuntimeEvent, H256>;
type Fields = [Field<PortableForm>];

/// enum nesting cap. xcm is the deepest thing an event carries and it is nowhere near
/// this; the cap only exists so a pathological type graph cannot blow the stack.
const MAX_DEPTH: u32 = 128;

/// why a build or a record failed. kept small and cheap to format — it ends up in one
/// log line, next to the name of the event it cost us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
	/// the metadata blob is not runtime metadata.
	NotMetadata,
	/// metadata version this reader does not know. it handles v14 and v15, which is every
	/// version the `Metadata` runtime api is asked for here.
	UnsupportedVersion(u32),
	/// `System::Events` is missing, or is not the `Vec<EventRecord<..>>` shape.
	NoEventsEntry,
	/// a type id in the metadata resolves to nothing.
	DanglingType(u32),
	/// ran out of bytes mid-record.
	Truncated,
	/// the chain's variant index is not in the chain's own metadata — garbage bytes.
	UnknownVariant(u8),
	/// the chain has a variant this node's `RuntimeEvent` does not.
	MissingVariant(String),
	/// same field set, but this node's type cannot hold one of them.
	MissingField(String),
	/// the two type trees disagree on shape (composite vs enum, arity, primitive width).
	Shape,
	/// a type kind that cannot appear in an event, or nesting past `MAX_DEPTH`.
	Unsupported(&'static str),
	/// transcoded cleanly into the node's layout, and the node still would not decode it.
	NodeRejected,
}

/// how the chain's event encoding relates to this node's, for one pair of types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compat {
	/// byte-identical: the bytes can be copied straight through.
	Same,
	/// representable, but the bytes have to be rewritten.
	Rewrite,
	/// this node has no type that can hold it.
	No,
}

impl Compat {
	/// fields of one struct or variant: identical only if every field is, unusable if any is.
	fn and(self, other: Self) -> Self {
		match (self, other) {
			(Compat::No, _) | (_, Compat::No) => Compat::No,
			(Compat::Same, Compat::Same) => Compat::Same,
			_ => Compat::Rewrite,
		}
	}
}

/// whether this node can decode the chain's events with its own compiled types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	/// the chain encodes event records exactly as this node's `RuntimeEvent` does, so a
	/// plain `Decode` is provably correct — not merely observed to succeed.
	Identical,
	/// the layouts differ somewhere; records have to be transcoded.
	Divergent,
}

/// one side of the comparison: a type registry plus where the event record lives in it.
struct Shape {
	registry: PortableRegistry,
	/// `EventRecord<RuntimeEvent, Hash>`
	record: u32,
	/// the outer `RuntimeEvent` enum, used to name an event we had to drop.
	event: u32,
}

impl Shape {
	/// Walk `System::Events` rather than v15's `outer_enums`, so v14 and v15 take the same
	/// path and the `phase`/`topics` type ids come along for free.
	fn from_metadata(prefixed: RuntimeMetadataPrefixed) -> Result<Self, Fault> {
		if prefixed.0 != META_RESERVED {
			return Err(Fault::NotMetadata);
		}
		let (registry, events_ty) = match prefixed.1 {
			RuntimeMetadata::V14(m) => {
				let ty = system_events_type(m.pallets.iter().map(|p| (p.name.as_str(), p.storage.as_ref())))?;
				(m.types, ty)
			}
			RuntimeMetadata::V15(m) => {
				let ty = system_events_type(m.pallets.iter().map(|p| (p.name.as_str(), p.storage.as_ref())))?;
				(m.types, ty)
			}
			other => return Err(Fault::UnsupportedVersion(other.version())),
		};
		Self::from_events_type(registry, events_ty)
	}

	/// `events_ty` is a `Vec<EventRecord<RuntimeEvent, Hash>>`.
	fn from_events_type(registry: PortableRegistry, events_ty: u32) -> Result<Self, Fault> {
		let record = match &registry
			.resolve(events_ty)
			.ok_or(Fault::DanglingType(events_ty))?
			.type_def
		{
			TypeDef::Sequence(seq) => seq.type_param.id,
			_ => return Err(Fault::NoEventsEntry),
		};
		// EventRecord { phase, event, topics }
		let event = match &registry.resolve(record).ok_or(Fault::DanglingType(record))?.type_def {
			TypeDef::Composite(c) => {
				c.fields
					.iter()
					.find(|f| f.name.as_deref() == Some("event"))
					.ok_or(Fault::NoEventsEntry)?
					.ty
					.id
			}
			_ => return Err(Fault::NoEventsEntry),
		};
		Ok(Shape {
			registry,
			record,
			event,
		})
	}

	fn resolve(&self, id: u32) -> Result<&TypeDef<PortableForm>, Fault> {
		self.registry
			.resolve(id)
			.map(|ty| &ty.type_def)
			.ok_or(Fault::DanglingType(id))
	}
}

/// `System.Events`' value type id.
fn system_events_type<'a, I>(pallets: I) -> Result<u32, Fault>
where
	I: Iterator<
		Item = (
			&'a str,
			Option<&'a frame_metadata::v14::PalletStorageMetadata<PortableForm>>,
		),
	>,
{
	for (name, storage) in pallets {
		if name != "System" {
			continue;
		}
		let entries = &storage.ok_or(Fault::NoEventsEntry)?.entries;
		for entry in entries {
			if entry.name == "Events" {
				return match &entry.ty {
					StorageEntryType::Plain(ty) => Ok(ty.id),
					StorageEntryType::Map { .. } => Err(Fault::NoEventsEntry),
				};
			}
		}
	}
	Err(Fault::NoEventsEntry)
}

/// This node's own event types, from the same `TypeInfo` a runtime builds its metadata out
/// of. Built once.
///
/// Not `Runtime::metadata()`: that evaluates every pallet constant, some of which read
/// storage, so it panics outside an externalities environment — which is exactly where a
/// node's rpc threads live.
fn node_shape() -> Result<Arc<Shape>, Fault> {
	static NODE: OnceLock<Result<Arc<Shape>, Fault>> = OnceLock::new();
	NODE.get_or_init(|| {
		let mut registry = Registry::new();
		let events = registry.register_type(&MetaType::new::<Vec<NodeRecord>>()).id;
		Shape::from_events_type(registry.into(), events).map(Arc::new)
	})
	.clone()
}

/// an event the chain can raise and this node has no way to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
	/// `Pallet.Variant`, as runtime metadata names them.
	pub name: String,
	/// synth reads this event, so losing it changes what `eth_getLogs` returns. When false
	/// this is a newer runtime's new event and costs nothing here.
	pub costs_logs: bool,
}

impl Missing {
	fn new(pallet: &str, variant: &str) -> Self {
		let name = format!("{pallet}.{variant}");
		Self {
			costs_logs: feeds_synth(&name),
			name,
		}
	}
}

/// Does an event named `Pallet.Variant` feed synth?
///
/// This is what separates a loss that changes `eth_getLogs` from one that costs nothing, so
/// it errs towards the former: a `Pallet.*` stands for the whole pallet, and a record this
/// reader could not even name is assumed to matter.
pub fn feeds_synth(name: &str) -> bool {
	match name.split_once('.') {
		Some((pallet, "*")) => SYNTH_EVENTS.iter().any(|(p, _)| *p == pallet),
		Some((pallet, variant)) => SYNTH_EVENTS.iter().any(|(p, v)| *p == pallet && *v == variant),
		None => true,
	}
}

/// what came out of one block's `System::Events`.
pub struct Decoded {
	pub records: Vec<NodeRecord>,
	/// events this node cannot represent, named `Pallet.Variant` — each one is a log this
	/// block will not have. empty means nothing was lost.
	pub dropped: Vec<(String, Fault)>,
	/// how many records the blob claims to hold.
	pub expected: usize,
	/// bytes after the last record this reader could measure. non-zero means the record
	/// stream itself broke and everything past it is gone.
	pub trailing: usize,
}

/// The chain's event encoding at some block, paired with this node's, and the machinery to
/// move records from one to the other.
pub struct EventLayout {
	chain: Shape,
	node: Arc<Shape>,
	/// memoised structural comparison, keyed by (chain type, node type). a pair that is
	/// missing is treated as `Rewrite`: transcode it rather than trust a copy.
	compat: HashMap<(u32, u32), Compat>,
	verdict: Verdict,
}

impl EventLayout {
	/// `metadata` is a scale-encoded `RuntimeMetadataPrefixed`, as the `Metadata` runtime
	/// api returns it.
	pub fn new(metadata: &[u8]) -> Result<Self, Fault> {
		let prefixed = RuntimeMetadataPrefixed::decode(&mut &metadata[..]).map_err(|_| Fault::NotMetadata)?;
		let chain = Shape::from_metadata(prefixed)?;
		let node = node_shape()?;

		let mut compat = HashMap::new();
		let mut open = HashSet::new();
		let top = compare(&chain, &node, chain.record, node.record, &mut compat, &mut open);
		Ok(Self {
			chain,
			node,
			compat,
			verdict: if top == Compat::Same {
				Verdict::Identical
			} else {
				Verdict::Divergent
			},
		})
	}

	pub fn verdict(&self) -> Verdict {
		self.verdict
	}

	/// Everything this node cannot represent, known before a single block is read.
	///
	/// An event is unrepresentable either because this node's `RuntimeEvent` has no variant of
	/// that name, or because it has one whose fields cannot be filled from what the chain
	/// encoded — stable2603 added a `poll_index` to `ConvictionVoting::Voted`, and no amount of
	/// transcoding can invent a value the chain never wrote.
	///
	/// The distinction that matters is [`Missing::costs_logs`]: a new event in a newer runtime
	/// is nothing to synth, while one of [`SYNTH_EVENTS`] going missing changes what
	/// `eth_getLogs` returns.
	pub fn unrepresentable(&self) -> Vec<Missing> {
		let mut out = Vec::new();
		let (Ok(TypeDef::Variant(chain)), Ok(TypeDef::Variant(node))) =
			(self.chain.resolve(self.chain.event), self.node.resolve(self.node.event))
		else {
			return out;
		};
		// the comparison memo is read-write; work on a copy so this stays a `&self` query.
		let mut memo = self.compat.clone();
		let mut open = HashSet::new();
		for pallet in &chain.variants {
			let Some(node_pallet) = node.variants.iter().find(|v| v.name == pallet.name) else {
				out.push(Missing::new(&pallet.name, "*"));
				continue;
			};
			// a pallet's event is a single-field newtype around the pallet's own event enum.
			let (Some(c), Some(n)) = (pallet.fields.first(), node_pallet.fields.first()) else {
				continue;
			};
			let (Ok(TypeDef::Variant(inner_chain)), Ok(TypeDef::Variant(inner_node))) =
				(self.chain.resolve(c.ty.id), self.node.resolve(n.ty.id))
			else {
				continue;
			};
			for variant in &inner_chain.variants {
				let usable = inner_node
					.variants
					.iter()
					.find(|v| v.name == variant.name)
					.map(|target| {
						compare_fields(
							&self.chain,
							&self.node,
							&variant.fields,
							&target.fields,
							&mut memo,
							&mut open,
						) != Compat::No
					})
					.unwrap_or(false);
				if !usable {
					out.push(Missing::new(&pallet.name, &variant.name));
				}
			}
		}
		out
	}

	/// Which of [`SYNTH_EVENTS`] this chain does not emit under that name.
	///
	/// Not a layout problem — nothing is being lost in the decode. It means synth is looking
	/// for an event the runtime no longer raises, so the logs derived from it silently stop.
	/// That is the same shape of failure this module exists to end, one level up, so it is
	/// worth saying out loud even though only a runtime change can cause it.
	pub fn synth_events_absent(&self) -> Vec<String> {
		let Ok(TypeDef::Variant(chain)) = self.chain.resolve(self.chain.event) else {
			return Vec::new();
		};
		SYNTH_EVENTS
			.iter()
			.filter(|(pallet, variant)| {
				!chain
					.variants
					.iter()
					.find(|v| v.name == *pallet)
					.and_then(|p| p.fields.first())
					.and_then(|f| self.chain.resolve(f.ty.id).ok())
					.and_then(|def| match def {
						TypeDef::Variant(inner) => Some(inner),
						_ => None,
					})
					.map(|inner| inner.variants.iter().any(|v| v.name == *variant))
					.unwrap_or(false)
			})
			.map(|(pallet, variant)| format!("{pallet}.{variant}"))
			.collect()
	}

	/// Decode a whole `System::Events` blob.
	///
	/// Every record is measured against the *chain's* metadata before this node's types get
	/// a say, so a record that cannot be transcoded costs exactly itself.
	pub fn decode(&self, raw: &[u8]) -> Decoded {
		let mut input = raw;
		let expected = match Compact::<u32>::decode(&mut input) {
			Ok(c) => c.0 as usize,
			Err(_) => {
				return Decoded {
					records: Vec::new(),
					dropped: Vec::new(),
					expected: 0,
					trailing: raw.len(),
				}
			}
		};
		let mut records = Vec::new();
		let mut dropped = Vec::new();

		for _ in 0..expected {
			if input.is_empty() {
				break;
			}
			// measure first: this is the step that keeps a bad record from eating its neighbour.
			let start = input;
			let mut probe = start;
			if self.skip(self.chain.record, &mut probe, 0).is_err() {
				break;
			}
			let span = start.len() - probe.len();
			let bytes = &start[..span];
			input = probe;

			match self.transcode_record(bytes) {
				Ok(record) => records.push(record),
				Err(fault) => dropped.push((self.name_of(bytes), fault)),
			}
		}

		Decoded {
			records,
			dropped,
			expected,
			trailing: input.len(),
		}
	}

	/// One record's bytes, chain layout in, node `EventRecord` out.
	fn transcode_record(&self, bytes: &[u8]) -> Result<NodeRecord, Fault> {
		let mut out = Vec::with_capacity(bytes.len());
		let mut input = bytes;
		self.transcode(self.chain.record, self.node.record, &mut input, &mut out, 0)?;
		if !input.is_empty() {
			return Err(Fault::Shape);
		}
		let mut read = &out[..];
		let record = NodeRecord::decode(&mut read).map_err(|_| Fault::NodeRejected)?;
		// a partial read would mean the transcoder emitted a longer record than the node's
		// types describe, which is a bug here rather than a skew.
		if !read.is_empty() {
			return Err(Fault::NodeRejected);
		}
		Ok(record)
	}

	/// `Pallet.Variant` for a record we could not keep, read from the chain's own metadata so
	/// it works even when this node has no such type.
	fn name_of(&self, bytes: &[u8]) -> String {
		let mut input = bytes;
		let phase = self.chain.resolve(self.chain.record).ok().and_then(|def| match def {
			TypeDef::Composite(c) => c.fields.first().map(|f| f.ty.id),
			_ => None,
		});
		if let Some(phase) = phase {
			if self.skip(phase, &mut input, 0).is_err() {
				return "<unreadable>".into();
			}
		}
		self.variant_path(self.chain.event, &mut input, 2)
			.unwrap_or_else(|| "<unreadable>".into())
	}

	/// Dotted names of the outermost `depth` nested enum variants, e.g. `Balances.Burned`.
	fn variant_path(&self, ty: u32, input: &mut &[u8], depth: u32) -> Option<String> {
		let TypeDef::Variant(v) = self.chain.resolve(ty).ok()? else {
			return None;
		};
		let index = input.read_byte().ok()?;
		let variant = v.variants.iter().find(|c| c.index == index)?;
		if depth <= 1 {
			return Some(variant.name.clone());
		}
		let inner = variant
			.fields
			.first()
			.and_then(|f| self.variant_path(f.ty.id, input, depth - 1));
		Some(match inner {
			Some(inner) => format!("{}.{}", variant.name, inner),
			None => variant.name.clone(),
		})
	}

	fn compat_of(&self, chain: u32, node: u32) -> Compat {
		// unknown pair: the newtype-unwrapping arms below can reach types the eager walk
		// never visited. rewriting an identical type is wasteful, never wrong.
		self.compat.get(&(chain, node)).copied().unwrap_or(Compat::Rewrite)
	}

	/// Rewrite one value from the chain's layout into this node's.
	fn transcode(&self, c: u32, n: u32, input: &mut &[u8], out: &mut Vec<u8>, depth: u32) -> Result<(), Fault> {
		if depth > MAX_DEPTH {
			return Err(Fault::Unsupported("nesting"));
		}
		if self.compat_of(c, n) == Compat::Same {
			return self.copy(c, input, out, depth);
		}
		match (self.chain.resolve(c)?, self.node.resolve(n)?) {
			(TypeDef::Composite(a), TypeDef::Composite(b)) => {
				self.transcode_fields(&a.fields, &b.fields, input, out, depth)
			}
			(TypeDef::Variant(a), TypeDef::Variant(b)) => {
				let index = input.read_byte().map_err(|_| Fault::Truncated)?;
				let chain_variant = a
					.variants
					.iter()
					.find(|v| v.index == index)
					.ok_or(Fault::UnknownVariant(index))?;
				let node_variant = b
					.variants
					.iter()
					.find(|v| v.name == chain_variant.name)
					.ok_or_else(|| Fault::MissingVariant(chain_variant.name.clone()))?;
				out.push(node_variant.index);
				self.transcode_fields(&chain_variant.fields, &node_variant.fields, input, out, depth)
			}
			(TypeDef::Sequence(a), TypeDef::Sequence(b)) => {
				let len = Compact::<u32>::decode(input).map_err(|_| Fault::Truncated)?;
				Compact(len.0).encode_to(out);
				for _ in 0..len.0 {
					self.transcode(a.type_param.id, b.type_param.id, input, out, depth + 1)?;
				}
				Ok(())
			}
			(TypeDef::Array(a), TypeDef::Array(b)) if a.len == b.len => {
				for _ in 0..a.len {
					self.transcode(a.type_param.id, b.type_param.id, input, out, depth + 1)?;
				}
				Ok(())
			}
			(TypeDef::Tuple(a), TypeDef::Tuple(b)) if a.fields.len() == b.fields.len() => {
				for (x, y) in a.fields.iter().zip(b.fields.iter()) {
					self.transcode(x.id, y.id, input, out, depth + 1)?;
				}
				Ok(())
			}
			(TypeDef::Primitive(a), TypeDef::Primitive(b)) if a == b => self.copy(c, input, out, depth),
			// compact and bitvec encodings are self-describing enough to copy: their bytes do
			// not depend on anything an sdk bump moves.
			(TypeDef::Compact(_), TypeDef::Compact(_)) => self.copy(c, input, out, depth),
			(TypeDef::BitSequence(a), TypeDef::BitSequence(b))
				if self.chain.resolve(a.bit_store_type.id)? == self.node.resolve(b.bit_store_type.id)? =>
			{
				self.copy(c, input, out, depth)
			}
			// a newtype gained or lost on one side only: unwrap and keep going. costs nothing
			// when it does not apply, since these arms are only reached on a shape mismatch.
			(TypeDef::Composite(a), _) if a.fields.len() == 1 => {
				self.transcode(a.fields[0].ty.id, n, input, out, depth + 1)
			}
			(TypeDef::Tuple(a), _) if a.fields.len() == 1 => self.transcode(a.fields[0].id, n, input, out, depth + 1),
			(_, TypeDef::Composite(b)) if b.fields.len() == 1 => {
				self.transcode(c, b.fields[0].ty.id, input, out, depth + 1)
			}
			(_, TypeDef::Tuple(b)) if b.fields.len() == 1 => self.transcode(c, b.fields[0].id, input, out, depth + 1),
			_ => Err(Fault::Shape),
		}
	}

	/// Struct or variant fields. Matched by name so a reordered struct still transcodes;
	/// positionally when unnamed, as tuple variants are.
	fn transcode_fields(
		&self,
		chain: &Fields,
		node: &Fields,
		input: &mut &[u8],
		out: &mut Vec<u8>,
		depth: u32,
	) -> Result<(), Fault> {
		if chain.len() != node.len() {
			return Err(Fault::Shape);
		}
		if chain.iter().zip(node.iter()).all(|(a, b)| a.name == b.name) {
			// same order (or both unnamed): stream straight through.
			for (a, b) in chain.iter().zip(node.iter()) {
				self.transcode(a.ty.id, b.ty.id, input, out, depth + 1)?;
			}
			return Ok(());
		}
		// reordered: read in the chain's order, emit in the node's.
		let mut slots: Vec<Option<Vec<u8>>> = vec![None; node.len()];
		for field in chain {
			let name = field.name.as_ref().ok_or(Fault::Shape)?;
			let at = node
				.iter()
				.position(|b| b.name.as_ref() == Some(name))
				.ok_or_else(|| Fault::MissingField(name.clone()))?;
			let mut piece = Vec::new();
			self.transcode(field.ty.id, node[at].ty.id, input, &mut piece, depth + 1)?;
			slots[at] = Some(piece);
		}
		for slot in slots {
			out.extend_from_slice(&slot.ok_or(Fault::Shape)?);
		}
		Ok(())
	}

	/// Copy a value verbatim: measure it against the chain's metadata, then move the bytes.
	fn copy(&self, c: u32, input: &mut &[u8], out: &mut Vec<u8>, depth: u32) -> Result<(), Fault> {
		let start = *input;
		self.skip(c, input, depth)?;
		out.extend_from_slice(&start[..start.len() - input.len()]);
		Ok(())
	}

	/// Advance past one value using only the chain's metadata. This is the measurement that
	/// makes a dropped record cost exactly itself, so it must never consult the node's types.
	fn skip(&self, ty: u32, input: &mut &[u8], depth: u32) -> Result<(), Fault> {
		if depth > MAX_DEPTH {
			return Err(Fault::Unsupported("nesting"));
		}
		match self.chain.resolve(ty)? {
			TypeDef::Composite(c) => {
				for field in &c.fields {
					self.skip(field.ty.id, input, depth + 1)?;
				}
				Ok(())
			}
			TypeDef::Variant(v) => {
				let index = input.read_byte().map_err(|_| Fault::Truncated)?;
				let variant = v
					.variants
					.iter()
					.find(|x| x.index == index)
					.ok_or(Fault::UnknownVariant(index))?;
				for field in &variant.fields {
					self.skip(field.ty.id, input, depth + 1)?;
				}
				Ok(())
			}
			TypeDef::Sequence(s) => {
				let len = Compact::<u32>::decode(input).map_err(|_| Fault::Truncated)?.0;
				for _ in 0..len {
					self.skip(s.type_param.id, input, depth + 1)?;
				}
				Ok(())
			}
			TypeDef::Array(a) => {
				for _ in 0..a.len {
					self.skip(a.type_param.id, input, depth + 1)?;
				}
				Ok(())
			}
			TypeDef::Tuple(t) => {
				for id in &t.fields {
					self.skip(id.id, input, depth + 1)?;
				}
				Ok(())
			}
			TypeDef::Primitive(p) => match p {
				TypeDefPrimitive::Str => {
					let len = Compact::<u32>::decode(input).map_err(|_| Fault::Truncated)?.0 as usize;
					take(input, len)
				}
				other => take(input, primitive_len(other).ok_or(Fault::Unsupported("primitive"))?),
			},
			TypeDef::Compact(_) => skip_compact(input),
			TypeDef::BitSequence(b) => {
				let store = match self.chain.resolve(b.bit_store_type.id)? {
					TypeDef::Primitive(p) => primitive_len(p).ok_or(Fault::Unsupported("bit store"))?,
					_ => return Err(Fault::Unsupported("bit store")),
				};
				let bits = Compact::<u32>::decode(input).map_err(|_| Fault::Truncated)?.0 as usize;
				let words = bits.div_ceil(store * 8);
				take(input, words * store)
			}
		}
	}
}

/// Fixed encoded width of a primitive. `Str` is length-prefixed and handled separately;
/// `char` has no scale encoding at all.
fn primitive_len(p: &TypeDefPrimitive) -> Option<usize> {
	Some(match p {
		TypeDefPrimitive::Bool | TypeDefPrimitive::U8 | TypeDefPrimitive::I8 => 1,
		TypeDefPrimitive::U16 | TypeDefPrimitive::I16 => 2,
		TypeDefPrimitive::U32 | TypeDefPrimitive::I32 => 4,
		TypeDefPrimitive::U64 | TypeDefPrimitive::I64 => 8,
		TypeDefPrimitive::U128 | TypeDefPrimitive::I128 => 16,
		TypeDefPrimitive::U256 | TypeDefPrimitive::I256 => 32,
		TypeDefPrimitive::Str | TypeDefPrimitive::Char => return None,
	})
}

fn take(input: &mut &[u8], len: usize) -> Result<(), Fault> {
	if input.len() < len {
		return Err(Fault::Truncated);
	}
	*input = &input[len..];
	Ok(())
}

/// Compact's own two low bits give its width, whatever integer it wraps.
fn skip_compact(input: &mut &[u8]) -> Result<(), Fault> {
	let first = *input.first().ok_or(Fault::Truncated)?;
	let len = match first & 0b11 {
		0b00 => 1,
		0b01 => 2,
		0b10 => 4,
		_ => 1 + (first >> 2) as usize + 4,
	};
	take(input, len)
}

/// Structural comparison, memoised. Answers "can this node hold the chain's type, and can
/// the bytes be copied or must they be rewritten".
///
/// A type graph can be cyclic (xcm nests itself), so a pair already being compared is
/// assumed `Same`. That is optimistic, and deliberately so: the fast path it unlocks is
/// only ever taken after a plain `Decode` of the whole blob has also succeeded, which is
/// what catches a wrong guess.
fn compare(
	chain: &Shape,
	node: &Shape,
	c: u32,
	n: u32,
	memo: &mut HashMap<(u32, u32), Compat>,
	open: &mut HashSet<(u32, u32)>,
) -> Compat {
	if let Some(known) = memo.get(&(c, n)) {
		return *known;
	}
	if !open.insert((c, n)) {
		return Compat::Same;
	}
	let verdict = compare_uncached(chain, node, c, n, memo, open);
	open.remove(&(c, n));
	memo.insert((c, n), verdict);
	verdict
}

fn compare_uncached(
	chain: &Shape,
	node: &Shape,
	c: u32,
	n: u32,
	memo: &mut HashMap<(u32, u32), Compat>,
	open: &mut HashSet<(u32, u32)>,
) -> Compat {
	let (Ok(a), Ok(b)) = (chain.resolve(c), node.resolve(n)) else {
		return Compat::No;
	};
	match (a, b) {
		(TypeDef::Composite(a), TypeDef::Composite(b)) => compare_fields(chain, node, &a.fields, &b.fields, memo, open),
		(TypeDef::Variant(a), TypeDef::Variant(b)) => {
			// an enum stays usable even when some of its variants are not: the record says
			// which one it is. a variant with no counterpart is charged to the record
			// carrying it, not to the type — which is why `No` here needs *every* variant
			// to be unusable. an empty enum encodes nothing, so nothing is lost to it.
			let mut all_same = true;
			let mut all_no = !a.variants.is_empty();
			for variant in &a.variants {
				let outcome = match b.variants.iter().find(|x| x.name == variant.name) {
					None => Compat::No,
					Some(target) => {
						let fields = compare_fields(chain, node, &variant.fields, &target.fields, memo, open);
						if target.index == variant.index {
							fields
						} else {
							fields.and(Compat::Rewrite)
						}
					}
				};
				all_same &= outcome == Compat::Same;
				all_no &= outcome == Compat::No;
			}
			match (all_same, all_no) {
				(true, _) => Compat::Same,
				(_, true) => Compat::No,
				_ => Compat::Rewrite,
			}
		}
		(TypeDef::Sequence(a), TypeDef::Sequence(b)) => {
			compare(chain, node, a.type_param.id, b.type_param.id, memo, open)
		}
		(TypeDef::Array(a), TypeDef::Array(b)) if a.len == b.len => {
			compare(chain, node, a.type_param.id, b.type_param.id, memo, open)
		}
		(TypeDef::Tuple(a), TypeDef::Tuple(b)) if a.fields.len() == b.fields.len() => {
			a.fields.iter().zip(b.fields.iter()).fold(Compat::Same, |acc, (x, y)| {
				acc.and(compare(chain, node, x.id, y.id, memo, open))
			})
		}
		(TypeDef::Primitive(a), TypeDef::Primitive(b)) if a == b => Compat::Same,
		(TypeDef::Compact(_), TypeDef::Compact(_)) => Compat::Same,
		(TypeDef::BitSequence(a), TypeDef::BitSequence(b)) => {
			match (chain.resolve(a.bit_store_type.id), node.resolve(b.bit_store_type.id)) {
				(Ok(x), Ok(y)) if x == y => Compat::Same,
				_ => Compat::No,
			}
		}
		_ => Compat::No,
	}
}

fn compare_fields(
	chain: &Shape,
	node: &Shape,
	a: &Fields,
	b: &Fields,
	memo: &mut HashMap<(u32, u32), Compat>,
	open: &mut HashSet<(u32, u32)>,
) -> Compat {
	if a.len() != b.len() {
		return Compat::No;
	}
	if a.iter().zip(b.iter()).all(|(x, y)| x.name == y.name) {
		return a.iter().zip(b.iter()).fold(Compat::Same, |acc, (x, y)| {
			acc.and(compare(chain, node, x.ty.id, y.ty.id, memo, open))
		});
	}
	// reordered or renamed: usable only if every chain field has a name this node also has.
	let mut verdict = Compat::Rewrite;
	for field in a {
		let Some(name) = field.name.as_ref() else {
			return Compat::No;
		};
		let Some(target) = b.iter().find(|y| y.name.as_ref() == Some(name)) else {
			return Compat::No;
		};
		verdict = verdict.and(compare(chain, node, field.ty.id, target.ty.id, memo, open));
	}
	verdict
}

/// Mainnet's runtime metadata at spec 433, reduced to what describes an event. Shared with
/// [`super::compat_events`]' tests. See `regenerate_metadata_fixture`.
#[cfg(test)]
pub const TEST_CHAIN_METADATA: &[u8] = include_bytes!("test_data/metadata_433.trimmed.scale");

#[cfg(test)]
mod tests {
	use super::*;
	use frame_metadata::v14::{
		ExtrinsicMetadata, PalletMetadata, PalletStorageMetadata, RuntimeMetadataV14, StorageEntryMetadata,
		StorageEntryModifier,
	};
	use frame_system::Phase;
	use hydradx_runtime::Runtime;
	use pallet_balances::Event as BalancesEvent;
	use primitives::AccountId;
	use scale_info::{Path, Type, TypeDefTuple};

	type Records = Vec<NodeRecord>;

	/// `System::Events` from mainnet block 13386492 (spec 433, stable2506): 84 records over an
	/// `Omnipool.sell` and a `Utility.batch_all([EVM.call, EVM.call])` that burns DAI through
	/// the ntt spoke manager and publishes wormhole message sequence 5.
	const MAINNET_EVENTS: &[u8] = include_bytes!("test_data/events_13386492.scale");

	/// `System::Events` from mainnet block 13384676 (50 records) — the DAI transfer that
	/// published wormhole sequence 2. Also carries `ConvictionVoting.Voted` and
	/// `Scheduler.{Scheduled,Canceled}`, whose layouts moved between stable2506 and
	/// stable2603 as well, so a stable2603 node cannot read it with its own types at all.
	const MAINNET_EVENTS_SEQ2: &[u8] = include_bytes!("test_data/events_13384676.scale");

	/// Mainnet's runtime metadata at those blocks (spec 433), reduced to what describes an
	/// event: the `System.Events` storage entry and the types reachable from it, with docs
	/// stripped. Every byte that decides an encoding — variant indices, field names and
	/// order, type shapes — is exactly as the chain emitted it.
	const CHAIN_METADATA_433: &[u8] = TEST_CHAIN_METADATA;

	/// keccak256("LogMessagePublished(address,uint64,uint32,bytes,uint8)")
	const LOG_MESSAGE_PUBLISHED: [u8; 32] =
		hex_literal::hex!("6eb224fb001ed210e379b335e35efe88672a8ce935d981a6896b27ffdf52a3b2");
	/// hydration wormhole core bridge
	const CORE_BRIDGE: [u8; 20] = hex_literal::hex!("3792a6d63c31941B2805181771795D9176fA82A1");

	/// Nothing lost: every record the blob claims decoded, none dropped, no trailing bytes.
	/// A test-local predicate rather than a method on `Decoded` — production code decides what
	/// to do from the individual fields, so a method here would be dead code, and `-D warnings`
	/// in the Makefile turns dead code into a build failure.
	fn complete(d: &Decoded) -> bool {
		d.dropped.is_empty() && d.trailing == 0 && d.records.len() == d.expected
	}

	fn layout_433() -> EventLayout {
		EventLayout::new(CHAIN_METADATA_433).expect("the checked-in mainnet metadata must build a layout")
	}

	/// The wormhole message sequence out of a block's records, if it published one.
	fn wormhole_sequence(records: &[NodeRecord]) -> Option<u64> {
		records
			.iter()
			.filter_map(|r| match &r.event {
				RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => Some(log),
				_ => None,
			})
			.find(|log| log.address.0 == CORE_BRIDGE && log.topics.first().map(|t| t.0) == Some(LOG_MESSAGE_PUBLISHED))
			.map(|log| u64::from_be_bytes(log.data[24..32].try_into().expect("32-byte word")))
	}

	// ---- the guarantee, on real mainnet data ----

	/// The whole point of this module: mainnet's own metadata lets this node read mainnet's
	/// events, whatever polkadot-sdk it was built against. Nothing synth reads may be lost —
	/// that is the guarantee, and it holds on a node whose sdk matches the runtime and on one
	/// whose does not. A record this node genuinely has no type for is allowed to be dropped
	/// only if synth would not have read it anyway, and it must be named rather than silent.
	#[test]
	fn mainnet_events_decode_without_losing_anything_synth_reads() {
		let layout = layout_433();
		let harmful: Vec<_> = layout.unrepresentable().into_iter().filter(|m| m.costs_logs).collect();
		assert!(
			harmful.is_empty(),
			"this node cannot hold mainnet event(s) that synth reads: {harmful:?}"
		);

		for (label, raw, expected_records, sequence) in [
			("13386492", MAINNET_EVENTS, 84usize, 5u64),
			("13384676", MAINNET_EVENTS_SEQ2, 50, 2),
		] {
			let decoded = layout.decode(raw);
			println!(
				"block {label}: {} of {} records via metadata, verdict {:?}, dropped {:?}",
				decoded.records.len(),
				decoded.expected,
				layout.verdict(),
				decoded.dropped
			);
			assert_eq!(decoded.expected, expected_records, "block {label} record count");
			assert_eq!(decoded.trailing, 0, "block {label} left bytes unread");
			assert_eq!(
				decoded.records.len() + decoded.dropped.len(),
				decoded.expected,
				"block {label}: every record must be either decoded or named as dropped"
			);
			// whatever was dropped, it was named, and synth would not have read it.
			for (name, fault) in &decoded.dropped {
				assert!(
					!feeds_synth(name),
					"block {label} dropped {name} ({fault:?}), which synth reads"
				);
			}
			assert_eq!(
				wormhole_sequence(&decoded.records),
				Some(sequence),
				"block {label} must publish wormhole sequence {sequence}"
			);
		}
	}

	/// What mainnet has and this node does not, spelled out. Informational rather than an
	/// assertion of emptiness: a branch whose runtime predates mainnet's legitimately lacks
	/// its newest events. The assertion that matters is that none of them feed synth, which
	/// `mainnet_events_decode_without_losing_anything_synth_reads` makes.
	#[test]
	fn unrepresentable_mainnet_events_are_named() {
		for missing in layout_433().unrepresentable() {
			println!(
				"{} — {}",
				missing.name,
				if missing.costs_logs {
					"SYNTH READS THIS"
				} else {
					"not read by synth"
				}
			);
		}
	}

	/// `SYNTH_EVENTS` decides which losses are reported as errors, so a typo or a rename in it
	/// would quietly downgrade a real gap. Every entry has to exist in this node's own
	/// `RuntimeEvent`.
	#[test]
	fn every_synth_event_exists() {
		let node = node_shape().expect("node shape");
		let TypeDef::Variant(outer) = node.resolve(node.event).expect("RuntimeEvent") else {
			panic!("RuntimeEvent is an enum")
		};
		for (pallet, variant) in SYNTH_EVENTS {
			let found = outer
				.variants
				.iter()
				.find(|v| v.name == *pallet)
				.and_then(|p| p.fields.first())
				.and_then(|f| node.resolve(f.ty.id).ok())
				.and_then(|def| match def {
					TypeDef::Variant(inner) => Some(inner),
					_ => None,
				})
				.map(|inner| inner.variants.iter().any(|v| v.name == *variant))
				.unwrap_or(false);
			assert!(
				found,
				"SYNTH_EVENTS lists {pallet}.{variant}, which this node's RuntimeEvent does not have — \
				 fix the name in event_logs::SYNTH_EVENTS"
			);
		}
	}

	/// `Identical` is a claim that a plain decode is correct, so it had better be. The reverse
	/// does not hold — a layout can diverge in a variant this block never raised.
	#[test]
	fn identical_verdict_implies_a_plain_decode_works() {
		let layout = layout_433();
		println!("verdict against this node's types: {:?}", layout.verdict());
		if layout.verdict() == Verdict::Identical {
			for raw in [MAINNET_EVENTS, MAINNET_EVENTS_SEQ2] {
				assert!(
					Records::decode(&mut &raw[..]).is_ok(),
					"verdict says the layouts are identical, so the compiled types must decode"
				);
			}
		}
	}

	/// The property that turned one moved variant into a whole block of missing logs: a record
	/// has to be measurable on its own. Measured from the chain's metadata, the records tile
	/// the blob exactly — so dropping one can only ever cost that one.
	#[test]
	fn record_spans_tile_the_blob_exactly() {
		let layout = layout_433();
		for (label, raw) in [("13386492", MAINNET_EVENTS), ("13384676", MAINNET_EVENTS_SEQ2)] {
			let mut input = raw;
			let count = Compact::<u32>::decode(&mut input).expect("count").0 as usize;
			let mut spans = 0usize;
			for i in 0..count {
				layout
					.skip(layout.chain.record, &mut input, 0)
					.unwrap_or_else(|e| panic!("block {label} record {i} is not measurable: {e:?}"));
				spans += 1;
			}
			assert_eq!(spans, count, "block {label}");
			assert!(
				input.is_empty(),
				"block {label} has {} byte(s) the records do not account for",
				input.len()
			);
		}
	}

	// ---- the rewrite paths, without needing a second sdk to build against ----

	/// This node's own event types as v14 metadata, so a test can move a variant or a field the
	/// way an sdk bump does and hand the result to the real entry point. Only `System.Events`
	/// is populated — that is all a `Shape` reads.
	fn node_metadata() -> RuntimeMetadataV14 {
		let mut registry = Registry::new();
		let events = registry.register_type(&MetaType::new::<Vec<NodeRecord>>());
		RuntimeMetadataV14 {
			types: registry.into(),
			pallets: vec![PalletMetadata {
				name: "System".to_string(),
				storage: Some(PalletStorageMetadata {
					prefix: "System".to_string(),
					entries: vec![StorageEntryMetadata {
						name: "Events".to_string(),
						modifier: StorageEntryModifier::Default,
						ty: StorageEntryType::Plain(events),
						default: Vec::new(),
						docs: Vec::new(),
					}],
				}),
				calls: None,
				event: None,
				constants: Vec::new(),
				error: None,
				index: 0,
			}],
			extrinsic: ExtrinsicMetadata {
				ty: events,
				version: 4,
				signed_extensions: Vec::new(),
			},
			ty: events,
		}
	}

	/// The `RuntimeEvent` type id, and the id of one pallet's own event enum inside it.
	fn event_ids(m: &RuntimeMetadataV14, pallet: &str) -> (u32, u32) {
		let events =
			system_events_type(m.pallets.iter().map(|p| (p.name.as_str(), p.storage.as_ref()))).expect("System.Events");
		let TypeDef::Sequence(seq) = &m.types.resolve(events).unwrap().type_def else {
			panic!("Vec<EventRecord>")
		};
		let TypeDef::Composite(record) = &m.types.resolve(seq.type_param.id).unwrap().type_def else {
			panic!("EventRecord")
		};
		let outer = record
			.fields
			.iter()
			.find(|f| f.name.as_deref() == Some("event"))
			.unwrap()
			.ty
			.id;
		let TypeDef::Variant(v) = &m.types.resolve(outer).unwrap().type_def else {
			panic!("RuntimeEvent")
		};
		let inner = v
			.variants
			.iter()
			.find(|x| x.name == pallet)
			.unwrap_or_else(|| panic!("no {pallet} pallet"))
			.fields[0]
			.ty
			.id;
		(outer, inner)
	}

	fn type_mut(m: &mut RuntimeMetadataV14, id: u32) -> &mut Type<PortableForm> {
		&mut m
			.types
			.types
			.iter_mut()
			.find(|t| t.id == id)
			.expect("type in registry")
			.ty
	}

	fn encoded(m: RuntimeMetadataV14) -> Vec<u8> {
		RuntimeMetadataPrefixed::from(m).encode()
	}

	/// One record's bytes as this node encodes them, wrapped as a `System::Events` blob.
	fn blob(records: &[NodeRecord]) -> Vec<u8> {
		records.to_vec().encode()
	}

	fn balances_record(event: BalancesEvent<Runtime>) -> NodeRecord {
		EventRecord {
			phase: Phase::ApplyExtrinsic(3),
			event: RuntimeEvent::Balances(event),
			topics: Vec::new(),
		}
	}

	/// The exact shape of the outage: an sdk inserted variants into
	/// `pallet_balances::Event`, so every later variant encodes under a different index. The
	/// chain's metadata says which index means which name, and that is enough — no
	/// hand-written per-sdk layout, no node rebuild.
	#[test]
	fn a_moved_variant_index_still_decodes() {
		let mut chain = node_metadata();
		let (_, balances) = event_ids(&chain, "Balances");
		let TypeDef::Variant(v) = &mut type_mut(&mut chain, balances).type_def else {
			panic!("Balances event enum")
		};
		// push every variant well clear of where this node expects it.
		let shift = 100u8;
		let mut moved = Vec::new();
		for variant in &mut v.variants {
			moved.push((variant.name.clone(), variant.index, variant.index + shift));
			variant.index += shift;
		}
		let (_, index, shifted) = moved
			.iter()
			.find(|(name, ..)| name == "Transfer")
			.expect("Transfer variant")
			.clone();

		let record = balances_record(BalancesEvent::Transfer {
			from: AccountId::new([1u8; 32]),
			to: AccountId::new([2u8; 32]),
			amount: 7,
		});
		let mut raw = blob(&[record.clone()]);
		// rewrite just the variant byte, exactly as a different sdk would have written it.
		// layout: compact count ‖ phase ‖ pallet ‖ variant ‖ ...
		let at = Compact(1u32).encode().len() + Phase::ApplyExtrinsic(3).encode().len();
		assert_eq!(
			(raw[at], raw[at + 1]),
			(balances_pallet_index(), index),
			"pallet and variant bytes"
		);
		raw[at + 1] = shifted;
		assert!(
			Records::decode(&mut &raw[..]).is_err(),
			"the point of the fixture is that this node's own types cannot read it"
		);

		let layout = EventLayout::new(&encoded(chain)).expect("layout");
		assert_eq!(layout.verdict(), Verdict::Divergent);
		let decoded = layout.decode(&raw);
		assert!(complete(&decoded), "dropped {:?}", decoded.dropped);
		assert_eq!(decoded.records, vec![record]);
	}

	/// Fields moving within a variant is the other half of a layout change, and it is not
	/// something a shifted index can express. Matched by name, emitted in this node's order.
	#[test]
	fn a_reordered_struct_still_decodes() {
		let mut chain = node_metadata();
		let (_, balances) = event_ids(&chain, "Balances");
		let TypeDef::Variant(v) = &mut type_mut(&mut chain, balances).type_def else {
			panic!("Balances event enum")
		};
		let transfer = v.variants.iter_mut().find(|x| x.name == "Transfer").unwrap();
		let index = transfer.index;
		// amount, from, to — instead of from, to, amount.
		transfer.fields.rotate_right(1);

		// hand-encode in the chain's order; there is no rust type for it.
		let (from, to, amount) = (AccountId::new([1u8; 32]), AccountId::new([2u8; 32]), 7u128);
		let mut event = vec![balances_pallet_index(), index];
		amount.encode_to(&mut event);
		from.encode_to(&mut event);
		to.encode_to(&mut event);
		let mut raw = Compact(1u32).encode();
		Phase::ApplyExtrinsic(3).encode_to(&mut raw);
		raw.extend_from_slice(&event);
		Vec::<H256>::new().encode_to(&mut raw);

		let layout = EventLayout::new(&encoded(chain)).expect("layout");
		let decoded = layout.decode(&raw);
		assert!(complete(&decoded), "dropped {:?}", decoded.dropped);
		assert_eq!(
			decoded.records,
			vec![balances_record(BalancesEvent::Transfer { from, to, amount })]
		);
	}

	/// The containment property, stated directly: an event this node has no type for costs
	/// exactly itself. Both neighbours survive, and the loss is named rather than silent.
	#[test]
	fn an_unrepresentable_event_costs_only_itself() {
		let mut chain = node_metadata();
		let (_, balances) = event_ids(&chain, "Balances");
		let TypeDef::Variant(v) = &mut type_mut(&mut chain, balances).type_def else {
			panic!("Balances event enum")
		};
		let transfer = v.variants.iter_mut().find(|x| x.name == "Transfer").unwrap();
		let index = transfer.index;
		// a name this node's RuntimeEvent does not have, as a newer runtime's new event would be.
		transfer.name = "TransferV2".to_string();

		let neighbour = |who: u8| {
			balances_record(BalancesEvent::Deposit {
				who: AccountId::new([who; 32]),
				amount: u128::from(who),
			})
		};
		let mut raw = Compact(3u32).encode();
		raw.extend_from_slice(&blob(&[neighbour(9)])[1..]);
		{
			let mut event = vec![balances_pallet_index(), index];
			AccountId::new([1u8; 32]).encode_to(&mut event);
			AccountId::new([2u8; 32]).encode_to(&mut event);
			7u128.encode_to(&mut event);
			Phase::ApplyExtrinsic(3).encode_to(&mut raw);
			raw.extend_from_slice(&event);
			Vec::<H256>::new().encode_to(&mut raw);
		}
		raw.extend_from_slice(&blob(&[neighbour(8)])[1..]);

		let layout = EventLayout::new(&encoded(chain)).expect("layout");
		let missing = layout.unrepresentable();
		assert_eq!(missing.len(), 1, "{missing:?}");
		assert_eq!(missing[0].name, "Balances.TransferV2");
		let decoded = layout.decode(&raw);
		assert_eq!(decoded.expected, 3);
		assert_eq!(
			decoded.records,
			vec![neighbour(9), neighbour(8)],
			"the records either side of an unrepresentable one must survive intact"
		);
		assert_eq!(
			decoded.trailing, 0,
			"the dropped record must not cost any bytes beyond itself"
		);
		assert_eq!(
			decoded.dropped.len(),
			1,
			"the loss must be reported, not silent: {:?}",
			decoded.dropped
		);
		assert_eq!(decoded.dropped[0].0, "Balances.TransferV2");
	}

	fn balances_pallet_index() -> u8 {
		use frame_support::traits::PalletInfoAccess;
		<hydradx_runtime::Balances as PalletInfoAccess>::index() as u8
	}

	// ---- robustness ----

	/// Garbage must produce nothing, not a panic and not a wrong record.
	#[test]
	fn nonsense_input_is_survivable() {
		let layout = layout_433();
		for raw in [
			&[][..],
			&[0xff][..],
			&[0x04, 0xff, 0xff, 0xff][..],
			&vec![0xaa; 512][..],
		] {
			let decoded = layout.decode(raw);
			assert!(decoded.records.is_empty() || !complete(&decoded));
		}
		// every truncation of a real blob.
		for cut in (1..MAINNET_EVENTS_SEQ2.len()).step_by(37) {
			let decoded = layout.decode(&MAINNET_EVENTS_SEQ2[..cut]);
			assert!(decoded.records.len() <= decoded.expected);
		}
	}

	// ---- fixture generation ----

	/// Rebuild `metadata_433.trimmed.scale` from a full metadata blob:
	///
	/// ```text
	/// curl -s -X POST -H 'content-type: application/json' \
	///   -d '{"jsonrpc":"2.0","id":1,"method":"state_getMetadata","params":["<block hash>"]}' \
	///   https://node-dir.kril.hydration.cloud | jq -r .result | tail -c +3 | xxd -r -p > full.scale
	/// HYDRA_FULL_METADATA=full.scale cargo test -p hydradx regenerate_metadata_fixture -- --ignored
	/// ```
	///
	/// Keeps the `System.Events` entry and every type reachable from it, byte for byte; drops
	/// the other pallets and all documentation, which no decoder reads.
	#[test]
	#[ignore = "regenerates a checked-in fixture; needs HYDRA_FULL_METADATA"]
	fn regenerate_metadata_fixture() {
		let path = std::env::var("HYDRA_FULL_METADATA").expect("HYDRA_FULL_METADATA=<full metadata blob>");
		let full = std::fs::read(&path).expect("readable metadata blob");
		let trimmed = trim(&full);
		let out = concat!(
			env!("CARGO_MANIFEST_DIR"),
			"/src/synthetic_logs/test_data/metadata_433.trimmed.scale"
		);
		std::fs::write(out, &trimmed).expect("writable fixture");
		println!("{} -> {} bytes, wrote {out}", full.len(), trimmed.len());

		// the trimmed fixture has to describe events exactly as the full one does.
		let (a, b) = (EventLayout::new(&full).unwrap(), EventLayout::new(&trimmed).unwrap());
		assert_eq!(a.verdict(), b.verdict());
		for raw in [MAINNET_EVENTS, MAINNET_EVENTS_SEQ2] {
			assert_eq!(a.decode(raw).records, b.decode(raw).records);
		}
	}

	fn trim(full: &[u8]) -> Vec<u8> {
		let mut meta = match RuntimeMetadataPrefixed::decode(&mut &full[..]).expect("metadata").1 {
			RuntimeMetadata::V14(m) => m,
			other => panic!("expected v14, got v{}", other.version()),
		};
		let events = system_events_type(meta.pallets.iter().map(|p| (p.name.as_str(), p.storage.as_ref())))
			.expect("System.Events");

		// everything the events entry can reach.
		let mut keep = HashSet::new();
		let mut queue = vec![events];
		while let Some(id) = queue.pop() {
			if !keep.insert(id) {
				continue;
			}
			let Some(ty) = meta.types.resolve(id) else { continue };
			queue.extend(referenced(&ty.type_def));
		}

		let unit = Type {
			path: Path { segments: Vec::new() },
			type_params: Vec::new(),
			type_def: TypeDef::Tuple(TypeDefTuple { fields: Vec::new() }),
			docs: Vec::new(),
		};
		for entry in meta.types.types.iter_mut() {
			if !keep.contains(&entry.id) {
				entry.ty = unit.clone();
				continue;
			}
			// docs and type params decide nothing about an encoding.
			entry.ty.docs = Vec::new();
			entry.ty.type_params = Vec::new();
			match &mut entry.ty.type_def {
				TypeDef::Composite(c) => strip_fields(&mut c.fields),
				TypeDef::Variant(v) => {
					for variant in v.variants.iter_mut() {
						variant.docs = Vec::new();
						strip_fields(&mut variant.fields);
					}
				}
				_ => {}
			}
		}

		// only System, and only its Events entry.
		meta.pallets.retain(|p| p.name == "System");
		for pallet in meta.pallets.iter_mut() {
			pallet.calls = None;
			pallet.event = None;
			pallet.error = None;
			pallet.constants = Vec::new();
			if let Some(storage) = pallet.storage.as_mut() {
				storage.entries.retain(|e| e.name == "Events");
				for entry in storage.entries.iter_mut() {
					entry.docs = Vec::new();
					entry.default = Vec::new();
				}
			}
		}
		meta.extrinsic.signed_extensions = Vec::new();
		encoded(meta)
	}

	fn strip_fields(fields: &mut Vec<Field<PortableForm>>) {
		for field in fields.iter_mut() {
			field.docs = Vec::new();
			field.type_name = None;
		}
	}

	fn referenced(def: &TypeDef<PortableForm>) -> Vec<u32> {
		match def {
			TypeDef::Composite(c) => c.fields.iter().map(|f| f.ty.id).collect(),
			TypeDef::Variant(v) => v
				.variants
				.iter()
				.flat_map(|x| x.fields.iter().map(|f| f.ty.id))
				.collect(),
			TypeDef::Sequence(s) => vec![s.type_param.id],
			TypeDef::Array(a) => vec![a.type_param.id],
			TypeDef::Tuple(t) => t.fields.iter().map(|f| f.id).collect(),
			TypeDef::Primitive(_) => Vec::new(),
			TypeDef::Compact(c) => vec![c.type_param.id],
			TypeDef::BitSequence(b) => vec![b.bit_store_type.id, b.bit_order_type.id],
		}
	}
}
