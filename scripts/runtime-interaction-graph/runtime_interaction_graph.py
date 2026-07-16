#!/usr/bin/env python3
"""Build an audit-oriented FRAME/EVM interaction graph.

The extractor is intentionally conservative: syntactic edges carry evidence and
unresolved associated-type calls remain explicit nodes. Semantic call-graph data
can later be merged without changing the output schema.
"""

from __future__ import annotations

import argparse
import html
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict, deque
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import mir_parser
import runtime_inventory
import semantic_inventory
import analysis_provenance
import collect_contracts
import collect_mir
import collect_rapx


PALLET_CALL = re.compile(
	r"((?:pallet_|orml_|cumulus_pallet_|frame_)[a-zA-Z0-9_]+)::Pallet(?:::<[^>]+>)?::([a-zA-Z0-9_]+)",
)
LOCAL_PALLET_CALL = re.compile(
	r"\b(?:Self|Pallet)\s*::\s*(?:<[^>{};\n]+>\s*::\s*)?([a-z_][a-zA-Z0-9_]*)\s*\(",
)
ASSOCIATED_CALL = re.compile(
	r"\b(T|R|Runtime)\s*::\s*([A-Z][A-Za-z0-9_]*)\s*::\s*([a-zA-Z0-9_]+)",
)
RUNTIME_CALL = re.compile(r"\b([A-Z][A-Za-z0-9_]*)::([a-zA-Z0-9_]+)\s*\(")
FN = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z0-9_]+)\s*(?:<[^>{}]*>)?\s*\(")
STORAGE = re.compile(r"\b([A-Z][A-Za-z0-9_]*)(?:::\s*<[^;\n]*?>)?::(get|insert|remove|take|put|mutate|try_mutate|append|clear|clear_prefix)\b")
EXTERNAL = re.compile(r"(::call\b|\.call\s*\(|\.dispatch\s*\(|::dispatch\s*\(|::transfer\s*\(|::deposit\s*\(|::withdraw\s*\(|::mint_into\s*\(|::burn_from\s*\(|handle\.call\s*\(|Runner::call\b)")
STORAGE_WRITES = {"insert", "remove", "take", "put", "mutate", "try_mutate", "append", "clear", "clear_prefix"}
EVM_ENTRY = re.compile(r"(?:\bRunner::call\b|\bEVM::call\b|\bEvm::call\b|\bhandle\.call\s*\()")
INTERNAL_EVM_EXECUTOR = re.compile(r"\bExecutor\s*::\s*<[^>]+>\s*::\s*(call|view)\s*\(")
FRAME_DISPATCH = re.compile(r"(?:\btry_dispatch\s*\(|\.dispatch\s*\(|::dispatch\s*\()")
XCM_SEND = re.compile(r"\b(?:send_xcm|send_xcm_on_behalf|transfer_assets|reserve_transfer_assets)\s*\(")
XCM_ASSET = re.compile(r"::(?:deposit_asset|withdraw_asset|transfer_asset|check_in|check_out)\s*\(")
PRECOMPILE_PUBLIC = re.compile(r'#\[precompile::public\("([^"]+)"\)\]')
GENERATED_SELECTOR_ENUM = re.compile(
	r"(?P<attributes>(?:\s*#\[[^\]]+\])+\s*)(?:pub(?:\([^)]*\))?\s+)?enum\s+"
	r"(?P<name>[A-Z][A-Za-z0-9_]*)\s*\{",
)
SELECTOR_VARIANT = re.compile(r"\b([A-Z][A-Za-z0-9_]*)::([A-Z][A-Za-z0-9_]*)")
SIMPLE_USE = re.compile(
	r"\buse\s+((?:crate|self|super)(?:::[A-Za-z_][A-Za-z0-9_]*)+)"
	r"(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*;",
)
USE_STATEMENT = re.compile(r"(?ms)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?use[ \t]+(.+?);[ \t]*$")
RUNTIME_CONFIG = re.compile(
	r"impl\s+([a-z_][a-zA-Z0-9_]*(?:::[a-zA-Z_][a-zA-Z0-9_]*)*)::Config"
	r"(?:\s*<([^>{}]+)>)?\s+for\s+Runtime\s*\{",
)
CONFIG_TRAIT = re.compile(r"(?:pub\s+)?trait\s+Config(?:\s*<[^>{}]+>)?(?:\s*:[^{]+)?\s*\{")
RUNTIME_ALIAS = re.compile(
	r"(?m)^[ \t]*([A-Z][A-Za-z0-9_]*)[ \t]*:(?!:)[ \t\r\n]*"
	r"([a-z_][a-zA-Z0-9_]*(?:::[a-zA-Z_][a-zA-Z0-9_]*)*)"
	r"(?:[ \t]*::[ \t]*<[^>{}\n]+>)?"
	r"(?:[ \t\r\n]+(?:exclude_parts|use_parts)[ \t]*\{[^{}]*\})?"
	r"[ \t\r\n]*=[ \t]*\d+[ \t]*,?",
)
HELPER_MODULE = re.compile(
	r"(?:#\[cfg\([^\]]*(?:test|runtime-benchmarks)[^\]]*\)\]\s*)?"
	r"(?:pub(?:\([^)]*\))?\s+)?mod\s+(?:tests?|mocks?|benchmarking|benchmarks|benchmark_helpers)\s*\{",
)
CFG_ATTRIBUTE = re.compile(r"#\s*\[\s*cfg\s*\((?P<expression>[^\]]+)\)\s*\]")
ORIGIN_CHECKS = {"ensure_root": "root", "ensure_signed": "signed", "ensure_none": "none",
	"ensure_signed_or_root": "signed-or-root"}
LIFECYCLE_HOOKS = {"on_initialize", "on_finalize", "on_idle", "on_runtime_upgrade", "offchain_worker",
	"on_poll", "integrity_test", "pre_upgrade", "post_upgrade"}
SPECIAL_ENTRYPOINTS = {
	"validate_unsigned": "unsigned-validation", "pre_dispatch": "unsigned-validation",
	"create_inherent": "inherent", "check_inherent": "inherent", "is_inherent": "inherent",
	"offchain_worker": "offchain-worker", "on_runtime_upgrade": "runtime-migration",
	"pre_upgrade": "try-runtime", "post_upgrade": "try-runtime",
}
ASSET_METHODS = {
	"transfer": "transfer", "transfer_keep_alive": "transfer", "deposit": "mint", "deposit_creating": "mint",
	"mint_into": "mint", "issue": "issue", "withdraw": "withdraw", "burn_from": "burn", "burn": "burn",
	"reserve": "reserve", "unreserve": "unreserve", "hold": "hold", "release": "release",
	"repatriate_reserved": "repatriate", "set_lock": "lock", "remove_lock": "unlock",
}
STORAGE_SEMANTICS = {
	("pallet:xyk", "TotalLiquidity"): ("pool-share-supply",),
	("pallet:xyk", "ShareToken"): ("pool-share-identity",),
	("pallet:stableswap", "Pools"): ("pool-configuration",),
	("pallet:stableswap", "PoolSnapshots"): ("pool-reserves",),
	("pallet:omnipool", "Assets"): ("pool-reserves", "pool-share-supply"),
	("pallet:omnipool", "Positions"): ("pool-share-ownership",),
	("pallet:circuit-breaker", "AssetLockdownState"): ("issuance-limit", "lock-or-hold"),
	("precompile:call-permit", "NoncesStorage"): ("nonce-replay",),
}

EDGE_PROJECTIONS = {
	"execution": {
		"direct-call", "runtime-alias-call", "resolved-call", "rapx-call", "mir-call", "mir-component-call",
		"mir-resolved-call", "enters-evm", "mir-enters-evm", "dispatches-frame", "mir-dispatches-frame",
		"mir-external-call", "invokes-precompile", "sends-xcm", "receives-xcm", "moves-xcm-asset",
		"enters-function", "callback-entry", "nested-dispatch", "executes-evm", "submits-ethereum-transaction",
		"delivers-xcm", "invokes",
	},
	"callback": {"dynamic-call", "resolved-call", "callback-entry", "binding-resolves-to", "mir-dynamic-call",
		"mir-resolved-call"},
	"configuration": {"contains", "instantiates", "exposes-entrypoint", "config-binding", "binding-resolves-to", "runtime-config-read",
		"runtime-config-type-reference", "weight-evaluation", "configured-as", "configured-by"},
	"state": {"storage-access", "mir-storage-access", "reads-state", "writes-state", "owns-state",
		"depends-on-state", "enforces-invariant", "guarded-by-invariant", "affects-invariant",
		"owns", "reads", "writes", "enforces", "guards", "must-equal", "tracks", "derives-from",
		"locks", "updates", "backs"},
	"asset": {"asset-operation", "uses-asset-backend", "asset-kind-resolves-to", "defines-asset-kind", "exposed-as",
		"issues-asset-kind", "routes-asset-to", "backed-by", "may-use-native-backend",
		"may-be-protocol-controlled", "routes-to", "backs", "mints", "burns", "transfers-from",
		"transfers-to", "derives-from", "updates"},
	"authorization": {"authorizes-entry", "configured-origin", "origin-resolves-to"},
	"evm-interface": {"dispatches-evm-selector", "encodes-evm-selector", "signature-hashes-to-selector",
		"exposes-function", "selector-matches-contract-function"},
	"deployment": {"uses-deployed-contract", "runtime-configures-contract", "proxy-implementation",
		"bytecode-embeds-address", "exposes-function", "selector-matches-contract-function",
		"deployment-aliases-contract", "deployment-step-produces-alias",
		"deployment-step-references-address"},
}

COMPONENT_NODE_KINDS = {
	"pallet", "frame", "precompile", "evm-adapter", "runtime-adapter", "runtime-pallet-instance",
	"xcm-component", "execution-boundary", "asset-component", "runtime", "deployed-contract", "deployment-alias",
	"deployment-step", "deployment-address-reference",
}

SEMANTIC_COMPONENTS = {
	"component:balances": "pallet:balances",
	"component:orml-tokens": "pallet:orml-tokens",
	"component:erc20-currency": "component:evm:erc20_currency",
	"component:stableswap": "pallet:stableswap",
	"component:xyk": "pallet:xyk",
	"component:omnipool": "pallet:omnipool",
	"component:circuit-breaker": "pallet:circuit-breaker",
}

SEMANTIC_NODE_KINDS = {
	"component": "semantic-component",
	"configuration": "runtime-configuration",
	"guard": "state-guard",
	"invariant": "state-invariant",
	"ledger": "asset-ledger",
	"operation": "asset-operation",
	"pool-state": "storage-model",
	"router": "asset-router",
}


def line_of(text: str, offset: int) -> int:
	return text.count("\n", 0, offset) + 1


def is_storage_match(match: re.Match[str]) -> bool:
	"""Exclude associated-type calls such as Currency::get from storage syntax."""
	return "::<" in match.group(0).replace(" ", "") or match.group(1).endswith("Storage")


def body_end(text: str, start: int) -> int | None:
	opening = text.find("{", start)
	if opening < 0:
		return None
	depth = 0
	for i in range(opening, len(text)):
		if text[i] == "{":
			depth += 1
		elif text[i] == "}":
			depth -= 1
			if depth == 0:
				return i + 1
	return None


def function_body_end(text: str, start: int) -> int | None:
	paren = 1
	bracket = 0
	angle = 0
	i = start
	while i < len(text):
		if text.startswith("//", i):
			i = text.find("\n", i + 2)
			if i < 0:
				return None
			continue
		if text.startswith("/*", i):
			end = text.find("*/", i + 2)
			if end < 0:
				return None
			i = end + 2
			continue
		char = text[i]
		if char == '"':
			i += 1
			while i < len(text):
				if text[i] == "\\":
					i += 2
					continue
				if text[i] == '"':
					i += 1
					break
				i += 1
			continue
		if char == "(":
			paren += 1
		elif char == ")":
			paren = max(0, paren - 1)
		elif char == "[":
			bracket += 1
		elif char == "]":
			bracket = max(0, bracket - 1)
		elif char == "<":
			angle += 1
		elif char == ">" and angle:
			angle -= 1
		elif char == ";" and not (paren or bracket or angle):
			return None
		elif char == "{" and not (paren or bracket or angle):
			return body_end(text, i)
		i += 1
	return None


def source_id(prefix: str, rel: str, name: str, occurrence: int) -> str:
	suffix = f":{occurrence}" if occurrence > 1 else ""
	return f"{prefix}:{rel}:{name}{suffix}"


def component_id(crate: str) -> str:
	name = crate.removeprefix("pallet_")
	if name == "warehouse_liquidity_mining":
		name = "liquidity_mining"
	return f"pallet:{name.replace('_', '-')}"


def configured_type_roots(value: str) -> list[str]:
	value = re.sub(r"//[^\n]*|/\*.*?\*/", "", value, flags=re.S).strip()
	if not value:
		return []
	if value.startswith("("):
		depth = 0
		closing = None
		for offset, char in enumerate(value):
			if char == "(":
				depth += 1
			elif char == ")":
				depth -= 1
				if depth == 0:
					closing = offset
					break
		if closing == len(value) - 1:
			body = value[1:-1]
			parts = []
			start = 0
			paren = bracket = brace = angle = 0
			for offset, char in enumerate(body):
				if char == "(":
					paren += 1
				elif char == ")" and paren:
					paren -= 1
				elif char == "[":
					bracket += 1
				elif char == "]" and bracket:
					bracket -= 1
				elif char == "{":
					brace += 1
				elif char == "}" and brace:
					brace -= 1
				elif char == "<":
					angle += 1
				elif char == ">" and angle:
					angle -= 1
				elif char == "," and not (paren or bracket or brace or angle):
					parts.append(body[start:offset])
					start = offset + 1
			parts.append(body[start:])
			if len(parts) > 1:
				return [root for part in parts for root in configured_type_roots(part)]
			return configured_type_roots(body)
	match = re.match(r"(?:::)?(?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*", value)
	return [match.group(0)] if match else []


def config_callback_targets(value: str, aliases: dict[str, str],
	local_symbols: dict[str, set[str]]) -> list[str]:
	targets = set()
	for type_path in configured_type_roots(value):
		parts = type_path.removeprefix("::").split("::")
		first, symbol = parts[0], parts[-1]
		if first in aliases:
			targets.add(component_id(aliases[first]))
		elif first == "warehouse_liquidity_mining" or first.startswith(
			("pallet_", "orml_", "cumulus_pallet_", "frame_")
		):
			targets.add(component_id(first))
		elif len(local_symbols.get(symbol, ())) == 1:
			targets.add(next(iter(local_symbols[symbol])))
	return sorted(targets)


def scope_ranges(text: str) -> list[tuple[int, int, str]]:
	ranges = []
	pattern = re.compile(
		r"(?m)^[ \t]*(?P<header>(?:(?:pub(?:\([^)]*\))?\s+)?mod\s+[a-zA-Z0-9_]+|"
		r"(?:pub\s+)?trait\s+[A-Za-z0-9_]+[^;{]*|impl(?:<[^{}]*>)?[^;{]*))\s*\{",
	)
	for match in pattern.finditer(text):
		end = body_end(text, match.end() - 1)
		if end is not None:
			ranges.append((match.start(), end, re.sub(r"\s+", " ", match.group("header")).strip()))
	return ranges


def function_source_id(text: str, match: re.Match[str], rel: str, scopes: list[tuple[int, int, str]],
	prefix: str = "function") -> str:
	opening = text.find("{", match.end())
	signature = re.sub(r"\s+", " ", text[match.start():opening if opening >= 0 else match.end()]).strip()
	enclosing = [header for start, end, header in scopes if start < match.start() < end]
	identity = json.dumps({"signature": signature, "scope": enclosing}, sort_keys=True, separators=(",", ":"))
	digest = hashlib.sha256(identity.encode()).hexdigest()[:12]
	return f"{prefix}:{rel}:{match.group(1)}:{digest}"


def impl_context(offset: int, scopes: list[tuple[int, int, str]]) -> str | None:
	impls = [(end - start, header) for start, end, header in scopes
		if start < offset < end and header.startswith("impl")]
	return min(impls)[1] if impls else None


def implemented_trait(header: str | None) -> str | None:
	if not header or " for " not in header:
		return None
	prefix = header.split(" for ", 1)[0]
	prefix = re.sub(r"^impl(?:<[^>]*>)?\s*", "", prefix)
	return prefix.strip()


def balanced_end(text: str, start: int, opening: str = "<", closing: str = ">") -> int | None:
	if start >= len(text) or text[start] != opening:
		return None
	depth = 0
	for offset in range(start, len(text)):
		if text[offset] == opening:
			depth += 1
		elif text[offset] == closing:
			depth -= 1
			if depth == 0:
				return offset + 1
	return None


def mask_rust_comments(text: str) -> str:
	"""Replace Rust comments and literals while preserving byte offsets and line breaks."""
	masked = list(text)

	def clear(start: int, end: int) -> None:
		for offset in range(start, min(end, len(masked))):
			if masked[offset] != "\n":
				masked[offset] = " "

	position = 0
	while position < len(text):
		if text.startswith("//", position):
			end = text.find("\n", position + 2)
			end = len(text) if end < 0 else end
			clear(position, end)
			position = end
			continue
		if text.startswith("/*", position):
			depth = 1
			end = position + 2
			while end < len(text) and depth:
				if text.startswith("/*", end):
					depth += 1
					end += 2
				elif text.startswith("*/", end):
					depth -= 1
					end += 2
				else:
					end += 1
			clear(position, end)
			position = end
			continue
		raw = re.match(r'(?:br|r)(?P<hashes>#{0,255})"', text[position:])
		if raw and (position == 0 or not (text[position - 1].isalnum() or text[position - 1] == "_")):
			terminator = '"' + raw.group("hashes")
			end = text.find(terminator, position + raw.end())
			end = len(text) if end < 0 else end + len(terminator)
			clear(position, end)
			position = end
			continue
		if text[position] == '"':
			end = position + 1
			while end < len(text):
				if text[end] == "\\":
					end += 2
					continue
				end += 1
				if text[end - 1] == '"':
					break
			clear(position, end)
			position = end
			continue
		if text[position] == "'":
			end = None
			if position + 2 < len(text) and text[position + 1] != "\\" \
				and text[position + 2] == "'" and text[position + 1] not in {"\n", "\r", "'"}:
				end = position + 3
			elif text.startswith("'\\u{", position):
				brace = text.find("}", position + 4)
				if brace >= 0 and brace + 1 < len(text) and text[brace + 1] == "'":
					end = brace + 2
			elif text.startswith("'\\x", position) and position + 5 < len(text) \
				and re.fullmatch(r"[0-9A-Fa-f]{2}", text[position + 3:position + 5]) \
				and text[position + 5] == "'":
				end = position + 6
			elif position + 3 < len(text) and text[position + 1] == "\\" and text[position + 3] == "'":
				end = position + 4
			if end is not None:
				clear(position, end)
				position = end
				continue
		position += 1
	return "".join(masked)


def config_type_items(text: str, delimiter: str) -> list[dict[str, object]]:
	"""Parse Config associated declarations or assignments without consuming nested GAT bounds."""
	items = []
	for match in re.finditer(r"\btype\s+([A-Z][A-Za-z0-9_]*)", text):
		position = match.end()
		while position < len(text) and text[position].isspace():
			position += 1
		generics = None
		if position < len(text) and text[position] == "<":
			end = balanced_end(text, position)
			if end is None:
				continue
			generics = text[position:end]
			position = end
		while position < len(text) and text[position].isspace():
			position += 1
		if position >= len(text) or text[position] != delimiter:
			continue
		value_start = position + 1
		paren = bracket = brace = angle = 0
		end = None
		for offset in range(value_start, len(text)):
			char = text[offset]
			if char == "(":
				paren += 1
			elif char == ")" and paren:
				paren -= 1
			elif char == "[":
				bracket += 1
			elif char == "]" and bracket:
				bracket -= 1
			elif char == "{":
				brace += 1
			elif char == "}" and brace:
				brace -= 1
			elif char == "<":
				angle += 1
			elif char == ">" and angle:
				angle -= 1
			elif char == ";" and not (paren or bracket or brace or angle):
				end = offset
				break
		if end is None:
			continue
		items.append({"name": match.group(1), "generics": generics,
			"value": text[value_start:end].strip(), "start": match.start(), "end": end + 1})
	return items


def associated_calls(text: str) -> list[dict[str, object]]:
	"""Return associated calls while preserving an explicit `<T as Trait>` qualifier."""
	calls = [{"start": match.start(), "subject": match.group(1), "trait_path": None,
		"associated_type": match.group(2), "method": match.group(3)}
		for match in ASSOCIATED_CALL.finditer(text)]
	for prefix in re.finditer(r"<\s*([A-Za-z_][A-Za-z0-9_]*)\s+as\s+", text):
		end = balanced_end(text, prefix.start())
		if end is None:
			continue
		tail = re.match(
			r"\s*::\s*([A-Z][A-Za-z0-9_]*)\s*::\s*([a-zA-Z0-9_]+)", text[end:])
		if not tail:
			continue
		calls.append({"start": prefix.start(), "subject": prefix.group(1),
			"trait_path": text[prefix.end():end - 1].strip(),
			"associated_type": tail.group(1), "method": tail.group(2)})
	return sorted(calls, key=lambda item: int(item["start"]))


def config_projection(value: str) -> tuple[str, str, str | None, str] | None:
	if not value.startswith("<"):
		return None
	end = balanced_end(value, 0)
	if end is None:
		return None
	qualified = value[1:end - 1]
	parts = re.match(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s+as\s+(.+?)\s*$", qualified)
	if not parts:
		return None
	subject, trait = parts.groups()
	config = trait.rfind("Config")
	if config < 0:
		return None
	trait_path = trait[:config + len("Config")]
	remainder = trait[config + len("Config"):]
	if remainder and (not remainder.startswith("<") or balanced_end(remainder, 0) != len(remainder)):
		return None
	instance = remainder[1:-1].strip() if remainder else None
	return subject, trait_path, instance, value[end:]


def mir_associated_call(callee: str) -> dict[str, str | None] | None:
	"""Return a Config projection only when it is the MIR call receiver."""
	if callee.startswith("<<"):
		outer_end = balanced_end(callee, 0)
		if outer_end is not None:
			method = re.fullmatch(r"::([A-Za-z_][A-Za-z0-9_]*)", callee[outer_end:])
			projection = config_projection(callee[1:outer_end - 1])
			if method and projection:
				subject, trait_path, instance, remainder = projection
				associated = re.match(r"^::([A-Z][A-Za-z0-9_]*)\s+as\s+", remainder)
				if associated:
					return {"subject": subject, "trait_path": trait_path,
						"config_instance": instance,
						"associated_type": associated.group(1), "method": method.group(1)}
	projection = config_projection(callee)
	if projection:
		subject, trait_path, instance, remainder = projection
		direct = re.fullmatch(
			r"::([A-Z][A-Za-z0-9_]*)::([A-Za-z_][A-Za-z0-9_]*)", remainder)
		if direct:
			return {"subject": subject, "trait_path": trait_path,
				"config_instance": instance,
				"associated_type": direct.group(1), "method": direct.group(2)}
	return None


def config_associated_types(text: str) -> dict[str, str]:
	result = {}
	inactive_ranges = inactive_cfg_ranges(text)
	masked = mask_rust_comments(text)
	for match in CONFIG_TRAIT.finditer(masked):
		end = body_end(masked, match.end() - 1)
		if end is None:
			continue
		body = masked[match.end():end - 1]
		for associated in config_type_items(body, ":"):
			if any(start <= match.end() + int(associated["start"]) < stop for start, stop in inactive_ranges):
				continue
			result[str(associated["name"])] = re.sub(r"\s+", " ", str(associated["value"])).strip()
	return result


def split_top_level(value: str, delimiter: str = ",") -> list[str]:
	parts = []
	start = 0
	paren = bracket = brace = angle = 0
	for offset, char in enumerate(value):
		if char == "(":
			paren += 1
		elif char == ")" and paren:
			paren -= 1
		elif char == "[":
			bracket += 1
		elif char == "]" and bracket:
			bracket -= 1
		elif char == "{":
			brace += 1
		elif char == "}" and brace:
			brace -= 1
		elif char == "<":
			angle += 1
		elif char == ">" and angle:
			angle -= 1
		elif char == delimiter and not (paren or bracket or brace or angle):
			parts.append(value[start:offset].strip())
			start = offset + 1
	parts.append(value[start:].strip())
	return [part for part in parts if part]


def expand_use_tree(tree: str, prefix: str = "") -> list[str]:
	tree = tree.strip()
	opening = tree.find("{")
	if opening < 0:
		return ["::".join(part for part in (prefix, tree) if part)]
	closing = balanced_end(tree, opening, "{", "}")
	if closing is None:
		return []
	head = tree[:opening].strip().removesuffix("::")
	base = "::".join(part for part in (prefix, head) if part)
	result = []
	for item in split_top_level(tree[opening + 1:closing - 1]):
		if item == "self":
			result.append(base)
		elif re.fullmatch(r"self\s+as\s+[A-Za-z_][A-Za-z0-9_]*", item):
			result.append(f"{base} {item.removeprefix('self ')}")
		else:
			result.extend(expand_use_tree(item, base))
	return result


def rust_use_bindings(text: str) -> dict[str, set[str]]:
	bindings: dict[str, set[str]] = defaultdict(set)
	inactive_ranges = inactive_cfg_ranges(text)
	for statement in USE_STATEMENT.finditer(mask_rust_comments(text)):
		if any(start <= statement.start() < end for start, end in inactive_ranges):
			continue
		for imported in expand_use_tree(statement.group(1)):
			alias = re.search(r"\s+as\s+([A-Za-z_][A-Za-z0-9_]*)\s*$", imported)
			path = re.sub(r"\s+as\s+[A-Za-z_][A-Za-z0-9_]*\s*$", "", imported).strip()
			if not path or path.endswith("::*") or (alias and alias.group(1) == "_"):
				continue
			local = alias.group(1) if alias else path.rsplit("::", 1)[-1]
			bindings[local].add(path)
	return dict(bindings)


def canonical_import_path(path: str, bindings: dict[str, set[str]]) -> str:
	path = re.sub(r"\s+", "", path)
	for _ in range(4):
		parts = path.split("::")
		targets = bindings.get(parts[0], set())
		if len(targets) != 1:
			break
		replacement = next(iter(targets))
		candidate = "::".join([replacement, *parts[1:]])
		if candidate == path:
			break
		path = candidate
	return path


def canonical_config_crate(crate: str) -> str:
	return "pallet_liquidity_mining" if crate == "warehouse_liquidity_mining" else crate


def source_config_trait(src: str, relative: str) -> str | None:
	_, module = rust_source_module(relative)
	if relative.startswith("external/"):
		crate = relative.split("/", 2)[1]
	elif relative.startswith("pallets/"):
		crate = f"pallet_{relative.split('/', 2)[1].replace('-', '_')}"
	else:
		return None
	return "::".join([crate, *module, "Config"])


def nearest_config_trait(candidate: str | None, declarations: dict[str, dict[str, str]]) -> str | None:
	if not candidate or candidate in declarations:
		return candidate
	parts = candidate.split("::")[:-1]
	matches = []
	for trait in declarations:
		trait_parts = trait.split("::")[:-1]
		if len(trait_parts) <= len(parts) and parts[:len(trait_parts)] == trait_parts:
			matches.append(trait)
	return max(matches, key=lambda trait: len(trait.split("::")), default=None)


def rust_type_aliases(text: str, bindings: dict[str, set[str]]) -> dict[str, set[str]]:
	aliases: dict[str, set[str]] = defaultdict(set)
	for match in re.finditer(
		r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?type[ \t]+([A-Z][A-Za-z0-9_]*)[ \t]*=[ \t]*([^;\n]+);",
		mask_rust_comments(text),
	):
		aliases[match.group(1)].add(canonical_import_path(match.group(2).strip(), bindings))
	return dict(aliases)


def config_reference(raw: str, bindings: dict[str, set[str]], default_trait: str | None,
	type_aliases: dict[str, set[str]] | None = None) -> tuple[str, str | None] | None:
	raw = re.sub(r"\s+", "", raw)
	config = raw.rfind("Config")
	if config < 0:
		return None
	trait = raw[:config + len("Config")]
	remainder = raw[config + len("Config"):]
	instance = None
	if remainder:
		if not remainder.startswith("<") or balanced_end(remainder, 0) != len(remainder):
			return None
		instance = remainder[1:-1].strip()
	if trait in {"Config", "crate::Config", "crate::pallet::Config", "self::Config", "super::Config"}:
		targets = bindings.get("Config", set())
		trait = next(iter(targets)) if len(targets) == 1 else default_trait
	else:
		trait = canonical_import_path(trait, bindings)
	if not trait:
		return None
	if default_trait and re.fullmatch(r"(?:(?:self|super)::)+(?:pallet::)?Config", trait):
		trait = default_trait
	elif default_trait and trait in {"crate::Config", "crate::pallet::Config"}:
		trait = f"{default_trait.split('::', 1)[0]}::Config"
	if trait.startswith("pallet::") and default_trait:
		trait = default_trait.rsplit("::", 1)[0] + trait.removeprefix("pallet")
	if "::" in trait:
		crate, remainder = trait.split("::", 1)
		trait = f"{canonical_config_crate(crate)}::{remainder}"
	if not trait.endswith("::Config") and trait != "Config":
		return None
	if instance and type_aliases:
		targets = type_aliases.get(instance, set())
		if len(targets) == 1:
			instance = canonical_import_path(next(iter(targets)), bindings)
	if instance and "::" in instance:
		instance = instance.rsplit("::", 1)[-1]
	if instance and not (re.fullmatch(r"I[0-9]*", instance) or re.fullmatch(r"Instance[0-9]+", instance)):
		instance = None
	return trait, instance


def config_references(text: str, bindings: dict[str, set[str]], default_trait: str | None,
	type_aliases: dict[str, set[str]] | None = None) -> set[tuple[str, str | None]]:
	result = set()
	for match in re.finditer(
		r"(?<![A-Za-z0-9_])((?:[a-zA-Z_][a-zA-Z0-9_]*::)*Config(?:\s*<[^>{}]+>)?)", text
	):
		reference = config_reference(match.group(1), bindings, default_trait, type_aliases)
		if reference:
			result.add(reference)
	return result


def config_component(trait: str) -> str:
	return component_id(trait.split("::", 1)[0])


def config_owner(trait: str, current: str) -> str:
	crate = trait.split("::", 1)[0]
	if crate.startswith(("pallet_", "orml_", "frame_", "cumulus_pallet_")):
		return component_id(crate)
	return current


def associated_identity(trait: str, instance: str | None, associated_type: str) -> str:
	parts = trait.split("::")
	component = config_component(trait)
	modules = parts[1:-1]
	qualifiers = [*modules, *([instance] if instance else [])]
	qualified = f":{':'.join(qualifiers)}" if qualifiers else ""
	return f"associated:{component}{qualified}:{associated_type}"


def config_trait_components(text: str, current: str) -> set[str]:
	components = set()
	for crate in re.findall(r"\b([a-z_][a-zA-Z0-9_]*)::Config\b", text):
		if crate in {"crate", "self", "pallet"}:
			components.add(current)
		elif crate.startswith(("pallet_", "orml_", "frame_", "cumulus_pallet_")):
			components.add(component_id(crate))
	return components


def associated_config_owner(candidates: set[str], associated_type: str,
	declarations: dict[str, dict[str, str]], parents: dict[str, set[str]]) -> str | None:
	found = set()
	queue = deque(sorted(candidates))
	visited = set()
	while queue:
		component = queue.popleft()
		if component in visited:
			continue
		visited.add(component)
		if associated_type in declarations.get(component, {}):
			found.add(component)
		queue.extend(sorted(parents.get(component, set()) - visited))
	return next(iter(found)) if len(found) == 1 else None


def associated_config_references(candidates: set[tuple[str, str | None]], associated_type: str,
	declarations: dict[str, dict[str, str]], parents: dict[str, set[str]]) -> set[tuple[str, str | None]]:
	found = set()
	queue = deque(sorted(candidates, key=lambda item: (item[0], item[1] or "")))
	visited = set()
	while queue:
		trait, instance = queue.popleft()
		if (trait, instance) in visited:
			continue
		visited.add((trait, instance))
		if associated_type in declarations.get(trait, {}):
			found.add((trait, instance))
			continue
		queue.extend((parent, None) for parent in sorted(parents.get(trait, set())))
	return found


def active_config_references(reference: tuple[str, str | None],
	active: dict[str, set[str | None]]) -> list[tuple[str, str | None]]:
	trait, instance = reference
	instances = active.get(trait, set())
	if instance in instances or not instances:
		return [reference]
	# Generic Config<I> source code applies to every concrete runtime instance of that Config trait.
	if instance is not None and re.fullmatch(r"I[0-9]*", instance):
		return [(trait, configured) for configured in sorted(instances, key=lambda item: item or "")]
	if instance is not None:
		return [reference]
	if None not in instances:
		return [(trait, configured) for configured in sorted(instances, key=lambda item: item or "")]
	return [reference]


def associated_role(name: str, bounds: str | None) -> str:
	if name.endswith("WeightInfo") or name == "WeightInfo":
		return "weight-provider"
	if bounds and re.search(r"(?:^|[+:\s])(?:Get|GetByKey|TypedGet)\s*<", bounds):
		return "config-value"
	if name in {"RuntimeEvent", "RuntimeCall", "RuntimeOrigin", "AccountId", "AssetId", "Balance", "Amount",
		"BlockNumber", "Hash", "Hasher", "Lookup", "Nonce"}:
		return "config-type"
	return "callback" if bounds else "unknown"


def attribute_targets(text: str, pattern: re.Pattern[str]) -> dict[int, re.Match[str]]:
	return {offset: attributes[-1] for offset, attributes in attribute_target_lists(text, pattern).items()}


def attribute_target_lists(text: str, pattern: re.Pattern[str]) -> dict[int, list[re.Match[str]]]:
	result: dict[int, list[re.Match[str]]] = defaultdict(list)
	for attribute in pattern.finditer(text):
		function = FN.search(text, attribute.end())
		if function:
			result[function.start()].append(attribute)
	return dict(result)


def rust_source_module(relative: str) -> tuple[str, tuple[str, ...]]:
	if "/src/" in relative:
		crate_scope, source = relative.split("/src/", 1)
	elif relative.startswith("external/"):
		parts = relative.split("/")
		crate_scope, source = "/".join(parts[:2]), "/".join(parts[2:])
	else:
		crate_scope, source = relative.rsplit("/", 1) if "/" in relative else (relative, "lib.rs")
	parts = source.split("/")
	filename = parts.pop()
	if filename not in {"lib.rs", "mod.rs"}:
		parts.append(filename.removesuffix(".rs"))
	return crate_scope, tuple(parts)


def generated_selector_enums(text: str) -> dict[str, dict[str, str]]:
	result = {}
	for enum in GENERATED_SELECTOR_ENUM.finditer(text):
		if "generate_function_selector" not in enum.group("attributes"):
			continue
		end = body_end(text, enum.end() - 1)
		if end is None:
			continue
		variants = dict(re.findall(
			r'([A-Z][A-Za-z0-9_]*)\s*=\s*"([^"]+\([^"\n]*\))"',
			text[enum.start():end],
		))
		if variants:
			result[enum.group("name")] = variants
	return result


def resolve_selector_import(relative: str, path: str) -> tuple[str, tuple[str, ...], str] | None:
	crate_scope, current_module = rust_source_module(relative)
	parts = path.split("::")
	if parts[0] == "crate":
		module = []
		parts = parts[1:]
	elif parts[0] == "self":
		module = list(current_module)
		parts = parts[1:]
	elif parts[0] == "super":
		module = list(current_module)
		while parts and parts[0] == "super":
			if not module:
				return None
			module.pop()
			parts = parts[1:]
	else:
		return None
	if not parts:
		return None
	return crate_scope, tuple([*module, *parts[:-1]]), parts[-1]


def selector_type_bindings(text: str, relative: str,
	definitions: dict[tuple[str, tuple[str, ...], str], dict[str, str] | None]) -> dict[str, dict[str, str]]:
	crate_scope, module = rust_source_module(relative)
	bindings = {
		enum_name: variants
		for (scope, enum_module, enum_name), variants in definitions.items()
		if scope == crate_scope and enum_module == module and variants is not None
	}
	for imported in SIMPLE_USE.finditer(text):
		target = resolve_selector_import(relative, imported.group(1))
		variants = definitions.get(target) if target else None
		if variants is not None:
			bindings[imported.group(2) or target[2]] = variants
	return bindings


def macro_argument_ranges(text: str, names: tuple[str, ...]) -> list[tuple[int, int]]:
	pattern = re.compile(rf"\b(?:{'|'.join(map(re.escape, names))})!\s*\(")
	ranges = []
	for match in pattern.finditer(text):
		start = match.end() - 1
		depth = 0
		for offset in range(start, len(text)):
			if text[offset] == "(":
				depth += 1
			elif text[offset] == ")":
				depth -= 1
				if depth == 0:
					ranges.append((start, offset + 1))
					break
	return ranges


def runtime_config_blocks(text: str) -> list[tuple[re.Match[str], str]]:
	blocks = []
	for match in RUNTIME_CONFIG.finditer(text):
		end = body_end(text, match.end() - 1)
		if end is not None:
			blocks.append((match, text[match.end():end - 1]))
	return blocks


def helper_module_ranges(text: str) -> list[tuple[int, int]]:
	ranges = []
	for match in HELPER_MODULE.finditer(text):
		end = body_end(text, match.end() - 1)
		if end is not None:
			ranges.append((match.start(), end))
	return ranges


def inactive_cfg_ranges(text: str) -> list[tuple[int, int]]:
	"""Return item ranges enabled only in tests, benchmarks, or precompile testing builds."""
	ranges = []
	for attribute in CFG_ATTRIBUTE.finditer(text):
		expression = re.sub(r"not\([^)]*\)", "", attribute.group("expression"))
		if not (re.search(r'feature\s*=\s*"(?:runtime-benchmarks|testing)"', expression)
			or re.search(r"\btest\b", expression)):
			continue
		brace = text.find("{", attribute.end())
		semicolon = text.find(";", attribute.end())
		if semicolon >= 0 and (brace < 0 or semicolon < brace):
			end = semicolon + 1
		elif brace >= 0:
			end = body_end(text, brace)
		else:
			end = None
		if end is not None:
			ranges.append((attribute.start(), end))
	return ranges


def entrypoint_eligible(path: Path, offset: int, helper_ranges: list[tuple[int, int]]) -> bool:
	if path.name == "weights.rs" or any(part in {"weights", "tests", "test", "mock", "mocks", "benchmarking",
		"benchmarks"} for part in path.parts):
		return False
	return not any(start <= offset < end for start, end in helper_ranges)


def source_excluded(path: Path) -> bool:
	return path.name == "weights.rs" or "weights" in path.parts or "evm-utility" in path.parts or any(
		part in {"tests", "test", "testing", "mock", "mocks", "benchmarking", "benchmarks"}
		for part in path.parts
	) or path.name.startswith(("test", "mock", "bench"))


def owner(path: Path, root: Path) -> tuple[str, str]:
	rel = path.relative_to(root).as_posix()
	parts = rel.split("/")

	def module_name(prefix: str) -> str:
		module_parts = rel.removeprefix(prefix).strip("/").split("/")
		filename = module_parts[-1]
		if filename in {"lib.rs", "mod.rs"}:
			if len(module_parts) == 1:
				return filename.removesuffix(".rs")
			module_parts = module_parts[:-1]
		else:
			module_parts[-1] = filename.removesuffix(".rs")
		return "/".join(module_parts)

	if parts[0] == "pallets" and len(parts) > 1:
		return f"pallet:{parts[1]}", "frame"
	if parts[0] == "precompiles" and len(parts) > 1:
		return f"precompile:{parts[1]}", "precompile"
	if rel.startswith("runtime/hydradx/src/evm/precompiles/"):
		return f"precompile:runtime:{module_name('runtime/hydradx/src/evm/precompiles/')}", "precompile"
	if rel.startswith("runtime/hydradx/src/evm/"):
		return f"component:evm:{module_name('runtime/hydradx/src/evm/')}", "evm-adapter"
	if rel == "runtime/hydradx/src/lib.rs":
		return "runtime:hydradx", "runtime"
	if rel.startswith("runtime/hydradx/src/"):
		return f"component:runtime:{module_name('runtime/hydradx/src/')}", "runtime-adapter"
	if rel.startswith("runtime/adapters/src/"):
		return f"component:runtime-adapters:{module_name('runtime/adapters/src/')}", "runtime-adapter"
	return f"component:{parts[0]}", "rust"


class Graph:
	def __init__(self) -> None:
		self.nodes: dict[str, dict] = {}
		self.edges: list[dict] = []
		self._edge_keys: set[str] = set()

	def node(self, ident: str, kind: str, **data: object) -> None:
		node = self.nodes.setdefault(ident, {"id": ident, "kind": kind})
		if node["kind"] == "unresolved-reference" and kind != "unresolved-reference":
			node["kind"] = kind
			node.pop("placeholder", None)
		elif node["kind"] != kind:
			roles = set(node.get("roles", [])) | {node["kind"], kind}
			node["roles"] = sorted(roles)
			if kind in {"pallet", "precompile", "function", "entrypoint", "deployed-contract"}:
				node["kind"] = kind
		node.update(data)

	def edge(self, source: str, target: str, kind: str, **data: object) -> None:
		if source not in self.nodes:
			self.node(source, "unresolved-reference", placeholder=True)
		if target not in self.nodes:
			self.node(target, "unresolved-reference", placeholder=True)
		edge = {"source": source, "target": target, "kind": kind, **data}
		key = json.dumps(edge, sort_keys=True, separators=(",", ":"), default=str)
		if key in self._edge_keys:
			return
		self._edge_keys.add(key)
		self.edges.append(edge)

	def reindex_edges(self) -> None:
		unique = []
		self._edge_keys = set()
		for edge in self.edges:
			key = json.dumps(edge, sort_keys=True, separators=(",", ":"), default=str)
			if key in self._edge_keys:
				continue
			self._edge_keys.add(key)
			unique.append(edge)
		self.edges = unique


def add_entrypoint(g: Graph, function: str, entrypoint_kind: str, qualifier: str | None = None,
	**data: object) -> str:
	qualified_kind = f"{entrypoint_kind}:{qualifier}" if qualifier else entrypoint_kind
	ident = f"entrypoint:{qualified_kind}:{function}"
	g.node(ident, "entrypoint", entrypoint_kind=entrypoint_kind, owner=g.nodes[function].get("owner"), **data)
	g.edge(ident, function, "enters-function")
	return ident


def ensure_evm_selector(g: Graph, signature: str, selector: str | None = None) -> tuple[str, str]:
	computed = f"0x{collect_contracts.keccak256(signature.encode())[:4].hex()}"
	if selector is not None and selector.lower() != computed:
		raise ValueError(f"selector does not match canonical EVM signature: {signature}")
	signature_id = f"evm-signature:{signature}"
	selector_id = f"evm-selector:{computed}"
	signatures = sorted(set(g.nodes.get(selector_id, {}).get("signatures", [])) | {signature})
	g.node(signature_id, "evm-signature", domain="evm", signature=signature)
	g.node(selector_id, "evm-selector", domain="evm", selector=computed, signatures=signatures,
		selector_collision=len(signatures) > 1)
	g.edge(signature_id, selector_id, "signature-hashes-to-selector")
	return signature_id, selector_id


def add_precompile_selector_entrypoints(g: Graph, function: str, selectors: list[tuple[str, int]],
	file: str) -> list[str]:
	entrypoints = []
	for signature, line in selectors:
		_, selector_node = ensure_evm_selector(g, signature)
		selector = g.nodes[selector_node]["selector"]
		entrypoint = add_entrypoint(g, function, "precompile-selector", qualifier=selector,
			signature=signature, selector=selector)
		g.edge(entrypoint, selector_node, "dispatches-evm-selector", signature=signature, file=file, line=line)
		entrypoints.append(entrypoint)
	return entrypoints


def add_state_semantics(g: Graph) -> None:
	storage_ids = sorted({edge["target"] for edge in g.edges if edge["kind"] in {"storage-access", "mir-storage-access"}
		and edge["target"].startswith("storage:")})
	for storage_id in storage_ids:
		name = storage_id.rsplit(":", 1)[-1]
		owner_id = g.nodes.get(storage_id, {}).get("owner") or storage_id.removeprefix("storage:").rsplit(":", 1)[0]
		g.node(storage_id, "storage", storage_name=name, owner=owner_id)
		for invariant in STORAGE_SEMANTICS.get((owner_id, name), ()):
			ident = f"invariant:{owner_id}:{invariant}"
			g.node(ident, "state-invariant", domain="state", invariant=invariant, owner=owner_id,
				classification="explicit-storage-inventory")
			g.edge(storage_id, ident, "affects-invariant", evidence="explicit-storage-inventory")


def merge_semantic_inventory(g: Graph, root: Path) -> dict | None:
	path = root / "scripts/runtime-interaction-graph/semantic-inventory.json"
	if not path.is_file():
		return None
	inventory = semantic_inventory.load_inventory(root, path)
	for node in inventory["nodes"]:
		ident = SEMANTIC_COMPONENTS.get(node["id"], node["id"])
		kind = g.nodes.get(ident, {}).get("kind", SEMANTIC_NODE_KINDS[node["kind"]])
		metadata = {
			"semantic_kind": node["kind"],
			"semantic_domains": sorted(set(g.nodes.get(ident, {}).get("semantic_domains", [])) | {node["domain"]}),
			"semantic_label": node["label"],
			"semantic_description": node["description"],
			"semantic_evidence": node["evidence"],
			"semantic_source": node["semantic_source"],
		}
		if node["id"] != ident:
			metadata["semantic_inventory_id"] = node["id"]
		g.node(ident, kind, **metadata)
	for edge in inventory["edges"]:
		g.edge(SEMANTIC_COMPONENTS.get(edge["source"], edge["source"]),
			SEMANTIC_COMPONENTS.get(edge["target"], edge["target"]), edge["kind"],
			semantics=edge["semantics"], enforcement=edge["enforcement"],
			semantic_source=edge["semantic_source"], semantic_evidence=edge["evidence"])
	g.node("semantic-analysis:explicit-inventory", "semantic-coverage", tool="explicit-inventory",
		schema_version=inventory["schema_version"], **inventory["coverage"])
	return inventory


def node_runtime_active(g: Graph, node: dict) -> bool:
	if node.get("runtime_active") is False:
		return False
	owner = node.get("owner")
	if owner and g.nodes.get(owner, {}).get("runtime_active") is False:
		return False
	function = node.get("function")
	return not function or g.nodes.get(function, {}).get("runtime_active") is not False


def edge_runtime_active(g: Graph, edge: dict) -> bool:
	return all(node_runtime_active(g, g.nodes.get(endpoint, {}))
		for endpoint in (edge["source"], edge["target"]))


def projected_edges(g: Graph, projection: str, component_level: bool = False,
	active_only: bool | None = None) -> list[dict]:
	allowed = EDGE_PROJECTIONS[projection]
	edges = component_edges(g) if component_level else g.edges
	if active_only is None:
		active_only = projection in {"execution", "callback"}
	return [edge for edge in edges if edge["kind"] in allowed
		and (not active_only or edge_runtime_active(g, edge))]


def prioritize_gaps(g: Graph, components: list[dict]) -> list[dict]:
	function_entrypoints: dict[str, set[str]] = defaultdict(set)
	for edge in g.edges:
		if edge["kind"] == "enters-function":
			function_entrypoints[edge["target"]].add(edge["source"])
	covered = {edge["target"] for edge in g.edges if edge["kind"] == "test-covers-component"}
	covered.update(entrypoint for edge in g.edges if edge["kind"] == "test-covers-entrypoint"
		for entrypoint in function_entrypoints[edge["target"]])
	privileged_functions = {edge["target"] for edge in g.edges if edge["kind"] == "authorizes-entry"
		and edge["source"] in {"origin:root", "origin:signed-or-root"}}
	privileged = set().union(*(function_entrypoints[target] for target in privileged_functions)) \
		if privileged_functions else set()
	asset_nodes = {edge["source"] for edge in g.edges if edge["kind"] in {"asset-operation", "issues-asset-kind"}}
	boundary_nodes = {edge["source"] for edge in components if edge["target"].startswith("boundary:")}
	result = []
	for node in g.nodes.values():
		if node["kind"] not in {"entrypoint", "pallet", "precompile", "evm-adapter", "xcm-component"}:
			continue
		if node.get("runtime_active") is False:
			continue
		ident = node["id"]
		if ident in covered:
			continue
		score = 1
		reasons = ["no-static-integration-test-link"]
		if ident in privileged:
			score += 5; reasons.append("privileged")
		if ident in asset_nodes or node.get("owner") in asset_nodes:
			score += 4; reasons.append("asset-impact")
		if ident in boundary_nodes or node.get("owner") in boundary_nodes:
			score += 4; reasons.append("execution-boundary")
		if node.get("owner") and g.nodes.get(node["owner"], {}).get("domain") in {"precompile", "evm-adapter", "xcm"}:
			score += 3; reasons.append("cross-domain")
		result.append({"id": ident, "score": score, "reasons": reasons, "kind": node["kind"]})
	return sorted(result, key=lambda item: (-item["score"], item["id"]))


def component_edges(g: Graph) -> list[dict]:
	"""Collapse function-level calls into evidence-bearing component edges."""
	owners = {node["id"]: node.get("owner") for node in g.nodes.values() if node.get("owner")}
	result = []
	seen = set()
	for edge in g.edges:
		if not edge_runtime_active(g, edge):
			continue
		source = owners.get(edge["source"], edge["source"])
		target = owners.get(edge["target"], edge["target"])
		if not source or source == target or edge["kind"] not in {
			"direct-call", "runtime-alias-call", "dynamic-call", "resolved-call", "may-resolve-to",
			"binding-resolves-to", "enters-evm", "dispatches-frame", "asset-kind-resolves-to",
			"defines-asset-kind", "exposed-as", "may-use-native-backend", "may-be-protocol-controlled",
			"invokes-precompile", "uses-asset-backend", "issues-asset-kind",
			"rapx-call", "mir-enters-evm", "mir-dispatches-frame", "mir-external-call",
			"sends-xcm", "receives-xcm", "moves-xcm-asset", "enters-function", "callback-entry",
			"uses-deployed-contract",
			"proxy-implementation", "bytecode-embeds-address", "runtime-configures-contract",
			"deployment-aliases-contract", "deployment-step-produces-alias",
			"deployment-step-references-address",
			"encodes-evm-selector", "dispatches-evm-selector", "signature-hashes-to-selector",
			"selector-matches-contract-function",
			"authorizes-entry",
			"asset-operation", "affects-invariant",
			"mir-component-call", "mir-dynamic-call", "mir-resolved-call",
			"storage-access",
			"runtime-config-read",
			"runtime-config-type-reference", "weight-evaluation", "configured-as", "instantiates",
			"declares-associated-type", "executes-evm", "submits-ethereum-transaction", "delivers-xcm",
			"nested-dispatch", "mir-call", "owns-state", "depends-on-state", "reads-state", "writes-state",
			"enforces-invariant", "guarded-by-invariant", "routes-asset-to", "backed-by", "configured-origin",
			"origin-resolves-to",
			"exposes-entrypoint",
			"contains", "config-binding",
			"owns", "reads", "writes", "enforces", "guards", "must-equal", "tracks", "derives-from",
			"locks", "updates", "backs", "routes-to", "mints", "burns", "transfers-from",
			"transfers-to", "configured-by", "invokes",
		}:
			continue
		collapsed = {**edge, "source": source, "target": target}
		if source != edge["source"]:
			collapsed["evidence_source"] = edge["source"]
		if target != edge["target"]:
			collapsed["evidence_target"] = edge["target"]
		key = json.dumps(collapsed, sort_keys=True, separators=(",", ":"))
		if key in seen:
			continue
		seen.add(key)
		result.append(collapsed)
	return sorted(result, key=lambda edge: (edge["source"], edge["target"], edge["kind"],
		edge.get("line") or 0, json.dumps(edge, sort_keys=True, separators=(",", ":"))))


def enrich_resolutions(g: Graph) -> None:
	bindings: dict[str, list[dict]] = defaultdict(list)
	for edge in g.edges:
		if edge["kind"] == "binding-resolves-to":
			bindings[edge["source"]].append(edge)
	for associated, edges in bindings.items():
		node = g.nodes[associated]
		node["unresolved"] = False
		resolutions = {edge.get("resolution", "runtime-config") for edge in edges}
		node["resolution"] = next(iter(resolutions)) if len(resolutions) == 1 else "mixed-runtime-resolution"
		node["resolved_targets"] = sorted({edge["target"] for edge in edges})
	for edge in list(g.edges):
		if edge["kind"] != "dynamic-call":
			continue
		for binding in bindings.get(edge["target"], []):
			g.edge(edge["source"], binding["target"], "resolved-call", method=edge.get("method"),
				file=edge.get("file"), line=edge.get("line"),
				resolution=binding.get("resolution", "runtime-config"),
				associated_type=edge["target"], binding_file=binding.get("file"))


def enrich_unique_type_resolutions(g: Graph) -> None:
	"""Retained as a compatibility no-op; short associated names are not globally resolvable."""


def enrich_callback_entrypoints(g: Graph) -> None:
	functions: dict[tuple[str, str], list[str]] = defaultdict(list)
	for node in g.nodes.values():
		if node["kind"] == "function" and node.get("entrypoint_eligible", True):
			functions[(node.get("owner"), node["name"])].append(node["id"])
	for edge in list(g.edges):
		if edge["kind"] != "resolved-call" or not edge.get("method"):
			continue
		associated = g.nodes.get(edge.get("associated_type"), {})
		bounds = associated.get("trait_bounds", "")
		trait_names = set(re.findall(r"\b([A-Z][A-Za-z0-9_]*)\s*(?:<|\+|$)", bounds))
		candidates = functions[(edge["target"], edge["method"])]
		qualified = [function for function in candidates if any(
			trait in (g.nodes[function].get("implemented_trait") or "") for trait in trait_names)]
		for function in qualified:
			entrypoint = add_entrypoint(g, function, "runtime-callback", method=edge["method"],
				associated_type=edge.get("associated_type"), resolution="trait-qualified")
			g.edge(edge["source"], entrypoint, "callback-entry", method=edge["method"],
				associated_type=edge.get("associated_type"), resolution="trait-qualified")


def config_is_active(trait: str, instance: str | None,
	active_configs: dict[str, set[str | None]]) -> bool:
	instances = active_configs.get(trait, set())
	if instance is None or re.fullmatch(r"I[0-9]*", instance or ""):
		return bool(instances)
	return instance in instances


def migration_source(path: str | None) -> bool:
	if not path:
		return False
	parts = path.lower().split("/")
	return any(part in {"migration", "migrations"} or part.startswith("migration.")
		or part.startswith("migrations.") for part in parts)


def enrich_migration_calls(g: Graph) -> None:
	functions: dict[tuple[str, str], list[str]] = defaultdict(list)
	for node in g.nodes.values():
		if node.get("kind") == "function" and migration_source(node.get("file")):
			functions[(node["file"], node["name"])].append(node["id"])
	for node in list(g.nodes.values()):
		if node.get("kind") != "function" or not migration_source(node.get("file")):
			continue
		for call in node.get("local_pallet_calls", []):
			candidates = functions.get((node["file"], call["method"]), [])
			if len(candidates) == 1:
				g.edge(node["id"], candidates[0], "direct-call", method=call["method"],
					file=node["file"], line=call["line"], resolution="source-local-migration")


def propagate_runtime_activity(g: Graph) -> None:
	changed = True
	while changed:
		changed = False
		for node in g.nodes.values():
			if node.get("runtime_active") is False:
				continue
			dependencies = []
			if node.get("function"):
				dependencies.append(node["function"])
			if node.get("instance") and isinstance(node.get("instance"), str) \
				and node["instance"].startswith("mir-instance:"):
				dependencies.append(node["instance"])
			if any(g.nodes.get(dependency, {}).get("runtime_active") is False for dependency in dependencies):
				node.update({"runtime_active": False, "runtime_activity": "inventory-only",
					"runtime_activity_reason": "inactive-source-function"})
				changed = True
		for edge in g.edges:
			if edge["kind"] != "enters-function":
				continue
			if g.nodes.get(edge["target"], {}).get("runtime_active") is False \
				and g.nodes.get(edge["source"], {}).get("runtime_active") is not False:
				g.nodes[edge["source"]].update({"runtime_active": False,
					"runtime_activity": "inventory-only", "runtime_activity_reason": "inactive-entrypoint"})
				changed = True


def classify_runtime_activity(g: Graph, runtime_components: set[str],
	active_configs: dict[str, set[str | None]], inventory_complete: bool,
	known_config_traits: set[str] | None = None) -> None:
	for component in runtime_components:
		if component in g.nodes:
			g.nodes[component].update({"runtime_active": True, "runtime_instantiated": True})
	configured_migrations = {edge["target"] for edge in g.edges if edge["kind"] == "enters-function"
		and g.nodes.get(edge["source"], {}).get("entrypoint_kind") in {"runtime-migration", "try-runtime"}}
	active_migration_functions = set(configured_migrations)
	changed = True
	while changed:
		changed = False
		for edge in g.edges:
			if edge["kind"] != "direct-call" or edge["source"] not in active_migration_functions:
				continue
			target = g.nodes.get(edge["target"], {})
			if target.get("kind") != "function" or not migration_source(target.get("file")) \
				or edge["target"] in active_migration_functions:
				continue
			active_migration_functions.add(edge["target"])
			changed = True
	for node in g.nodes.values():
		if node.get("kind") != "function":
			continue
		if migration_source(node.get("file")):
			active = node["id"] in active_migration_functions
			node.update({"runtime_active": active,
				"runtime_activity": "active" if active else "inventory-only",
				"runtime_activity_reason": "configured-migration" if active else "migration-not-configured"})
			continue
		requirements = [(item.get("trait"), item.get("instance"))
			for item in node.get("required_config_traits", []) if item.get("trait")
			and (known_config_traits is None or item.get("trait") in known_config_traits)]
		inactive_requirements = [(trait, instance) for trait, instance in requirements
			if not config_is_active(trait, instance, active_configs)]
		if not inactive_requirements:
			continue
		owner_active = node.get("owner") in runtime_components
		if inventory_complete:
			node.update({"runtime_active": False, "runtime_activity": "inventory-only",
				"runtime_activity_reason": "config-trait-not-implemented" if owner_active
				else "component-not-instantiated", "inactive_config_traits": [
					{"trait": trait, "instance": instance} for trait, instance in inactive_requirements]})
	used_associated = {edge["target"] for edge in g.edges
		if edge["kind"] in {"dynamic-call", "mir-dynamic-call", "configured-origin"}}
	inactive_components = set()
	for node in g.nodes.values():
		if node.get("kind") != "associated-type" or node["id"] not in used_associated:
			continue
		trait = node.get("config_trait")
		instance = node.get("config_instance")
		if trait and not config_is_active(trait, instance, active_configs) and inventory_complete:
			owner_active = node.get("owner") in runtime_components
			node.update({"runtime_active": False, "runtime_activity": "inventory-only",
				"runtime_activity_reason": "config-trait-not-implemented" if owner_active
				else "component-not-instantiated"})
			if not owner_active and node.get("owner"):
				inactive_components.add(node["owner"])
			continue
		incoming = [edge for edge in g.edges if edge["target"] == node["id"]
			and edge["kind"] in {"dynamic-call", "mir-dynamic-call", "configured-origin"}]
		if incoming and all(g.nodes.get(edge["source"], {}).get("runtime_active") is False
			for edge in incoming):
			node.update({"runtime_active": False, "runtime_activity": "inventory-only",
				"runtime_activity_reason": "migration-not-configured"})
	for component in inactive_components:
		if component in g.nodes:
			g.nodes[component].update({"runtime_active": False, "runtime_instantiated": False,
				"runtime_activity": "inventory-only", "runtime_activity_reason": "component-not-instantiated"})
	propagate_runtime_activity(g)


def classify_unresolved(g: Graph) -> None:
	propagate_runtime_activity(g)
	bindings_by_node: dict[str, set[str]] = defaultdict(set)
	used_associated = {edge["target"] for edge in g.edges
		if edge["kind"] in {"dynamic-call", "mir-dynamic-call", "configured-origin"}}
	for edge in g.edges:
		if edge["kind"] == "binding-resolves-to":
			bindings_by_node[edge["source"]].add(edge["target"])
	for node in g.nodes.values():
		if node["kind"] != "associated-type" or not node.get("unresolved"):
			continue
		if node.get("runtime_active") is False:
			node.update({"unresolved": False, "resolution": "inventory-only", "candidate_targets": [],
				"ambiguity_reason": node.get("runtime_activity_reason", "inventory-only")})
			continue
		if node["id"] not in used_associated and not bindings_by_node[node["id"]]:
			node.update({"unresolved": False, "resolution": "declaration-only", "candidate_targets": []})
			continue
		targets = bindings_by_node[node["id"]]
		node["ambiguity_reason"] = "multiple-runtime-targets" if len(targets) > 1 else "no-runtime-binding"
		node["candidate_targets"] = sorted(targets)


def normalize_config_value_calls(g: Graph) -> None:
	incoming: dict[str, list[dict]] = defaultdict(list)
	for edge in g.edges:
		if edge["kind"] in {"dynamic-call", "mir-dynamic-call"}:
			incoming[edge["target"]].append(edge)
	for target, edges in incoming.items():
		node = g.nodes.get(target, {})
		role = node.get("associated_role")
		if role not in {"config-value", "config-type", "weight-provider"}:
			continue
		kind = {"config-value": "runtime-config-value", "config-type": "runtime-config-type",
			"weight-provider": "runtime-weight-provider"}[role]
		node.update({"kind": kind, "unresolved": False, "resolution": "trait-bound-classification"})
		for edge in edges:
			edge["kind"] = {"config-value": "runtime-config-read", "config-type": "runtime-config-type-reference",
				"weight-provider": "weight-evaluation"}[role]
	# Declaration-only types are classified only when their Config trait provides enough evidence.
	for node in g.nodes.values():
		role = node.get("associated_role")
		if node.get("kind") != "associated-type" or node["id"] in incoming \
			or role not in {"config-value", "config-type", "weight-provider"}:
			continue
		node.update({"kind": {"config-value": "runtime-config-value", "config-type": "runtime-config-type",
			"weight-provider": "runtime-weight-provider"}[role], "unresolved": False,
			"resolution": "trait-bound-classification"})
	g.reindex_edges()


def enrich_configured_origins(g: Graph) -> None:
	bindings: dict[str, set[str]] = defaultdict(set)
	for edge in g.edges:
		if edge["kind"] == "binding-resolves-to":
			bindings[edge["source"]].add(edge["target"])
	for edge in list(g.edges):
		if edge["kind"] != "configured-origin":
			continue
		for target in bindings[edge["target"]]:
			g.edge(edge["source"], target, "origin-resolves-to", associated_type=edge["target"],
				resolution="runtime-config")


def add_configured_migrations(g: Graph, root: Path) -> None:
	path = root / "runtime/hydradx/src/migrations/mod.rs"
	if not path.is_file():
		return
	text = path.read_text(errors="replace")
	symbols = set(re.findall(r"\b([A-Z][A-Za-z0-9_]*(?:Migration|Migrations|Version)[A-Za-z0-9_]*)\b", text))
	for function in list(g.nodes.values()):
		if function.get("kind") != "function" or function.get("name") not in {
			"on_runtime_upgrade", "pre_upgrade", "post_upgrade"}:
			continue
		header = function.get("impl_header") or ""
		if not any(symbol in header for symbol in symbols):
			continue
		kind = "runtime-migration" if function["name"] == "on_runtime_upgrade" else "try-runtime"
		add_entrypoint(g, function["id"], kind, method=function["name"], configured_by=path.relative_to(root).as_posix())


def enrich_runtime_instances(g: Graph, entries: list[dict]) -> None:
	instances: dict[str, list[tuple[str, dict]]] = defaultdict(list)
	for entry in entries:
		instances[component_id(entry["crate"])].append((f"runtime-instance:{entry['alias']}", entry))
	for entrypoint in [node for node in g.nodes.values() if node.get("kind") == "entrypoint"]:
		for instance, entry in instances[entrypoint.get("owner")]:
			if entrypoint.get("entrypoint_kind") == "extrinsic" and "Call" in entry["excluded_parts"]:
				continue
			g.edge(instance, entrypoint["id"], "exposes-entrypoint", runtime_alias=entry["alias"],
				instance=entry["instance"], pallet_index=entry["index"])


def merge_integration_tests(g: Graph, root: Path, aliases: dict[str, str]) -> None:
	functions: dict[tuple[str, str], list[str]] = defaultdict(list)
	entrypoint_functions = {edge["target"] for edge in g.edges if edge["kind"] == "enters-function"}
	for node in g.nodes.values():
		if node["kind"] == "function" and node["id"] in entrypoint_functions:
			functions[(node.get("owner"), node["name"])].append(node["id"])
	for path in sorted((root / "integration-tests/src").rglob("*.rs")):
		text = path.read_text(errors="replace")
		rel = path.relative_to(root).as_posix()
		scopes = scope_ranges(text)
		test_targets = attribute_targets(text, re.compile(r"#\[(?:tokio::)?test\]"))
		definitions = {}
		for match in FN.finditer(text):
			end = function_body_end(text, match.end())
			if not end:
				continue
			definitions.setdefault(match.group(1), (match, text[match.start():end]))
		for match, body in definitions.values():
			if match.start() not in test_targets:
				continue
			ident = function_source_id(text, match, rel, scopes, "integration-test")
			assertions = len(re.findall(r"\b(?:assert|assert_eq|assert_ne|assert_ok|assert_noop|assert_err|assert_storage_noop)!", body))
			dispatch_assertions = len(re.findall(r"\b(?:assert_ok|assert_noop|assert_err)!\s*\(", body))
			confidence = "mixed-call-site" if dispatch_assertions else ("assertion-present" if assertions else "reference")
			g.node(ident, "integration-test", file=rel, line=line_of(text, match.start()), name=match.group(1),
				assertion_count=assertions, dispatch_assertion_count=dispatch_assertions, confidence=confidence)
			segments = [(body, "test-body")]
			seen_helpers = set()
			queue = deque(re.findall(r"\b([a-z_][a-zA-Z0-9_]*)\s*\(", body))
			while queue and len(seen_helpers) < 50:
				helper = queue.popleft()
				if helper in seen_helpers or helper not in definitions or helper == match.group(1):
					continue
				seen_helpers.add(helper)
				helper_body = definitions[helper][1]
				segments.append((helper_body, "local-helper"))
				queue.extend(re.findall(r"\b([a-z_][a-zA-Z0-9_]*)\s*\(", helper_body))
			combined = "\n".join(segment for segment, _ in segments)
			components = set()
			for alias, crate in aliases.items():
				if re.search(rf"\b{alias}\b", combined):
					components.add(component_id(crate))
			for crate in re.findall(r"\b((?:pallet|orml|cumulus_pallet)_[a-zA-Z0-9_]+)\b", combined):
				components.add(component_id(crate))
			stem = path.stem.replace("_", "-")
			if f"pallet:{stem}" in g.nodes:
				components.add(f"pallet:{stem}")
			if re.search(r"\b(?:EVM|Evm|eth_|H160|Precompile)\b", combined):
				components.add("boundary:evm-execution")
			if re.search(r"\b(?:Xcm|XCM|xcm|Location|VersionedAssets)\b", combined):
				components.add("boundary:xcm-outbound")
			for component in sorted(components):
				g.edge(ident, component, "test-covers-component", evidence="source-reference", confidence="reference")
			for segment, segment_source in segments:
				dispatch_ranges = macro_argument_ranges(segment, ("assert_ok", "assert_noop", "assert_err"))
				for call in re.finditer(r"\b([A-Z][A-Za-z0-9_]*)::([a-zA-Z0-9_]+)\s*\(", segment):
					alias, method = call.groups()
					crate = aliases.get(alias)
					if not crate:
						continue
					asserted = any(start <= call.start() < end for start, end in dispatch_ranges)
					call_confidence = "direct-dispatch-assertion" if asserted else (
						"helper-propagated" if segment_source == "local-helper" else "source-call")
					for function in functions[(component_id(crate), method)]:
						g.edge(ident, function, "test-covers-entrypoint", evidence=segment_source,
							confidence=call_confidence, asserted=asserted)


def semantic_source_prefix(owner: str) -> str | None:
	if owner == "runtime:hydradx":
		return "runtime/hydradx/src/"
	if owner.startswith("pallet:"):
		return f"pallets/{owner.removeprefix('pallet:')}/src/"
	if owner.startswith("precompile:"):
		return f"precompiles/{owner.removeprefix('precompile:')}/src/"
	return None


def semantic_package_function(node: dict, owner: str) -> bool:
	prefix = semantic_source_prefix(owner)
	if node.get("kind") != "function":
		return False
	file = node.get("file") or ""
	return file.startswith(prefix) if prefix and file else node.get("owner") == owner


def resolve_semantic_symbol(g: Graph, candidates: list[str], symbol: str, owner: str) -> str | None:
	if len(candidates) == 1:
		return candidates[0]
	prefix = semantic_source_prefix(owner)
	if not prefix:
		return None
	qualified = []
	for candidate in candidates:
		file = g.nodes[candidate].get("file") or ""
		if not file.startswith(prefix):
			continue
		module = file.removeprefix(prefix).removesuffix(".rs")
		module = module.removesuffix("/mod")
		if module == "lib":
			module = ""
		module = module.replace("/", "::")
		if module and re.search(rf"(?:^|[<\s]){re.escape(module)}::", symbol):
			qualified.append(candidate)
	return qualified[0] if len(qualified) == 1 else None


def merge_rapx(g: Graph, path: Path, owner: str) -> int:
	functions: dict[str, list[str]] = defaultdict(list)
	for node in g.nodes.values():
		if semantic_package_function(node, owner):
			functions[node["name"]].append(node["id"])
	current = None
	added = 0
	for line in path.read_text(errors="replace").splitlines():
		caller = re.match(r"^  (.+) calls:$", line)
		if caller:
			symbol = caller.group(1)
			name = symbol.split("::")[-1].split("{")[0]
			current = resolve_semantic_symbol(g, functions.get(name, []), symbol, owner)
			continue
		callee = re.match(r"^    -> (.+)$", line)
		if not current or not callee:
			continue
		target_name = callee.group(1)
		method = target_name.split("::")[-1].split("{")[0]
		local_call = target_name.startswith("<impl pallet::Pallet") or target_name.startswith("pallet::Pallet")
		target = resolve_semantic_symbol(g, functions.get(method, []), target_name, owner) if local_call else None
		if not target:
			pallet = re.match(r"(pallet_[a-zA-Z0-9_]+)::Pallet", target_name)
			if pallet:
				target = f"pallet:{pallet.group(1).removeprefix('pallet_').replace('_', '-')}"
			elif "hydradx_traits::evm::EVM::call" in target_name:
				target = "boundary:evm-execution"
		if target and target != current:
			g.edge(current, target, "rapx-call", method=method, semantic_source="rapx",
				rapx_output=path.name)
			added += 1
	return added


def semantic_tool_inputs(tool: str) -> dict:
	scripts = Path(__file__).parent
	paths = [scripts / ("collect_mir.py" if tool == "rustc-mir" else "collect_rapx.py"),
		scripts / "analysis_provenance.py"]
	if tool == "rustc-mir":
		paths.append(scripts / "collect_rapx.py")
	return analysis_provenance.tool_input_fingerprint(paths)


def semantic_artifact_path(manifest_path: Path, value: object, context: str) -> Path:
	if not isinstance(value, str) or not value:
		raise ValueError(f"{context} must be a non-empty relative path")
	relative = Path(value)
	if relative.is_absolute():
		raise ValueError(f"{context} must stay inside the manifest directory")
	parent = manifest_path.parent.resolve()
	path = (parent / relative).resolve()
	try:
		path.relative_to(parent)
	except ValueError as error:
		raise ValueError(f"{context} must stay inside the manifest directory") from error
	return path


def valid_sha256(value: object) -> bool:
	return isinstance(value, str) and bool(re.fullmatch(r"[0-9a-f]{64}", value))


def validate_semantic_manifest(manifest: dict, path: Path, tool: str, root: Path | None = None) -> None:
	if manifest.get("schema_version") != 2 or manifest.get("tool") != tool:
		raise ValueError(f"{tool} manifest must use schema_version 2")
	collector_module = collect_mir if tool == "rustc-mir" else collect_rapx
	collector = Path(__file__).with_name("collect_mir.py" if tool == "rustc-mir" else "collect_rapx.py")
	if manifest.get("toolchain") != collector_module.TOOLCHAIN:
		raise ValueError(f"{tool} manifest toolchain is invalid")
	timeout = manifest.get("timeout_seconds")
	if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
		raise ValueError(f"{tool} manifest timeout is invalid")
	packages = manifest.get("packages")
	requested = manifest.get("requested_packages")
	if not isinstance(packages, list) or not isinstance(requested, list) \
		or any(not isinstance(package, str) or not package for package in requested) \
		or len(requested) != len(set(requested)) or any(not isinstance(package, dict) for package in packages):
		raise ValueError(f"{tool} manifest package inventory is invalid")
	package_names = [package.get("package") for package in packages]
	if any(not isinstance(package, str) or not package for package in package_names) \
		or len(package_names) != len(set(package_names)) or sorted(package_names) != sorted(requested):
		raise ValueError(f"{tool} manifest package inventory is incomplete")
	provenance = manifest.get("provenance", {})
	source_count = provenance.get("source_inputs", {}).get("file_count")
	tool_inputs = provenance.get("tool_inputs")
	if not valid_sha256(provenance.get("source_inputs", {}).get("sha256")) \
		or not isinstance(source_count, int) or isinstance(source_count, bool) or source_count < 0 \
		or not valid_sha256(provenance.get("collector_sha256")) \
		or provenance.get("toolchain") != collector_module.TOOLCHAIN \
		or not analysis_provenance.valid_tool_input_fingerprint(tool_inputs):
		raise ValueError(f"{tool} manifest has no verified source provenance")
	if tool_inputs["files"].get(collector.name) != provenance["collector_sha256"]:
		raise ValueError(f"{tool} manifest collector is not bound to its tooling fingerprint")
	if root is not None:
		current_sources = analysis_provenance.tree_fingerprint(root)
		if provenance["source_inputs"] != current_sources:
			raise ValueError(f"{tool} manifest source fingerprint is stale")
		if provenance["collector_sha256"] != analysis_provenance.file_sha256(collector):
			raise ValueError(f"{tool} manifest collector fingerprint is stale")
		if tool_inputs != semantic_tool_inputs(tool):
			raise ValueError(f"{tool} manifest tooling fingerprint is stale")
	workspace = None
	if root is not None and requested:
		workspace = {package["package"]: package for package in collect_rapx.workspace_packages(root)}
		if any(package not in workspace for package in requested):
			raise ValueError(f"{tool} manifest requests an unknown workspace package")
	requested_analyses = None
	if tool == "rapx":
		requested_analyses = manifest.get("requested_analyses")
		if not isinstance(requested_analyses, list) or not requested_analyses \
			or len(requested_analyses) != len(set(requested_analyses)) \
			or any(analysis not in {"callgraph", "mir", "dataflow"} for analysis in requested_analyses):
			raise ValueError("rapx manifest requested analysis inventory is invalid")
	for package in packages:
		if not isinstance(package.get("owner"), str) or not package["owner"] \
			or not isinstance(package.get("manifest"), str) or not package["manifest"]:
			raise ValueError(f"{tool} package metadata is invalid: {package.get('package')!r}")
		package_metadata = {key: package[key] for key in ("package", "owner", "manifest")}
		manifest_relative = Path(package["manifest"])
		if manifest_relative.is_absolute() or ".." in manifest_relative.parts:
			raise ValueError(f"{tool} package manifest path is invalid: {package['package']}")
		if workspace is not None and workspace[package["package"]] != package_metadata:
			raise ValueError(f"{tool} package metadata does not match the workspace: {package['package']}")
		if tool == "rapx":
			analyses = package.get("analyses")
			if not isinstance(analyses, dict) or set(analyses) != set(requested_analyses):
				raise ValueError(f"rapx package analysis inventory is incomplete: {package['package']}")
			artifacts = [(f"{package['package']}.{name}", name, analysis, "path")
				for name, analysis in analyses.items()]
			statuses = {"ok", "failed", "timeout", "invalid-output"}
		else:
			artifacts = [(package["package"], None, package, "artifact")]
			statuses = {"ok", "failed", "timeout"}
		for context, analysis_name, artifact, path_field in artifacts:
			if not isinstance(artifact, dict) or artifact.get("status") not in statuses:
				raise ValueError(f"{tool} artifact status is invalid: {context}")
			expected_command = (collect_rapx.analysis_command(package_metadata, analysis_name, timeout)
				if analysis_name is not None else collect_mir.mir_command(package_metadata))
			if artifact.get("command") != expected_command:
				raise ValueError(f"{tool} artifact command is invalid: {context}")
			fingerprint_context = ({**package_metadata, "analysis": analysis_name}
				if analysis_name is not None else package_metadata)
			expected_fingerprint = analysis_provenance.command_fingerprint(
				provenance, fingerprint_context, expected_command)
			if artifact.get("input_fingerprint") != expected_fingerprint:
				raise ValueError(f"{tool} artifact input fingerprint is invalid: {context}")
			semantic_artifact_path(path, artifact.get(path_field), f"{context}.{path_field}")
			if artifact.get("log") is not None:
				semantic_artifact_path(path, artifact["log"], f"{context}.log")
			if artifact["status"] != "ok":
				continue
			artifact_path = semantic_artifact_path(path, artifact.get(path_field), f"{context}.{path_field}")
			if not artifact_path.is_file() or not valid_sha256(artifact.get("artifact_sha256")) \
				or hashlib.sha256(artifact_path.read_bytes()).hexdigest() != artifact["artifact_sha256"]:
				raise ValueError(f"{tool} artifact is missing or stale: {artifact.get(path_field)}")


def merge_rapx_manifest(g: Graph, path: Path, root: Path | None = None) -> int:
	manifest = json.loads(path.read_text())
	validate_semantic_manifest(manifest, path, "rapx", root)
	added = 0
	coverage = []
	for package in manifest.get("packages", []):
		analysis = package.get("analyses", {}).get("callgraph", {})
		coverage.append({"package": package["package"], "owner": package["owner"],
			"callgraph": analysis.get("status", "missing"),
			"mir": package.get("analyses", {}).get("mir", {}).get("status", "not-requested"),
			"dataflow": package.get("analyses", {}).get("dataflow", {}).get("status", "not-requested")})
		if analysis.get("status") != "ok":
			continue
		added += merge_rapx(g, path.parent / analysis["path"], package["owner"])
	g.node("semantic-analysis:rapx-coverage", "semantic-coverage", tool="rapx",
		manifest=path.name, manifest_sha256=analysis_provenance.file_sha256(path), packages=coverage,
		callgraph_success=sum(item["callgraph"] == "ok" for item in coverage), total_packages=len(coverage))
	return added


def mir_operation(line: str) -> str | None:
	item = mir_parser.operation(line)
	return item["kind"] if item else None


def mir_order_flags(blocks: dict[int, dict]) -> tuple[bool, bool]:
	normalized = {block: {**data, "normal_successors": data.get("normal_successors", data.get("successors", []))}
		for block, data in blocks.items()}
	return mir_parser.order_flags(normalized)


def merge_rustc_mir(g: Graph, path: Path, owner: str) -> int:
	def in_package(node: dict) -> bool:
		return semantic_package_function(node, owner)

	def trait_matches(target: str, trait_path: str, function_owner: str) -> bool:
		target_node = g.nodes.get(target, {})
		target_trait = target_node.get("config_trait")
		if not target_trait:
			return False
		trait_path = re.sub(r"\s+", "", trait_path)
		if trait_path in {"Config", "pallet::Config", "crate::Config", "crate::pallet::Config",
			"self::Config", "super::Config", "super::pallet::Config"}:
			return config_component(target_trait) == function_owner
		crate, separator, remainder = trait_path.partition("::")
		canonical = f"{canonical_config_crate(crate)}::{remainder}" if separator else trait_path
		return target_trait == canonical

	def instance_matches(target: str, raw_instance: str | None, fallback: bool = False) -> bool:
		target_node = g.nodes.get(target, {})
		target_instance = target_node.get("config_instance")
		if raw_instance is None:
			return target_instance is None
		instance = re.sub(r"\s+", "", raw_instance).rsplit("::", 1)[-1]
		if re.fullmatch(r"I[0-9]*", instance):
			return target_node.get("runtime_instance") is not None
		if re.fullmatch(r"Instance[0-9]+", instance):
			return target_instance == instance
		return not fallback

	functions: dict[str, list[str]] = defaultdict(list)
	for node in g.nodes.values():
		if in_package(node):
			functions[node["name"]].append(node["id"])
	instances = mir_parser.parse(path.read_text(errors="replace"))
	added = 0
	matched_functions = set()
	matched_instances = 0
	for instance in instances:
		candidates = functions.get(instance["name"], [])
		if instance.get("source_file"):
			candidates = [candidate for candidate in candidates
				if g.nodes[candidate].get("file") == instance["source_file"]]
		if instance.get("impl_line") is not None and candidates:
			following = [candidate for candidate in candidates
				if g.nodes[candidate].get("line", 0) >= instance["impl_line"]]
			if following:
				candidates = [min(following,
					key=lambda candidate: g.nodes[candidate].get("line", 0) - instance["impl_line"])]
		if len(candidates) != 1:
			continue
		function = candidates[0]
		function_owner = g.nodes[function].get("owner") or owner
		source_associated_targets: dict[tuple[str, str | None], set[str]] = defaultdict(set)
		for source_edge in g.edges:
			if source_edge["source"] != function or source_edge["kind"] not in {
				"dynamic-call", "runtime-config-read", "runtime-config-type-reference", "weight-evaluation",
			}:
				continue
			target_node = g.nodes.get(source_edge["target"], {})
			associated_type = target_node.get("associated_type")
			if associated_type:
				source_associated_targets[(associated_type, source_edge.get("method"))].add(source_edge["target"])
		function_blocks = instance["blocks"]
		operations = [{"block": block, **operation} for block, data in sorted(function_blocks.items())
			for operation in data["operations"]]
		if not operations:
			continue
		matched_functions.add(function)
		matched_instances += 1
		before, after = mir_parser.order_flags(function_blocks)
		unwind_before, unwind_after = mir_parser.order_flags(function_blocks, "unwind_successors")
		g.nodes[function].setdefault("mir_operations", []).extend(
			[{**operation, "instance": instance["id"]} for operation in operations if operation["kind"] != "call"])
		g.nodes[function]["mir_write_before_external"] = \
			g.nodes[function].get("mir_write_before_external", False) or before
		g.nodes[function]["mir_write_after_external"] = \
			g.nodes[function].get("mir_write_after_external", False) or after
		g.nodes[function]["mir_unwind_write_before_external"] = \
			g.nodes[function].get("mir_unwind_write_before_external", False) or unwind_before
		g.nodes[function]["mir_unwind_write_after_external"] = \
			g.nodes[function].get("mir_unwind_write_after_external", False) or unwind_after
		g.nodes[function]["mir_source"] = path.name
		instance_node = f"mir-instance:{owner}:{instance['id']}"
		g.node(instance_node, "mir-instance", owner=function_owner, function=function, symbol=instance["symbol"],
			source_file=instance.get("source_file"), impl_line=instance.get("impl_line"), semantic_source="rustc-mir")
		g.edge(function, instance_node, "has-mir-instance", semantic_source="rustc-mir")
		operation_nodes: dict[int, list[str]] = defaultdict(list)
		for block, data in sorted(function_blocks.items()):
			for index, operation in enumerate(data["operations"]):
				callee = operation.get("callee") or ""
				component_match = re.search(
					r"\b((?:pallet_|orml_|cumulus_pallet_|frame_)[A-Za-z0-9_]+)::Pallet", callee)
				associated_call = mir_associated_call(callee)
				method = associated_call["method"] if associated_call \
					else (callee.rsplit("::", 1)[-1].split("<", 1)[0] if callee else None)
				associated_targets = set()
				if associated_call:
					associated_type = str(associated_call["associated_type"])
					candidates_by_type = source_associated_targets.get((associated_type, method), set()) \
						or source_associated_targets.get((associated_type, None), set())
					associated_targets = {target for target in candidates_by_type
						if trait_matches(target, str(associated_call["trait_path"]), function_owner)
						and instance_matches(target, associated_call["config_instance"])}
					if not associated_targets:
						associated_targets = {node["id"] for node in g.nodes.values()
							if node.get("associated_type") == associated_type
							and node.get("runtime_instance") is not None
							and trait_matches(node["id"], str(associated_call["trait_path"]), function_owner)
							and instance_matches(node["id"], associated_call["config_instance"], fallback=True)
							and node.get("runtime_active") is not False}
				local_target = None
				if operation["kind"] == "call" and method and (callee.startswith(("pallet::", "<impl", "Self::"))):
					local_candidates = functions.get(method, [])
					local_target = local_candidates[0] if len(local_candidates) == 1 else None
				relevant_call = operation["kind"] != "call" or component_match or associated_targets or local_target
				if not relevant_call:
					continue
				ident = f"mir-operation:{instance_node}:{block}:{index}"
				operation_nodes[block].append(ident)
				g.node(ident, "mir-operation", owner=function_owner, function=function, instance=instance_node, block=block,
					operation_index=index, operation=operation["kind"], statement=operation["statement"],
					callee=operation.get("callee"), semantic_source="rustc-mir")
				g.edge(instance_node, ident, "mir-contains-operation", block=block, operation_index=index)
				if operation["kind"] == "evm-call":
					g.edge(ident, "boundary:evm-execution", "mir-enters-evm", semantic_source="rustc-mir")
				elif operation["kind"] == "frame-dispatch":
					g.edge(ident, "boundary:frame-dispatch", "mir-dispatches-frame", semantic_source="rustc-mir")
				elif operation["kind"] == "external-call":
					g.edge(ident, "boundary:external-execution", "mir-external-call", semantic_source="rustc-mir")
				elif operation["kind"].startswith("storage-"):
					storage = re.search(r"_GeneratedPrefixForStorage([A-Za-z0-9_]+)", operation["statement"])
					if storage:
						target = f"storage:{function_owner}:{storage.group(1)}"
						g.node(target, "storage", owner=function_owner)
						g.edge(ident, target, "mir-storage-access", operation=operation["kind"],
							semantic_source="rustc-mir")
				if local_target:
					g.edge(ident, local_target, "mir-call", method=method, semantic_source="rustc-mir")
				if component_match:
					target = component_id(component_match.group(1))
					if target != function_owner:
						g.edge(ident, target, "mir-component-call", method=method, semantic_source="rustc-mir")
				if associated_targets:
					for target in sorted(associated_targets):
						target_node = g.nodes[target]
						edge_metadata = {"method": method, "semantic_source": "rustc-mir"}
						if target_node.get("config_trait") is not None:
							edge_metadata.update({"config_trait": target_node["config_trait"],
								"config_instance": target_node.get("config_instance")})
						g.edge(ident, target, "mir-dynamic-call", **edge_metadata)
						for resolved in g.nodes[target].get("resolved_targets", []):
							g.edge(ident, resolved, "mir-resolved-call", semantic_source="rustc-mir",
								associated_type=target)
			for left, right in zip(operation_nodes[block], operation_nodes[block][1:]):
				g.edge(left, right, "mir-control-flow", semantic_source="rustc-mir")
		for block, identifiers in operation_nodes.items():
			if not identifiers:
				continue
			for successor_kind, edge_kind in (("normal_successors", "mir-control-flow"),
				("unwind_successors", "mir-unwind-flow")):
				queue = deque(function_blocks[block].get(successor_kind, []))
				seen_blocks = set()
				found = False
				while queue:
					target_block = queue.popleft()
					if target_block in seen_blocks or target_block not in function_blocks:
						continue
					seen_blocks.add(target_block)
					if operation_nodes.get(target_block):
						g.edge(identifiers[-1], operation_nodes[target_block][0], edge_kind,
							semantic_source="rustc-mir")
						found = True
					else:
						queue.extend(function_blocks[target_block].get(successor_kind, []))
				if successor_kind == "unwind_successors" and function_blocks[block].get(successor_kind) and not found:
					unwind_exit = f"mir-unwind-exit:{instance_node}"
					g.node(unwind_exit, "mir-unwind-exit", owner=function_owner,
						function=function, instance=instance_node)
					g.edge(identifiers[-1], unwind_exit, "mir-unwind-flow", semantic_source="rustc-mir")
		added += sum(len(identifiers) for identifiers in operation_nodes.values())
	source_functions = {node["id"] for node in g.nodes.values() if in_package(node)
		and not (node.get("file") or "").endswith("weights.rs")}
	g.node(f"semantic-analysis:rustc-mir:{owner}", "semantic-coverage", tool="rustc-mir",
		owner=owner, artifact=path.name, artifact_sha256=analysis_provenance.file_sha256(path),
		matched_functions=len(matched_functions),
		matched_instances=matched_instances, source_functions_total=len(source_functions),
		source_function_coverage=(len(matched_functions) / len(source_functions)) if source_functions else None,
		operation_count=added)
	return added


def merge_rustc_mir_manifest(g: Graph, path: Path, root: Path | None = None) -> int:
	manifest = json.loads(path.read_text())
	validate_semantic_manifest(manifest, path, "rustc-mir", root)
	added = 0
	for package in manifest.get("packages", []):
		if package.get("status") == "ok":
			added += merge_rustc_mir(g, path.parent / package["artifact"], package["owner"])
	g.node("semantic-analysis:rustc-mir-workspace", "semantic-coverage", tool="rustc-mir-workspace",
		manifest=path.name, manifest_sha256=analysis_provenance.file_sha256(path),
		packages=manifest.get("packages", []),
		success=sum(package.get("status") == "ok" for package in manifest.get("packages", [])),
		total=len(manifest.get("packages", [])))
	classify_unresolved(g)
	return added


def merge_contracts(g: Graph, path: Path) -> int:
	payload = json.loads(path.read_text())
	schema_version = payload.get("schema_version", 1)
	if not isinstance(schema_version, int) or isinstance(schema_version, bool) or schema_version not in {1, 2}:
		raise ValueError(f"unsupported contract manifest schema_version: {schema_version!r}")
	if schema_version == 1:
		observations = {(item["project"], item["network"], item["address"].lower()): item
			for item in payload.get("observations", [])}
		merged: dict[tuple[str, str, str], dict] = {}
		for contract in payload.get("contracts", []):
			key = (contract["project"], contract["network"], contract["address"].lower())
			entry = merged.setdefault(key,
				{"names": set(), "artifacts": set(), "signatures": set(), "records": []})
			entry["names"].add(contract["name"])
			entry["artifacts"].add(contract["artifact"])
			entry["signatures"].update(contract.get("abi_signatures", []))
			entry["records"].append({key: value for key, value in contract.items()
				if key not in {"abi_signatures", "artifact", "name", "project", "network", "address"}})
		contracts_by_address: dict[str, list[str]] = defaultdict(list)
		for project, network, address in merged:
			contracts_by_address[address].append(f"deployed-contract:{project}:{network}:{address}")
		for (project, network, address), entry in sorted(merged.items()):
			ident = f"deployed-contract:{project}:{network}:{address}"
			observation_data = {key: value
				for key, value in observations.get((project, network, address), {}).items()
				if key not in {"project", "network", "address"}}
			g.node(ident, "deployed-contract", domain="evm-contract", project=project, network=network,
				address=address, names=sorted(entry["names"]), artifacts=sorted(entry["artifacts"]),
				deployment_records=entry["records"], **observation_data)
			for signature in sorted(entry["signatures"]):
				function = f"contract-function:{project}:{network}:{address}:{signature}"
				g.node(function, "contract-function", owner=ident, signature=signature)
				g.edge(ident, function, "exposes-function")
				_, selector_node = ensure_evm_selector(g, signature)
				g.edge(selector_node, function, "selector-matches-contract-function")
			names = " ".join(entry["names"]).lower()
			if any(value in names for value in ("pool-proxy", "pooladdressesprovider", "aaveoracle")):
				g.edge("component:evm:aave_trade_executor", ident, "uses-deployed-contract")
			if any(value in names for value in ("gho", "hollar", "flashminter")):
				g.edge("pallet:hsm", ident, "uses-deployed-contract")
			if "gigahdx" in network or "giga" in names:
				g.edge("pallet:gigahdx", ident, "uses-deployed-contract")
			if project == "whm" and any(value in names for value in ("emitter", "basejump", "router")):
				g.edge("component:xcm:router", ident, "uses-deployed-contract")
			if project == "whm" and any(value in names for value in ("transactor", "landing", "receiver")):
				g.edge("component:xcm:asset-transactor", ident, "uses-deployed-contract")
			observation = observations.get((project, network, address), {})
			if observation.get("implementation"):
				implementation_address = observation["implementation"].lower()
				implementation_targets = contracts_by_address.get(implementation_address)
				implementation = (implementation_targets or
					[f"evm-address:{network}:{implementation_address}"])[0]
				g.node(implementation, "deployed-contract" if implementation_targets else "evm-address",
					domain="evm-contract", network=network, address=implementation_address)
				g.edge(ident, implementation, "proxy-implementation", semantic_source="eip-1967")
			for target_address in observation.get("embedded_addresses", []):
				target = f"evm-address:{network}:{target_address.lower()}"
				g.node(target, "evm-address", domain="evm-contract", network=network,
					address=target_address.lower())
				g.edge(ident, target, "bytecode-embeds-address", semantic_source="deployed-bytecode")
		for configuration in payload.get("runtime_configurations", []):
			address = configuration["address"].lower()
			targets = contracts_by_address.get(address) or [f"evm-address:hydration:{address}"]
			for target in targets:
				g.node(target, g.nodes.get(target, {}).get("kind", "evm-address"),
					domain="evm-contract", address=address)
				g.edge(configuration["component"], target, "runtime-configures-contract",
					storage=configuration["storage"], asset_id=configuration.get("asset_id"),
					block_hash=payload.get("substrate_snapshot", {}).get("block_hash"))
		g.node("semantic-analysis:contract-deployments", "semantic-coverage", tool="deployment-artifacts",
			manifest=path.name, manifest_sha256=analysis_provenance.file_sha256(path),
			contracts=len(merged), records=len(payload.get("contracts", [])))
		return len(merged)

	address_pattern = re.compile(r"^0x[0-9a-fA-F]{40}$")

	def normalized_address(value: object) -> str:
		if not isinstance(value, str) or not address_pattern.fullmatch(value):
			raise ValueError(f"invalid EVM address in schema-v2 contract manifest: {value!r}")
		return value.lower()

	def canonical_address(value: object = None, chain_id: object = None,
		address: object = None) -> tuple[int, str, str]:
		if value is not None:
			match = re.fullmatch(r"eip155:([0-9]+):(0x[0-9a-fA-F]{40})", str(value))
			if not match or int(match.group(1)) < 1:
				raise ValueError(f"invalid canonical chain address in schema-v2 contract manifest: {value!r}")
			parsed_chain = int(match.group(1))
			parsed_address = normalized_address(match.group(2))
			if chain_id is not None and (not isinstance(chain_id, int) or isinstance(chain_id, bool)
				or chain_id != parsed_chain):
				raise ValueError(f"chain id does not match canonical chain address: {value!r}")
			if address is not None and normalized_address(address) != parsed_address:
				raise ValueError(f"address does not match canonical chain address: {value!r}")
			return parsed_chain, parsed_address, f"eip155:{parsed_chain}:{parsed_address}"
		if not isinstance(chain_id, int) or isinstance(chain_id, bool) or chain_id < 1:
			raise ValueError(f"invalid EVM chain id in schema-v2 contract manifest: {chain_id!r}")
		parsed_address = normalized_address(address)
		return chain_id, parsed_address, f"eip155:{chain_id}:{parsed_address}"

	aliases: dict[tuple[str, str, str], dict] = {}
	canonical_selectors: dict[str, set[str]] = defaultdict(set)
	for contract in payload.get("contracts", []):
		address = normalized_address(contract.get("address"))
		project = contract.get("project")
		network = contract.get("network")
		if not isinstance(project, str) or not project or not isinstance(network, str) or not network:
			raise ValueError(f"invalid deployment alias identity: {contract!r}")
		key = (project, network, address)
		entry = aliases.setdefault(key, {"names": set(), "artifacts": set(), "functions": defaultdict(set),
			"records": []})
		if isinstance(contract.get("name"), str):
			entry["names"].add(contract["name"])
		if isinstance(contract.get("artifact"), str):
			entry["artifacts"].add(contract["artifact"])
		for function in contract.get("abi_functions", []):
			if not isinstance(function, dict) or not isinstance(function.get("signature"), str):
				raise ValueError(f"invalid canonical ABI function: {function!r}")
			selector = function.get("selector")
			if not isinstance(selector, str) or not re.fullmatch(r"0x[0-9a-fA-F]{8}", selector):
				raise ValueError(f"invalid canonical ABI selector: {function!r}")
			selector = selector.lower()
			entry["functions"][function["signature"]].add(selector)
			canonical_selectors[function["signature"]].add(selector)
		for signature in contract.get("abi_signatures", []):
			if isinstance(signature, str):
				entry["functions"].setdefault(signature, set())
		entry["records"].append({key: value for key, value in contract.items() if key not in {
			"abi_functions", "abi_signatures", "artifact", "name", "project", "network", "address"}})
	conflicting_selectors = {signature: sorted(selectors) for signature, selectors in canonical_selectors.items()
		if len(selectors) != 1}
	if conflicting_selectors:
		raise ValueError(f"canonical ABI signatures have conflicting selectors: {conflicting_selectors!r}")

	physical_observations: dict[str, list[dict]] = defaultdict(list)
	alias_physical: dict[tuple[str, str, str], str] = {}
	for observation in payload.get("observations", []):
		chain_id, address, chain_address = canonical_address(observation.get("chain_address_id"),
			observation.get("chain_id"), observation.get("address"))
		physical_observations[chain_address].append(observation)
		project = observation.get("project")
		network = observation.get("network")
		if isinstance(project, str) and isinstance(network, str):
			key = (project, network, address)
			previous = alias_physical.setdefault(key, chain_address)
			if previous != chain_address:
				raise ValueError(f"deployment alias resolves to multiple chain addresses: {key!r}")
	chains_by_network: dict[str, set[int]] = defaultdict(set)
	for chain in payload.get("chains", []):
		chain_id = chain.get("evm_chain_id")
		networks = chain.get("deployment_networks")
		if (not isinstance(chain_id, int) or isinstance(chain_id, bool) or chain_id < 1 or
			not isinstance(networks, list) or any(not isinstance(network, str) or not network for network in networks)):
			raise ValueError(f"invalid chain descriptor in schema-v2 contract manifest: {chain!r}")
		for network in networks:
			chains_by_network[network].add(chain_id)
	for key in aliases:
		if key in alias_physical or len(chains_by_network[key[1]]) != 1:
			continue
		_, _, chain_address = canonical_address(chain_id=next(iter(chains_by_network[key[1]])), address=key[2])
		alias_physical[key] = chain_address
		physical_observations.setdefault(chain_address, [])

	physical_ids = {chain_address: f"deployed-contract:{chain_address}"
		for chain_address in physical_observations}
	aliases_by_physical: dict[str, list[tuple[str, str, str]]] = defaultdict(list)
	for key, chain_address in alias_physical.items():
		if key in aliases:
			aliases_by_physical[chain_address].append(key)

	for chain_address, observations in sorted(physical_observations.items()):
		chain_id, address, _ = canonical_address(chain_address)
		alias_keys = sorted(aliases_by_physical.get(chain_address, []))
		alias_entries = [aliases[key] for key in alias_keys]
		sorted_observations = sorted(observations, key=lambda item: json.dumps(item, sort_keys=True))
		observation_data = {}
		observation_conflicts = {}
		observation_keys = sorted({key for observation in observations for key in observation} - {
			"project", "network", "chain_id", "address", "chain_address_id"})
		for key in observation_keys:
			values = {json.dumps(observation.get(key), sort_keys=True): observation.get(key)
				for observation in observations}
			if len(values) == 1:
				observation_data[key] = next(iter(values.values()))
			else:
				observation_conflicts[key] = [values[encoded] for encoded in sorted(values)]
		alias_ids = [f"deployment-alias:{project}:{network}:{alias_address}"
			for project, network, alias_address in alias_keys]
		projects = sorted({key[0] for key in alias_keys})
		networks = sorted({key[1] for key in alias_keys})
		g.node(physical_ids[chain_address], "deployed-contract", domain="evm-contract",
			chain_id=chain_id, chain_address_id=chain_address, address=address,
			onchain_observed=bool(observations),
			project=",".join(projects) or None, network=",".join(networks) or None,
			projects=projects, networks=networks,
			names=sorted({name for entry in alias_entries for name in entry["names"]}),
			artifacts=sorted({artifact for entry in alias_entries for artifact in entry["artifacts"]}),
			deployment_aliases=alias_ids,
			observations=sorted_observations,
			observation_conflicts=observation_conflicts,
			**observation_data)

	def ensure_physical_address(chain_address: str) -> str:
		chain_id, address, canonical = canonical_address(chain_address)
		if canonical in physical_ids:
			return physical_ids[canonical]
		ident = f"evm-address:{canonical}"
		g.node(ident, g.nodes.get(ident, {}).get("kind", "evm-address"), domain="evm-contract",
			chain_id=chain_id, chain_address_id=canonical, address=address)
		return ident

	for (project, network, address), entry in sorted(aliases.items()):
		alias = f"deployment-alias:{project}:{network}:{address}"
		chain_address = alias_physical.get((project, network, address))
		functions = []
		for signature, selectors in sorted(entry["functions"].items()):
			function = {"signature": signature}
			if len(selectors) == 1:
				function["selector"] = next(iter(selectors))
			elif selectors:
				function["selectors"] = sorted(selectors)
			functions.append(function)
		g.node(alias, "deployment-alias", domain="evm-contract", project=project, network=network,
			address=address, chain_address_id=chain_address, names=sorted(entry["names"]),
			artifacts=sorted(entry["artifacts"]), abi_functions=functions,
			deployment_records=sorted(entry["records"], key=lambda item: json.dumps(item, sort_keys=True)))
		for record in entry["records"]:
			step_name = record.get("migration_step")
			if not isinstance(step_name, str) or not step_name:
				continue
			step = f"deployment-step:{project}:{network}:{step_name}"
			g.node(step, "deployment-step", domain="evm-contract", project=project, network=network,
				migration_step=step_name)
			g.edge(step, alias, "deployment-step-produces-alias", field=record.get("field"),
				deployment_role=record.get("deployment_role"), artifact_sha256=record.get("artifact_sha256"))
		if chain_address:
			g.edge(alias, physical_ids[chain_address], "deployment-aliases-contract",
				semantic_source="deployment-artifact")
		target = physical_ids[chain_address] if chain_address else alias
		names = " ".join(entry["names"]).lower()
		evidence = {"deployment_alias": alias}
		if any(value in names for value in ("pool-proxy", "pooladdressesprovider", "aaveoracle")):
			g.edge("component:evm:aave_trade_executor", target, "uses-deployed-contract", **evidence)
		if any(value in names for value in ("gho", "hollar", "flashminter")):
			g.edge("pallet:hsm", target, "uses-deployed-contract", **evidence)
		if "gigahdx" in network or "giga" in names:
			g.edge("pallet:gigahdx", target, "uses-deployed-contract", **evidence)
		if project == "whm" and any(value in names for value in ("emitter", "basejump", "router")):
			g.edge("component:xcm:router", target, "uses-deployed-contract", **evidence)
		if project == "whm" and any(value in names for value in ("transactor", "landing", "receiver")):
			g.edge("component:xcm:asset-transactor", target, "uses-deployed-contract", **evidence)

	references: dict[tuple[str, str, str], list[dict]] = defaultdict(list)
	for reference in payload.get("address_references", []):
		if not isinstance(reference, dict):
			raise ValueError(f"invalid deployment address reference: {reference!r}")
		project = reference.get("project")
		network = reference.get("network")
		role = reference.get("role")
		field = reference.get("field")
		step_name = reference.get("migration_step")
		if any(not isinstance(value, str) or not value for value in (project, network, role, field, step_name)):
			raise ValueError(f"invalid deployment address reference: {reference!r}")
		address = normalized_address(reference.get("address"))
		references[(project, network, address)].append(reference)
	for (project, network, address), records in sorted(references.items()):
		reference_id = f"deployment-address-reference:{project}:{network}:{address}"
		candidate_chain_address = None
		if len(chains_by_network[network]) == 1:
			candidate_chain_id = next(iter(chains_by_network[network]))
			candidate_chain_address = f"eip155:{candidate_chain_id}:{address}"
		g.node(reference_id, "deployment-address-reference", domain="evm-contract", project=project,
			network=network, address=address, candidate_chain_address_id=candidate_chain_address,
			roles=sorted({record["role"] for record in records}),
			reference_records=sorted(records, key=lambda item: json.dumps(item, sort_keys=True)),
			onchain_observed=False)
		for record in records:
			step_name = record["migration_step"]
			step = f"deployment-step:{project}:{network}:{step_name}"
			g.node(step, "deployment-step", domain="evm-contract", project=project, network=network,
				migration_step=step_name)
			g.edge(step, reference_id, "deployment-step-references-address", role=record["role"],
				field=record["field"], artifact=record.get("artifact"),
				artifact_sha256=record.get("artifact_sha256"))

	for chain_address, ident in sorted(physical_ids.items()):
		chain_id, _, _ = canonical_address(chain_address)
		functions: dict[str, dict[str, set[str]]] = defaultdict(lambda: {"selectors": set(), "aliases": set()})
		for key in aliases_by_physical.get(chain_address, []):
			alias = f"deployment-alias:{key[0]}:{key[1]}:{key[2]}"
			for signature, selectors in aliases[key]["functions"].items():
				functions[signature]["selectors"].update(selectors)
				functions[signature]["aliases"].add(alias)
		for signature, function_data in sorted(functions.items()):
			function = f"contract-function:{chain_address}:{signature}"
			selectors = sorted(function_data["selectors"])
			node_data = {"owner": ident, "domain": "evm-contract", "chain_id": chain_id,
				"chain_address_id": chain_address, "signature": signature,
				"deployment_aliases": sorted(function_data["aliases"])}
			if len(selectors) == 1:
				node_data["selector"] = selectors[0]
			elif selectors:
				node_data["selector_conflicts"] = selectors
			g.node(function, "contract-function", **node_data)
			g.edge(ident, function, "exposes-function")
			_, selector_node = ensure_evm_selector(g, signature, selectors[0] if len(selectors) == 1 else None)
			g.edge(selector_node, function, "selector-matches-contract-function")

	for chain_address, observations in sorted(physical_observations.items()):
		chain_id, _, _ = canonical_address(chain_address)
		source = physical_ids[chain_address]
		for observation in observations:
			implementation = observation.get("implementation")
			implementation_chain_address = observation.get("implementation_chain_address_id")
			if implementation_chain_address:
				implementation_chain, implementation_address, implementation_chain_address = canonical_address(
					implementation_chain_address, address=implementation)
				if implementation_chain != chain_id:
					raise ValueError("proxy implementation must be on the same chain as its proxy")
			elif implementation:
				_, implementation_address, implementation_chain_address = canonical_address(
					chain_id=chain_id, address=implementation)
			else:
				implementation_address = None
			if implementation_address:
				target = ensure_physical_address(implementation_chain_address)
				g.edge(source, target, "proxy-implementation", semantic_source="eip-1967")
			for embedded_address in observation.get("embedded_addresses", []):
				_, _, embedded_chain_address = canonical_address(chain_id=chain_id, address=embedded_address)
				target = ensure_physical_address(embedded_chain_address)
				g.edge(source, target, "bytecode-embeds-address", semantic_source="deployed-bytecode")

	default_chain_id = payload.get("rpc_snapshot", {}).get("chain_id")
	for configuration in payload.get("runtime_configurations", []):
		configuration_chain_id = configuration.get("chain_id")
		if configuration_chain_id is None and not configuration.get("chain_address_id"):
			configuration_chain_id = default_chain_id
		chain_id, address, chain_address = canonical_address(configuration.get("chain_address_id"),
			configuration_chain_id, configuration.get("address"))
		target = ensure_physical_address(chain_address)
		g.edge(configuration["component"], target, "runtime-configures-contract",
			storage=configuration["storage"], asset_id=configuration.get("asset_id"), chain_id=chain_id,
			chain_address_id=chain_address,
			block_hash=payload.get("substrate_snapshot", {}).get("block_hash"))

	g.node("semantic-analysis:contract-deployments", "semantic-coverage", tool="deployment-artifacts",
		manifest=path.name, manifest_sha256=analysis_provenance.file_sha256(path), schema_version=2,
		contracts=len(physical_ids), aliases=len(aliases),
		records=len(payload.get("contracts", [])), address_references=len(payload.get("address_references", [])),
		runtime_configurations=len(payload.get("runtime_configurations", [])),
		asset_registry_erc20_configurations=sum(
			configuration.get("component") == "pallet:asset-registry"
			and configuration.get("storage") == "assetRegistry.assetLocations"
			and configuration.get("asset_type") == "erc20"
			for configuration in payload.get("runtime_configurations", [])),
		chain_context=payload.get("chain_context"),
		rpc_snapshot=payload.get("rpc_snapshot"), substrate_snapshot=payload.get("substrate_snapshot"),
		collection_provenance=payload.get("collection_provenance"),
		enrichment_provenance=payload.get("enrichment_provenance"),
		runtime_collection_provenance=payload.get("runtime_collection_provenance"),
		runtime_query_coverage=payload.get("runtime_collection_provenance", {}).get("query_coverage"))
	return len(physical_ids)


def bounded_paths_with_metadata(edges: list[dict], targets: set[str], max_depth: int = 5,
	limit: int = 50) -> tuple[list[list[str]], dict]:
	adj: dict[str, set[str]] = defaultdict(set)
	for edge in edges:
		adj[edge["source"]].add(edge["target"])
	starts = sorted({node for node in adj
		if node.startswith(("pallet:", "runtime:", "precompile:", "component:evm:"))}
		| {"boundary:evm-execution"})
	paths = []
	limit_truncated = []
	depth_truncated = []
	for start in starts:
		queue = deque([(start, [start])])
		found_for_start = 0
		while queue and found_for_start < limit:
			node, path = queue.popleft()
			if node in targets and len(path) > 1:
				paths.append(path)
				found_for_start += 1
				continue
			if len(path) - 1 >= max_depth:
				if adj[node]:
					depth_truncated.append(start)
				continue
			for target in sorted(adj[node]):
				if target not in path:
					queue.append((target, path + [target]))
		if queue and found_for_start >= limit:
			limit_truncated.append(start)
	metadata = {
		"max_depth": max_depth,
		"per_start_limit": limit,
		"start_count": len(starts),
		"limit_truncated_starts": sorted(set(limit_truncated)),
		"depth_truncated_starts": sorted(set(depth_truncated)),
	}
	return sorted(paths, key=lambda path: (len(path), path)), metadata


def bounded_paths(edges: list[dict], targets: set[str], max_depth: int = 5, limit: int = 50) -> list[list[str]]:
	return bounded_paths_with_metadata(edges, targets, max_depth, limit)[0]


def overview_selection(g: Graph, components: list[dict]) -> tuple[set[str], list[dict]]:
	components = [edge for edge in components if edge["kind"] != "may-resolve-to"]
	interesting = {"boundary:evm-execution", "boundary:frame-dispatch"}
	interesting.update(node["id"] for node in g.nodes.values() if node["kind"] in {"asset-kind", "asset-backend"})
	interesting.add("pallet:asset-registry")
	interesting.update(edge["target"] for edge in components
		if edge["source"] == "pallet:route-executor" and (edge.get("associated_type") or "").endswith(":AMM"))
	interesting.update(edge["source"] for edge in components
		if edge["target"] in {"boundary:evm-execution", "boundary:frame-dispatch"})
	for cycle in strongly_connected(components):
		interesting.update(cycle)
	selected = {}
	for edge in components:
		if edge["source"] in interesting and edge["target"] in interesting:
			selected.setdefault((edge["source"], edge["target"], edge["kind"]), edge)
	return interesting, list(selected.values())


def write_overview_dot(g: Graph, components: list[dict], output: Path) -> Path:
	interesting, selected = overview_selection(g, components)
	path = output / "audit-overview.dot"
	colors = {"frame": "#dbeafe", "evm": "#fee2e2", "precompile": "#fef3c7", "asset": "#dcfce7",
		"runtime": "#ede9fe", "evm-adapter": "#fce7f3"}
	with path.open("w") as f:
		f.write("digraph audit_overview {\n  rankdir=LR;\n  graph [bgcolor=\"transparent\", pad=\"0.2\"];\n")
		f.write("  node [shape=box, style=\"rounded,filled\", fontname=\"sans-serif\", fontsize=10];\n")
		for ident in sorted(interesting):
			node = g.nodes.get(ident, {})
			color = colors.get(node.get("domain"), "#f3f4f6")
			label = ident.replace("associated:pallet:", "callback:").replace("asset-backend:", "backend:")
			f.write(f'  "{ident}" [label="{label}", fillcolor="{color}"];\n')
		for edge in selected:
			f.write(f'  "{edge["source"]}" -> "{edge["target"]}" [label="{edge["kind"]}"];\n')
		f.write("}\n")
	return path


def write_overview_svg(g: Graph, components: list[dict], output: Path) -> Path:
	nodes, edges = overview_selection(g, components)
	columns: dict[str, list[str]] = defaultdict(list)
	for ident in sorted(nodes):
		node = g.nodes.get(ident, {})
		if node.get("kind") == "asset-backend":
			column = "backend"
		elif node.get("kind") == "asset-kind":
			column = "asset"
		elif ident.startswith("boundary:"):
			column = "boundary"
		elif node.get("domain") in {"precompile", "evm-adapter"}:
			column = "evm"
		else:
			column = "frame"
		columns[column].append(ident)
	x_positions = {"frame": 40, "evm": 390, "boundary": 740, "asset": 1050, "backend": 1340}
	positions = {}
	for column, identifiers in columns.items():
		for index, ident in enumerate(identifiers):
			positions[ident] = (x_positions[column], 60 + index * 48)
	height = max((y for _, y in positions.values()), default=600) + 80
	colors = {"frame": "#dbeafe", "evm": "#fee2e2", "precompile": "#fef3c7", "asset": "#dcfce7",
		"runtime": "#ede9fe", "evm-adapter": "#fce7f3"}
	path = output / "audit-overview.svg"
	with path.open("w") as f:
		f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="1660" height="{height}" viewBox="0 0 1660 {height}">\n')
		f.write('<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#64748b"/></marker></defs>\n')
		f.write('<rect width="100%" height="100%" fill="#ffffff"/><text x="40" y="28" font-family="sans-serif" font-size="18" font-weight="bold">Hydration runtime audit overview</text>\n')
		for edge in edges:
			sx, sy = positions[edge["source"]]
			tx, ty = positions[edge["target"]]
			f.write(f'<path d="M {sx + 280} {sy + 15} C {(sx + tx) / 2:.0f} {sy + 15}, {(sx + tx) / 2:.0f} {ty + 15}, {tx} {ty + 15}" fill="none" stroke="#94a3b8" stroke-width="1.2" marker-end="url(#arrow)"><title>{html.escape(edge["kind"])}</title></path>\n')
		for ident in sorted(nodes):
			x, y = positions[ident]
			node = g.nodes.get(ident, {})
			color = colors.get(node.get("domain"), "#f3f4f6")
			label = ident.replace("associated:pallet:", "callback:").replace("asset-backend:", "backend:")
			f.write(f'<g><title>{html.escape(ident)}</title><rect x="{x}" y="{y}" width="280" height="30" rx="6" fill="{color}" stroke="#64748b"/><text x="{x + 8}" y="{y + 20}" font-family="monospace" font-size="11">{html.escape(label)}</text></g>\n')
		f.write("</svg>\n")
	return path


def graph_scale_summary(g: Graph) -> dict[str, object]:
	nodes = list(g.nodes.values())
	edges = list(g.edges)

	def node_activity(node: dict) -> str:
		if node.get("runtime_active") is False:
			return "inactive"
		for relation in ("owner", "function"):
			dependency = node.get(relation)
			if isinstance(dependency, str) and g.nodes.get(dependency, {}).get("runtime_active") is False:
				return "inactive"
		return "active" if node.get("runtime_active") is True else "unclassified"

	node_activity_by_id = {node["id"]: node_activity(node) for node in nodes}

	def edge_activity(edge: dict) -> str:
		statuses = {node_activity_by_id[edge[endpoint]] for endpoint in ("source", "target")}
		if "inactive" in statuses:
			return "inactive"
		return "active" if statuses == {"active"} else "unclassified"

	edge_activity_by_id = {id(edge): edge_activity(edge) for edge in edges}

	def activity_counts(items: list[dict], statuses: dict, identity) -> dict[str, int]:
		counts = Counter(statuses[identity(item)] for item in items)
		return {"raw": len(items), "operational": counts["active"] + counts["unclassified"],
			"active": counts["active"], "unclassified": counts["unclassified"],
			"inactive": counts["inactive"]}

	def distribution(items: list[dict], statuses: dict, identity, key) -> list[dict]:
		raw = Counter(str(key(item) or "unclassified") for item in items)
		activity = {status: Counter(str(key(item) or "unclassified") for item in items
			if statuses[identity(item)] == status) for status in ("active", "unclassified", "inactive")}
		return [{"name": name, "raw": raw[name],
			"operational": activity["active"][name] + activity["unclassified"][name],
			"active": activity["active"][name], "unclassified": activity["unclassified"][name],
			"inactive": activity["inactive"][name]}
			for name in sorted(raw, key=lambda name: (-raw[name], name))]

	component_nodes = [node for node in nodes if node["kind"] in COMPONENT_NODE_KINDS]
	entrypoints = [node for node in nodes if node["kind"] == "entrypoint"]
	inventory_targets = [node for node in nodes if node.get("runtime_active") is False
		and (node.get("kind") == "associated-type" or "associated-type" in (node.get("roles") or []))]

	def evidence(edge: dict) -> str:
		kind = edge["kind"]
		source = edge.get("semantic_source")
		if source == "rustc-mir" or kind.startswith("mir-"):
			return "rustc MIR"
		if source == "rapx" or kind == "rapx-call":
			return "RAPx"
		if source == "explicit-inventory":
			return "semantic inventory"
		deployment_kinds = {
			"uses-deployed-contract", "runtime-configures-contract", "proxy-implementation",
			"bytecode-embeds-address", "deployment-aliases-contract", "deployment-step-produces-alias",
			"deployment-step-references-address", "selector-matches-contract-function",
		}
		deployment_nodes = {"deployed-contract", "deployment-alias", "deployment-step",
			"deployment-address-reference", "contract-function"}
		if source in {"deployment-artifact", "eip-1967", "deployed-bytecode"} or kind in deployment_kinds \
			or any(g.nodes.get(edge[endpoint], {}).get("kind") in deployment_nodes
				for endpoint in ("source", "target")):
			return "deployment / chain"
		return "source scan"

	evidence_order = ["source scan", "rustc MIR", "RAPx", "semantic inventory", "deployment / chain"]
	evidence_raw = Counter(evidence(edge) for edge in edges)
	evidence_activity = {status: Counter(evidence(edge) for edge in edges
		if edge_activity_by_id[id(edge)] == status) for status in ("active", "unclassified", "inactive")}

	def effective_domain(node: dict) -> str:
		if node.get("domain"):
			return str(node["domain"])
		for relation in ("owner", "function"):
			related = g.nodes.get(node.get(relation), {})
			if related.get("domain"):
				return str(related["domain"])
		return "unclassified"

	return {
		"totals": {
			"nodes": activity_counts(nodes, node_activity_by_id, lambda node: node["id"]),
			"edges": activity_counts(edges, edge_activity_by_id, id),
			"components": activity_counts(component_nodes, node_activity_by_id, lambda node: node["id"]),
			"entrypoints": activity_counts(entrypoints, node_activity_by_id, lambda node: node["id"]),
		},
		"inventory_only_targets": len(inventory_targets),
		"unresolved_targets": sum(bool(node.get("unresolved")) for node in nodes),
		"domains": distribution(nodes, node_activity_by_id, lambda node: node["id"], effective_domain),
		"node_kinds": distribution(nodes, node_activity_by_id, lambda node: node["id"],
			lambda node: node["kind"]),
		"edge_kinds": distribution(edges, edge_activity_by_id, id, lambda edge: edge["kind"]),
		"evidence": [{"name": name, "raw": evidence_raw[name],
			"operational": evidence_activity["active"][name] + evidence_activity["unclassified"][name],
			"active": evidence_activity["active"][name],
			"unclassified": evidence_activity["unclassified"][name],
			"inactive": evidence_activity["inactive"][name]}
			for name in evidence_order],
	}


def write_graph_scale_svg(g: Graph, output: Path) -> Path:
	summary = graph_scale_summary(g)
	totals = summary["totals"]
	width, height = 1600, 1200
	path = output / "graph-scale.svg"
	lines = [
		f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
		f'viewBox="0 0 {width} {height}" role="img" aria-labelledby="title" aria-describedby="description">',
		'<title id="title">Hydration runtime interaction graph scale</title>',
		'<desc id="description">Raw graph totals with explicit active, unclassified, inactive, operational, and evidence provenance counts.</desc>',
		'''<defs>
			<filter id="shadow" x="-10%" y="-10%" width="120%" height="130%"><feDropShadow dx="0" dy="5" stdDeviation="8" flood-color="#172033" flood-opacity=".08"/></filter>
		</defs>
		<style>
			.title{font:700 32px Inter,system-ui,sans-serif;fill:#172033}.subtitle{font:15px Inter,system-ui,sans-serif;fill:#475569}
			.eyebrow{font:700 11px Inter,system-ui,sans-serif;letter-spacing:1.5px;fill:#64748b}.metric{font:700 36px Inter,system-ui,sans-serif;fill:#172033}
			.metric-detail{font:12px Inter,system-ui,sans-serif;fill:#475569}.panel-title{font:700 17px Inter,system-ui,sans-serif;fill:#172033}
			.label{font:13px Inter,system-ui,sans-serif;fill:#334155}.count{font:12px ui-monospace,SFMono-Regular,monospace;fill:#475569}
			.chip{font:700 12px Inter,system-ui,sans-serif;fill:#334155}.legend{font:12px Inter,system-ui,sans-serif;fill:#475569}
		</style>''',
		'<rect width="1600" height="1200" fill="#f5f7fb"/>',
		'<text class="title" x="48" y="53">Hydration runtime interaction graph</text>',
		'<text class="subtitle" x="48" y="82">Executive scale overview · raw inventory, operational records, and explicit activity evidence</text>',
	]

	chips = [("inventory-only targets", summary["inventory_only_targets"], 1050, 190),
		("unresolved targets", summary["unresolved_targets"], 1260, 190)]
	for label, value, x, chip_width in chips:
		lines.extend([
			f'<rect x="{x}" y="37" width="{chip_width}" height="38" rx="19" fill="#ffffff" stroke="#dbe2ee"/>',
			f'<text class="chip" x="{x + 16}" y="61">{html.escape(label)} · {value:,}</text>',
		])

	card_colors = ["#6366f1", "#0ea5e9", "#22c55e", "#f59e0b"]
	for index, key in enumerate(("nodes", "edges", "components", "entrypoints")):
		metric = totals[key]
		x = 48 + index * 380
		percentage = (metric["operational"] / metric["raw"] * 100) if metric["raw"] else 0
		lines.extend([
			f'<g filter="url(#shadow)"><rect x="{x}" y="110" width="364" height="120" rx="18" fill="#ffffff"/>',
			f'<rect x="{x}" y="110" width="364" height="5" rx="2.5" fill="{card_colors[index]}"/></g>',
			f'<text class="eyebrow" x="{x + 22}" y="143">RAW {key.upper()}</text>',
			f'<text class="metric" x="{x + 22}" y="181">{metric["raw"]:,}</text>',
			f'<text class="metric-detail" x="{x + 22}" y="204">{metric["operational"]:,} operational · {percentage:.1f}%</text>',
			f'<text class="metric-detail" x="{x + 22}" y="221">{metric["active"]:,} active · {metric["unclassified"]:,} unclassified</text>',
		])

	lines.extend([
		'<g filter="url(#shadow)"><rect x="48" y="255" width="1504" height="185" rx="18" fill="#ffffff"/></g>',
		'<text class="panel-title" x="72" y="290">Runtime activity</text>',
		'<rect x="1110" y="275" width="12" height="12" rx="3" fill="#4f46e5"/><text class="legend" x="1130" y="286">explicit active</text>',
		'<rect x="1245" y="275" width="12" height="12" rx="3" fill="#0ea5e9"/><text class="legend" x="1265" y="286">unclassified</text>',
		'<rect x="1380" y="275" width="12" height="12" rx="3" fill="#cbd5e1"/><text class="legend" x="1400" y="286">inactive</text>',
	])
	for index, key in enumerate(("nodes", "edges")):
		metric = totals[key]
		y = 326 + index * 60
		bar_width = 1210
		active_width = bar_width * metric["active"] / metric["raw"] if metric["raw"] else 0
		operational_width = bar_width * metric["operational"] / metric["raw"] if metric["raw"] else 0
		lines.extend([
			f'<text class="label" x="72" y="{y + 18}">{key}</text>',
			f'<rect x="174" y="{y}" width="{bar_width}" height="24" rx="12" fill="#cbd5e1"/>',
			f'<rect x="174" y="{y}" width="{operational_width:.2f}" height="24" rx="12" fill="#0ea5e9"/>',
			f'<rect x="174" y="{y}" width="{active_width:.2f}" height="24" rx="12" fill="#4f46e5"/>',
			f'<text class="count" text-anchor="end" x="1528" y="{y + 18}">{metric["operational"]:,} / {metric["raw"]:,} operational</text>',
		])

	def compact(items: list[dict], limit: int = 8) -> list[dict]:
		if len(items) <= limit:
			return items
		return items[:limit - 1] + [{"name": f"other ({len(items) - limit + 1} more)",
			"raw": sum(item["raw"] for item in items[limit - 1:]),
			"operational": sum(item["operational"] for item in items[limit - 1:]),
			"active": sum(item["active"] for item in items[limit - 1:]),
			"unclassified": sum(item["unclassified"] for item in items[limit - 1:]),
			"inactive": sum(item["inactive"] for item in items[limit - 1:])}]

	def display_label(value: str) -> str:
		value = value.replace("_", " ")
		return value if len(value) <= 25 else value[:24] + "…"

	panels = [("Top domains", summary["domains"]), ("Top node kinds", summary["node_kinds"]),
		("Top edge kinds", summary["edge_kinds"])]
	for panel_index, (title, items) in enumerate(panels):
		x = 48 + panel_index * 504
		panel_items = compact(items)
		maximum = max((item["raw"] for item in panel_items), default=1)
		lines.extend([
			f'<g filter="url(#shadow)"><rect x="{x}" y="465" width="480" height="440" rx="18" fill="#ffffff"/></g>',
			f'<text class="panel-title" x="{x + 20}" y="500">{title}</text>',
			f'<text class="legend" text-anchor="end" x="{x + 458}" y="500">operational / raw</text>',
		])
		for row, item in enumerate(panel_items):
			y = 526 + row * 45
			raw_width = 235 * item["raw"] / maximum
			active_width = 235 * item["active"] / maximum
			operational_width = 235 * item["operational"] / maximum
			label = display_label(item["name"])
			lines.extend([
				f'<g><title>{html.escape(item["name"])}</title><text class="label" x="{x + 20}" y="{y + 15}">{html.escape(label)}</text>',
				f'<text class="count" text-anchor="end" x="{x + 458}" y="{y + 15}">{item["operational"]:,} / {item["raw"]:,}</text>',
				f'<rect x="{x + 20}" y="{y + 24}" width="235" height="8" rx="4" fill="#edf1f7"/>',
				f'<rect x="{x + 20}" y="{y + 24}" width="{raw_width:.2f}" height="8" rx="4" fill="#cbd5e1"/>',
				f'<rect x="{x + 20}" y="{y + 24}" width="{operational_width:.2f}" height="8" rx="4" fill="#0ea5e9"/>',
				f'<rect x="{x + 20}" y="{y + 24}" width="{active_width:.2f}" height="8" rx="4" fill="#4f46e5"/>',
				'</g>',
			])

	evidence = summary["evidence"]
	evidence_total = sum(item["raw"] for item in evidence)
	evidence_colors = ["#6366f1", "#0ea5e9", "#a855f7", "#f59e0b", "#22c55e"]
	lines.extend([
		'<g filter="url(#shadow)"><rect x="48" y="930" width="1504" height="210" rx="18" fill="#ffffff"/></g>',
		'<text class="panel-title" x="72" y="967">Edge evidence provenance</text>',
		'<text class="legend" x="72" y="990">mutually exclusive evidence classes · segment width is proportional to raw edge count</text>',
		'<rect x="72" y="1012" width="1456" height="28" rx="14" fill="#edf1f7"/>',
	])
	offset = 72.0
	for color, item in zip(evidence_colors, evidence):
		segment_width = 1456 * item["raw"] / evidence_total if evidence_total else 0
		lines.append(f'<rect x="{offset:.2f}" y="1012" width="{segment_width:.2f}" height="28" fill="{color}"/>')
		offset += segment_width
	for index, (color, item) in enumerate(zip(evidence_colors, evidence)):
		x = 76 + index * 292
		lines.extend([
			f'<rect x="{x}" y="1063" width="12" height="12" rx="3" fill="{color}"/>',
			f'<text class="label" x="{x + 20}" y="1074">{html.escape(item["name"])}</text>',
			f'<text class="count" x="{x + 20}" y="1097">{item["raw"]:,} raw · {item["operational"]:,} operational</text>',
			f'<text class="count" x="{x + 20}" y="1117">{item["active"]:,} active · {item["unclassified"]:,} unclassified</text>',
		])
	lines.extend([
		'<text class="legend" x="48" y="1174">Deterministic aggregate view · exact records and evidence remain in interaction-graph.json</text>',
		'</svg>',
	])
	path.write_text("\n".join(lines) + "\n")
	return path


def write_focus_svg(nodes: list[str], path: Path, title: str, cyclic: bool = False) -> None:
	width = max(700, len(nodes) * 260)
	with path.open("w") as f:
		f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="150" viewBox="0 0 {width} 150">\n')
		f.write('<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#64748b"/></marker></defs><rect width="100%" height="100%" fill="#fff"/>\n')
		f.write(f'<text x="24" y="26" font-family="sans-serif" font-size="16" font-weight="bold">{html.escape(title)}</text>\n')
		for index, node in enumerate(nodes):
			x = 24 + index * 250
			f.write(f'<rect x="{x}" y="60" width="210" height="34" rx="6" fill="#e2e8f0" stroke="#64748b"/><text x="{x + 8}" y="81" font-family="monospace" font-size="10">{html.escape(node)}</text>\n')
			if index + 1 < len(nodes):
				f.write(f'<line x1="{x + 210}" y1="77" x2="{x + 246}" y2="77" stroke="#64748b" marker-end="url(#arrow)"/>\n')
		if cyclic and len(nodes) > 1:
			end = 24 + (len(nodes) - 1) * 250
			f.write(f'<path d="M {end + 105} 96 C {end + 105} 135, 129 135, 129 96" fill="none" stroke="#dc2626" marker-end="url(#arrow)"/>\n')
		f.write("</svg>\n")


def write_router_svg(components: list[dict], path: Path) -> None:
	targets = sorted({edge["target"] for edge in components
		if edge["source"] == "pallet:route-executor" and (edge.get("associated_type") or "").endswith(":AMM")
		and edge["target"].startswith(("pallet:", "component:evm:"))})
	height = max(300, 80 + len(targets) * 52)
	with path.open("w") as f:
		f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="760" height="{height}" viewBox="0 0 760 {height}">\n')
		f.write('<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#64748b"/></marker></defs><rect width="100%" height="100%" fill="#fff"/>\n')
		f.write('<text x="24" y="28" font-family="sans-serif" font-size="16" font-weight="bold">Router AMM backends</text>\n')
		f.write(f'<rect x="30" y="{height / 2 - 18}" width="230" height="36" rx="6" fill="#dbeafe" stroke="#64748b"/><text x="40" y="{height / 2 + 5}" font-family="monospace" font-size="11">pallet:route-executor</text>\n')
		for index, target in enumerate(targets):
			y = 55 + index * 52
			color = "#fce7f3" if target.startswith("component:evm:") else "#dcfce7"
			f.write(f'<path d="M 260 {height / 2} C 350 {height / 2}, 350 {y + 18}, 455 {y + 18}" fill="none" stroke="#64748b" marker-end="url(#arrow)"/>\n')
			f.write(f'<rect x="460" y="{y}" width="270" height="36" rx="6" fill="{color}" stroke="#64748b"/><text x="470" y="{y + 23}" font-family="monospace" font-size="11">{html.escape(target)}</text>\n')
		f.write("</svg>\n")


def write_layer_svg(g: Graph, edges: list[dict], path: Path, title: str) -> None:
	nodes = sorted({edge[key] for edge in edges for key in ("source", "target")})
	columns: dict[str, list[str]] = defaultdict(list)
	for ident in nodes:
		node = g.nodes.get(ident, {})
		if ident.startswith("asset-kind:"):
			column = "kind"
		elif ident.startswith("asset-backend:"):
			column = "backend"
		elif ident.startswith("boundary:"):
			column = "boundary"
		elif ident.startswith("precompile:") or node.get("domain") in {"evm-adapter", "evm-contract"}:
			column = "evm"
		else:
			column = "frame"
		columns[column].append(ident)
	order = [column for column in ("frame", "evm", "boundary", "kind", "backend") if columns[column]]
	x_positions = {column: 30 + index * 330 for index, column in enumerate(order)}
	positions = {(ident): (x_positions[column], 60 + index * 48)
		for column in order for index, ident in enumerate(columns[column])}
	width = max(720, len(order) * 330 + 30)
	height = max((y for _, y in positions.values()), default=60) + 70
	with path.open("w") as f:
		f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">\n')
		f.write('<defs><marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#64748b"/></marker></defs><rect width="100%" height="100%" fill="#fff"/>\n')
		f.write(f'<text x="24" y="28" font-family="sans-serif" font-size="16" font-weight="bold">{html.escape(title)}</text>\n')
		for edge in edges:
			sx, sy = positions[edge["source"]]
			tx, ty = positions[edge["target"]]
			f.write(f'<path d="M {sx + 280} {sy + 15} C {(sx + tx + 280) / 2:.0f} {sy + 15}, {(sx + tx + 280) / 2:.0f} {ty + 15}, {tx} {ty + 15}" fill="none" stroke="#94a3b8" marker-end="url(#arrow)"><title>{html.escape(edge["kind"])}</title></path>\n')
		for ident in nodes:
			x, y = positions[ident]
			f.write(f'<rect x="{x}" y="{y}" width="280" height="30" rx="6" fill="#e2e8f0" stroke="#64748b"/><text x="{x + 8}" y="{y + 20}" font-family="monospace" font-size="10">{html.escape(ident)}</text>\n')
		f.write("</svg>\n")


def interactive_graph_payload(g: Graph, components: list[dict]) -> dict:
	projection_names = tuple(EDGE_PROJECTIONS)
	projection_stats = {name: {"nodes": set(), "pairs": set(), "evidence_edges": 0,
		"kinds": Counter()} for name in projection_names}
	pairs = {}
	node_projections: dict[str, set[str]] = defaultdict(set)
	sample_fields = ("kind", "file", "line", "method", "semantic_source", "resolution",
		"address", "selector", "semantics", "enforcement", "evidence_source", "evidence_target",
		"semantic_evidence")
	for edge in components:
		projections = tuple(name for name in projection_names if edge["kind"] in EDGE_PROJECTIONS[name])
		if not projections:
			continue
		key = (edge["source"], edge["target"])
		pair = pairs.setdefault(key, {"source": edge["source"], "target": edge["target"],
			"count": 0, "kind_counts": Counter(), "projection_counts": Counter(), "samples": {}})
		pair["count"] += 1
		pair["kind_counts"][edge["kind"]] += 1
		if edge["kind"] not in pair["samples"]:
			pair["samples"][edge["kind"]] = {field: edge[field] for field in sample_fields if field in edge}
		for projection in projections:
			pair["projection_counts"][projection] += 1
			stats = projection_stats[projection]
			stats["nodes"].update(key)
			stats["pairs"].add(key)
			stats["evidence_edges"] += 1
			stats["kinds"][edge["kind"]] += 1
			node_projections[edge["source"]].add(projection)
			node_projections[edge["target"]].add(projection)

	def activity(node: dict) -> str:
		if node.get("runtime_active") is False:
			return "inactive"
		for field in ("owner", "function"):
			dependency = node.get(field)
			if isinstance(dependency, str) and g.nodes.get(dependency, {}).get("runtime_active") is False:
				return "inactive"
		return "active" if node.get("runtime_active") is True else "unclassified"

	compact_nodes = []
	for ident in sorted(node_projections):
		node = g.nodes[ident]
		label = node.get("semantic_label") or node.get("name") or node.get("runtime_alias")
		if not label and isinstance(node.get("names"), list) and node["names"]:
			label = node["names"][0]
		compact = {"id": ident, "label": label or ident, "kind": node.get("kind", "unknown"),
			"domain": node.get("domain", "unspecified"), "activity": activity(node),
			"projections": sorted(node_projections[ident])}
		for field in ("description", "semantic_description", "file", "source", "configured_by",
			"address", "chain_address_id", "network", "runtime_alias", "entrypoint_kind"):
			if field in node:
				compact[field] = node[field]
		for field in ("roles", "names", "semantic_domains"):
			if isinstance(node.get(field), list):
				compact[field] = node[field][:20]
		compact_nodes.append(compact)

	compact_edges = []
	for key in sorted(pairs):
		pair = pairs[key]
		compact_edges.append({"source": pair["source"], "target": pair["target"],
			"count": pair["count"], "kind_counts": dict(sorted(pair["kind_counts"].items())),
			"projection_counts": dict(sorted(pair["projection_counts"].items())),
			"samples": [pair["samples"][kind] for kind in sorted(pair["samples"])[:12]]})

	scale = graph_scale_summary(g)
	return {"schema_version": 1,
		"summary": {"nodes": scale["totals"]["nodes"], "edges": scale["totals"]["edges"],
			"components": scale["totals"]["components"], "entrypoints": scale["totals"]["entrypoints"],
			"inventory_only_targets": scale["inventory_only_targets"],
			"unresolved_targets": scale["unresolved_targets"]},
		"projections": {name: {"nodes": len(stats["nodes"]), "pairs": len(stats["pairs"]),
			"evidence_edges": stats["evidence_edges"], "kinds": sorted(stats["kinds"])}
			for name, stats in projection_stats.items()},
		"nodes": compact_nodes, "edges": compact_edges}


def write_interactive_html(g: Graph, components: list[dict], output: Path) -> None:
	payload = json.dumps(interactive_graph_payload(g, components), sort_keys=True,
		separators=(",", ":"), ensure_ascii=True)
	payload = payload.replace("&", "\\u0026").replace("<", "\\u003c").replace(">", "\\u003e")
	style = Path(__file__).with_name("graph_explorer.css").read_text()
	script = Path(__file__).with_name("graph_explorer.js").read_text()
	template = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Hydration runtime interaction explorer</title><style>__GRAPH_STYLE__</style></head>
<body><header class="topbar"><div><p class="eyebrow">Hydration runtime</p><h1>Interaction explorer</h1>
<p class="subtitle">Aggregated, evidence-backed relationships. Click any node or edge to inspect it.</p></div>
<div class="headline-stats" aria-label="full graph scale"><div><strong id="full-node-count">—</strong><span>full nodes</span></div>
<div><strong id="full-edge-count">—</strong><span>full edges</span></div><div><strong id="component-count">—</strong><span>components</span></div></div>
<nav><a href="graph-scale.svg">scale overview</a><a href="interaction-graph.json">raw graph</a><a href="api/v1">API</a></nav></header>
<main class="app"><section class="workspace" aria-label="interactive topology">
<div class="toolbar"><label>projection<select id="projection"></select></label><label>domain<select id="domain"><option value="">all domains</option></select></label>
<label>edge kind<select id="edge-kind"><option value="">all kinds</option></select></label>
<label class="search-field">find node<input id="search" type="search" autocomplete="off" placeholder="omnipool, precompile, contract…" aria-controls="search-results"><span id="search-results" class="search-results"></span></label>
<button id="fit" type="button">fit graph</button><button id="reset-selection" type="button">clear selection</button>
<label class="check"><input id="labels" type="checkbox">more labels</label></div>
<div class="projection-strip"><span id="projection-note"></span><strong id="relation-count"></strong></div>
<div class="canvas-shell"><canvas id="graph-canvas" tabindex="0" role="img" aria-label="Interactive directed component graph. Use search or the inspector for an accessible node list."></canvas>
<div id="legend" class="legend" aria-label="domain legend"></div><div class="canvas-hint">drag to pan · wheel to zoom · click to inspect</div></div>
<p id="live-status" class="sr-only" aria-live="polite"></p></section>
<aside id="inspector" class="inspector" aria-label="selection details"><div class="empty-inspector"><span>↗</span><h2>Select a node or relationship</h2><p>Neighbors, edge kinds, evidence samples, and bounded API metadata appear here.</p></div></aside></main>
<noscript>This visualization requires JavaScript. The raw graph remains available through <a href="interaction-graph.json">interaction-graph.json</a>.</noscript>
<script>const payload=__GRAPH_PAYLOAD__;</script><script>__GRAPH_SCRIPT__</script></body></html>
"""
	style_prefix, remainder = template.split("__GRAPH_STYLE__", 1)
	payload_prefix, remainder = remainder.split("__GRAPH_PAYLOAD__", 1)
	script_prefix, suffix = remainder.split("__GRAPH_SCRIPT__", 1)
	html_text = style_prefix + style + payload_prefix + payload + script_prefix + script + suffix
	(output / "interaction-graph.html").write_text(html_text)


def query_packs(g: Graph, components: list[dict], projections: dict[str, list[dict]],
	cycles: list[list[str]], paths: list[list[str]], path_search: dict, dangerous: list[dict]) -> dict:
	operational = {name: [edge for edge in edges if edge_runtime_active(g, edge)]
		for name, edges in projections.items()}
	adj: dict[str, set[str]] = defaultdict(set)
	for edge in operational["execution"]:
		adj[edge["source"]].add(edge["target"])
	traces = []
	trace_limit = 10
	trace_depth = 8
	trace_limit_truncated = []
	trace_depth_truncated = []
	starts = sorted(node["id"] for node in g.nodes.values() if node["kind"] == "entrypoint")
	for start in starts:
		queue = deque([(start, [start])])
		found_for_start = 0
		while queue and found_for_start < trace_limit:
			node, trace = queue.popleft()
			if len(trace) > 1 and node.startswith("boundary:"):
				traces.append(trace)
				found_for_start += 1
				continue
			if len(trace) >= trace_depth:
				if adj[node]:
					trace_depth_truncated.append(start)
				continue
			for target in sorted(adj[node]):
				if target not in trace:
					queue.append((target, trace + [target]))
		if queue and found_for_start >= trace_limit:
			trace_limit_truncated.append(start)
	return {
		"schema_version": 1,
		"component_cycles": cycles,
		"cross_domain_paths": paths,
		"path_search": path_search,
		"state_external_ordering": [item for item in dangerous if "state-write" in item["rule"]
			and (not item.get("function")
				or node_runtime_active(g, g.nodes.get(item["function"], {})))],
		"privileged_entries": [edge for edge in operational["authorization"]
			if edge["kind"] == "authorizes-entry"],
		"lifecycle_entrypoints": [node for node in g.nodes.values()
			if node.get("entrypoint_kind") in {"runtime-hook", "runtime-callback"}
			and node_runtime_active(g, node)],
		"token_backend_edges": operational["asset"],
		"runtime_contract_edges": operational["deployment"],
		"state_invariant_edges": [edge for edge in operational["state"] if edge["kind"] in {
			"affects-invariant", "enforces-invariant", "guarded-by-invariant", "enforces", "guards",
			"must-equal"}],
		"prioritized_test_gaps": prioritize_gaps(g, components),
		"entrypoint_execution_traces": traces,
		"entrypoint_trace_search": {"max_depth": trace_depth, "per_entrypoint_limit": trace_limit,
			"limit_truncated_entrypoints": sorted(set(trace_limit_truncated)),
			"depth_truncated_entrypoints": sorted(set(trace_depth_truncated))},
	}


def dangerous_interactions(g: Graph, cycles: list[list[str]], paths: list[list[str]]) -> list[dict]:
	result = []
	for path in paths:
		if "boundary:evm-execution" in path and "boundary:frame-dispatch" in path:
			result.append({"rule": "evm-precompile-frame-dispatch", "severity": "high", "path": path})
		if path[0] == "pallet:route-executor" and "boundary:evm-execution" in path and len(path) <= 4:
			result.append({"rule": "polymorphic-router-reaches-evm", "severity": "high", "path": path})
	for cycle in cycles:
		if any("evm" in node or "precompile" in node for node in cycle):
			result.append({"rule": "cross-domain-component-cycle", "severity": "high", "path": cycle})
	for node in g.nodes.values():
		if node["kind"] != "function":
			continue
		if node.get("mir_write_before_external") and node.get("mir_write_after_external"):
			result.append({"rule": "mir-state-write-around-external-call", "severity": "high",
				"function": node["id"], "operations": node["mir_operations"]})
			continue
		operations = node.get("operations", [])
		calls = [index for index, operation in enumerate(operations) if operation["kind"] == "external-call"]
		writes = [index for index, operation in enumerate(operations) if operation["kind"] == "storage-write"]
		if calls and any(write < calls[0] for write in writes) and any(write > calls[0] for write in writes):
			result.append({"rule": "state-write-around-external-call", "severity": "high",
				"function": node["id"], "operations": operations})
		if node.get("lifecycle") and calls and not node.get("transactional"):
			result.append({"rule": "lifecycle-external-call-without-explicit-transaction", "severity": "review",
				"function": node["id"], "operations": operations})
	return result


def historical_property_status(g: Graph, components: list[dict]) -> dict[str, bool]:
	edges = {(edge["source"], edge["target"], edge["kind"]) for edge in components}
	router_targets = {edge["target"] for edge in components
		if edge["source"] == "pallet:route-executor" and edge["kind"] == "resolved-call"}
	call_permit = [node for node in g.nodes.values() if node["kind"] == "audit-candidate"
		and "precompiles/call-permit/src/lib.rs" in node["id"]]
	return {
		"call-permit-nonce-write-before-subcall": len(call_permit) == 1
			and bool(call_permit[0].get("storage_before")) and not call_permit[0].get("storage_after"),
		"router-resolves-all-amm-backends": {
			"pallet:omnipool", "pallet:stableswap", "pallet:xyk", "pallet:lbp", "pallet:hsm",
			"component:evm:aave_trade_executor",
		}.issubset(router_targets),
		"asset-kinds-cover-native-tokens-pool-shares-and-erc20": {
			("pallet:balances", "asset-backend:balances", "uses-asset-backend"),
			("pallet:orml-tokens", "asset-backend:tokens", "uses-asset-backend"),
			("component:evm:erc20_currency", "asset-backend:erc20", "uses-asset-backend"),
			("pallet:stableswap", "asset-kind:StableSwap", "issues-asset-kind"),
		}.issubset(edges),
		"xcm-connects-message-and-asset-boundaries": {
			("component:xcm:router", "boundary:xcm-outbound", "sends-xcm"),
			("boundary:xcm-inbound", "component:xcm:executor", "receives-xcm"),
			("component:xcm:asset-transactor", "asset-backend:xcm", "uses-asset-backend"),
		}.issubset(edges),
	}


def resolved_component_edges(g: Graph, edges: list[dict]) -> list[dict]:
	result = []
	for edge in edges:
		if edge["kind"] == "dynamic-call" and not g.nodes.get(edge["target"], {}).get("unresolved", True):
			continue
		if edge["kind"] == "binding-resolves-to" and not g.nodes.get(edge["source"], {}).get("unresolved", True):
			continue
		result.append(edge)
	return result


def strongly_connected(edges: list[dict]) -> list[list[str]]:
	adj: dict[str, set[str]] = defaultdict(set)
	for edge in edges:
		adj[edge["source"]].add(edge["target"])
	index = 0
	stack: list[str] = []
	on_stack: set[str] = set()
	indices: dict[str, int] = {}
	low: dict[str, int] = {}
	groups: list[list[str]] = []

	def visit(node: str) -> None:
		nonlocal index
		indices[node] = low[node] = index
		index += 1
		stack.append(node)
		on_stack.add(node)
		for target in adj[node]:
			if target not in indices:
				visit(target)
				low[node] = min(low[node], low[target])
			elif target in on_stack:
				low[node] = min(low[node], indices[target])
		if low[node] == indices[node]:
			group = []
			while True:
				item = stack.pop()
				on_stack.remove(item)
				group.append(item)
				if item == node:
					break
			if len(group) > 1:
				groups.append(sorted(group))

	for node in sorted(adj):
		if node not in indices:
			visit(node)
	return sorted(groups, key=lambda x: (-len(x), x))


def runtime_aliases(text: str) -> dict[str, str]:
	aliases = {}
	for alias, crate in RUNTIME_ALIAS.findall(text):
		aliases[alias] = crate.split("::")[0]
	return aliases


def scan(root: Path) -> Graph:
	g = Graph()
	asset_backends = {
		"balances": "Native pallet_balances currency",
		"tokens": "ORML Tokens currency",
		"erc20": "Arbitrary EVM ERC-20 contract",
		"mapped-erc20": "ERC-20 facade over a native asset",
		"xcm": "XCM-minted or reserve-backed asset",
		"pool-share": "AMM liquidity share asset",
		"protocol-minted": "Protocol-controlled synthetic asset",
		"unknown": "Dynamically selected asset backend",
	}
	for backend, description in asset_backends.items():
		g.node(f"asset-backend:{backend}", "asset-backend", domain="asset", description=description)
	asset_kinds = {
		"Token": ("tokens", "Asset registry token; execution is selected by the runtime currency adapter"),
		"XYK": ("pool-share", "XYK liquidity share registered by AssetRegistry"),
		"StableSwap": ("pool-share", "StableSwap liquidity share registered by AssetRegistry"),
		"Bond": ("protocol-minted", "Bond asset registered by AssetRegistry"),
		"External": ("xcm", "Externally reserved or XCM-controlled asset"),
		"Erc20": ("erc20", "Bound external ERC-20 contract"),
	}
	for kind, (backend, description) in asset_kinds.items():
		ident = f"asset-kind:{kind}"
		g.node(ident, "asset-kind", domain="asset", description=description,
			source="pallets/asset-registry/src/types.rs")
		g.edge("pallet:asset-registry", ident, "defines-asset-kind",
			file="pallets/asset-registry/src/types.rs")
		g.edge(ident, f"asset-backend:{backend}", "asset-kind-resolves-to",
			file="pallets/asset-registry/src/types.rs")
	for kind in ("Token", "XYK", "StableSwap", "Bond"):
		g.edge(f"asset-kind:{kind}", "asset-backend:mapped-erc20", "exposed-as",
			file="traits/src/evm.rs", evidence="Erc20Encoding/Erc20Mapping")
	g.edge("asset-kind:Token", "asset-backend:balances", "may-use-native-backend",
		file="runtime/hydradx/src/assets.rs")
	g.edge("asset-kind:Token", "asset-backend:protocol-minted", "may-be-protocol-controlled",
		file="runtime/hydradx/src/assets.rs")
	asset_components = {
		"pallet:balances": ("asset-backend:balances",),
		"pallet:orml-tokens": ("asset-backend:tokens",),
		"pallet:currencies": ("asset-backend:balances", "asset-backend:tokens"),
		"component:evm:erc20_currency": ("asset-backend:erc20",),
		"precompile:runtime:erc20_mapping": ("asset-backend:mapped-erc20",),
	}
	for component, backends in asset_components.items():
		g.node(component, "asset-component", domain="asset")
		for backend in backends:
			g.edge(component, backend, "uses-asset-backend", file="runtime/hydradx/src/assets.rs")
	for amm, share_kind in (("pallet:xyk", "XYK"), ("pallet:stableswap", "StableSwap")):
		g.edge(amm, f"asset-kind:{share_kind}", "issues-asset-kind")
	g.node("boundary:evm-execution", "execution-boundary", domain="evm",
		description="Native runtime entering EVM execution or a precompile making an EVM subcall")
	g.node("boundary:frame-dispatch", "execution-boundary", domain="frame",
		description="EVM/precompile execution dispatching a FRAME runtime call")
	g.node("boundary:xcm-outbound", "execution-boundary", domain="xcm",
		description="Runtime sending an XCM message to another consensus system")
	g.node("boundary:xcm-inbound", "execution-boundary", domain="xcm",
		description="Message queue delivering XCM for local execution")
	g.node("component:xcm:executor", "xcm-component", domain="xcm")
	g.node("component:xcm:router", "xcm-component", domain="xcm")
	g.node("component:xcm:asset-transactor", "xcm-component", domain="xcm")
	g.edge("component:xcm:router", "boundary:xcm-outbound", "sends-xcm", file="runtime/hydradx/src/xcm.rs")
	g.edge("boundary:xcm-inbound", "component:xcm:executor", "receives-xcm", file="runtime/hydradx/src/xcm.rs")
	g.edge("component:xcm:executor", "component:xcm:asset-transactor", "moves-xcm-asset",
		file="runtime/hydradx/src/xcm.rs")
	g.edge("component:xcm:asset-transactor", "asset-backend:xcm", "uses-asset-backend",
		file="runtime/hydradx/src/xcm.rs")
	precompile_config = "runtime/hydradx/src/evm/precompiles/mod.rs"
	precompile_text = (root / precompile_config).read_text()
	precompile_ids = {
		"callpermit": "precompile:call-permit",
		"flash-loan-receiver": "precompile:flash-loan",
		"lock-manager": "precompile:lock-manager",
		"dispatch-addr": "precompile:dispatch",
		"asset-address": "precompile:runtime:multicurrency",
		"oracle-address": "precompile:runtime:chainlink_adapter",
	}
	for route in runtime_inventory.precompile_inventory(precompile_text):
		precompile = precompile_ids.get(route["route"], f"precompile:standard:{route['route']}")
		g.node(precompile, "precompile", domain="precompile", configured_by=precompile_config,
			address=route.get("address"), address_predicate=route.get("predicate"), dynamic_address=route["dynamic"],
			executor=route.get("target"))
		g.edge("boundary:evm-execution", precompile, "invokes-precompile", file=precompile_config,
			address=route.get("address"), predicate=route.get("predicate"), executor=route.get("target"))
	g.edge("precompile:dispatch", "boundary:frame-dispatch", "dispatches-frame", file=precompile_config,
		line=line_of(precompile_text, precompile_text.index("pallet_evm_precompile_dispatch")),
		evidence="Dispatch::<R>::execute(handle)")
	runtime_lib = (root / "runtime/hydradx/src/lib.rs").read_text()
	runtime_entries = runtime_inventory.construct_runtime_entries(runtime_lib)
	aliases = {entry["alias"]: entry["crate"] for entry in runtime_entries}
	runtime_components = {component_id(entry["crate"]) for entry in runtime_entries}
	runtime_instances_by_config = {
		(f"{canonical_config_crate(entry['crate'])}::Config", entry["instance"]): f"runtime-instance:{entry['alias']}"
		for entry in runtime_entries
	}
	for entry in runtime_entries:
		pid = component_id(entry["crate"])
		instance = f"runtime-instance:{entry['alias']}"
		g.node(pid, "pallet", runtime_alias=entry["alias"], crate=entry["crate"], domain="frame")
		g.node(instance, "runtime-pallet-instance", owner=pid, domain="frame", **entry)
		g.edge("runtime:hydradx", instance, "contains")
		g.edge(instance, pid, "instantiates", runtime_alias=entry["alias"], instance=entry["instance"],
			pallet_index=entry["index"])
	if "EVM" in aliases:
		g.edge("runtime-instance:EVM", "boundary:evm-execution", "executes-evm", evidence="Frontier pallet")
	if "Ethereum" in aliases:
		g.edge("runtime-instance:Ethereum", "runtime-instance:EVM", "submits-ethereum-transaction")
	for alias in ("MessageQueue", "XcmpQueue", "CumulusXcm", "OrmlXcm"):
		if alias in aliases:
			g.edge("boundary:xcm-inbound", f"runtime-instance:{alias}", "delivers-xcm")

	files = list((root / "pallets").glob("*/src/**/*.rs"))
	files += list((root / "precompiles").glob("*/src/**/*.rs"))
	files += list((root / "runtime/hydradx/src").glob("**/*.rs"))
	files += list((root / "runtime/adapters/src").glob("**/*.rs"))
	source_specs: list[tuple[Path, str | None, str | None, str | None]] = [
		(path, None, None, None) for path in sorted(set(files)) if not source_excluded(path)
	]
	runtime_inventory_complete = True
	try:
		sources = runtime_inventory.runtime_source_inventory(root, runtime_entries)
		for source in sources:
			if not source["external"]:
				continue
			source_root = Path(source["source_root"])
			for path in runtime_inventory.active_external_sources(source_root):
				if source_excluded(path):
					continue
				rel = f"external/{source['crate']}/{path.relative_to(source_root).as_posix()}"
				source_specs.append((path, component_id(source["crate"]), "frame", rel))
		g.node("semantic-analysis:runtime-source-inventory", "semantic-coverage", tool="cargo-metadata",
			status="ok", runtime_entries=len(runtime_entries), resolved_sources=len(sources),
			external_sources=sum(source["external"] for source in sources))
	except (subprocess.CalledProcessError, StopIteration) as error:
		runtime_inventory_complete = False
		g.node("semantic-analysis:runtime-source-inventory", "semantic-coverage", tool="cargo-metadata",
			status="unavailable", error=str(error), runtime_entries=len(runtime_entries))
	config_declarations: dict[str, dict[str, str]] = defaultdict(dict)
	config_parents: dict[str, set[str]] = defaultdict(set)
	config_declarations_exact: dict[str, dict[str, str]] = defaultdict(dict)
	config_parents_exact: dict[str, set[str]] = defaultdict(set)
	active_configs: dict[str, set[str | None]] = defaultdict(set)
	source_config_data: dict[str, dict[str, object]] = {}
	selector_definitions: dict[tuple[str, tuple[str, ...], str], dict[str, str] | None] = {}
	for path, forced_owner, forced_domain, forced_rel in source_specs:
		text = path.read_text(errors="replace")
		src, _ = (forced_owner, forced_domain) if forced_owner else owner(path, root)
		rel = forced_rel or path.relative_to(root).as_posix()
		imports = rust_use_bindings(text)
		default_trait = source_config_trait(src, rel)
		type_aliases = rust_type_aliases(text, imports)
		declarations = config_associated_types(text)
		config_traits = list(CONFIG_TRAIT.finditer(mask_rust_comments(text)))
		config_declarations[src].update(declarations)
		if default_trait and config_traits:
			config_declarations_exact[default_trait].update(declarations)
		source_config_data[rel] = {"imports": imports, "default_trait": default_trait,
			"type_aliases": type_aliases}
		for match in config_traits:
			config_parents[src].update(config_trait_components(match.group(0), src) - {src})
			if default_trait:
				config_parents_exact[default_trait].update(
					trait for trait, _ in config_references(
						match.group(0), imports, default_trait, type_aliases)
					if trait != default_trait)
		for runtime_match, _ in runtime_config_blocks(text):
			raw = f"{runtime_match.group(1)}::Config"
			if runtime_match.group(2):
				raw += f"<{runtime_match.group(2)}>"
			reference = config_reference(raw, imports, default_trait, type_aliases)
			if reference:
				active_configs[reference[0]].add(reference[1])
		crate_scope, module = rust_source_module(rel)
		for enum_name, variants in generated_selector_enums(text).items():
			key = (crate_scope, module, enum_name)
			if key not in selector_definitions:
				selector_definitions[key] = variants
			elif selector_definitions[key] != variants:
				selector_definitions[key] = None
	for config_data in source_config_data.values():
		config_data["default_trait"] = nearest_config_trait(
			config_data["default_trait"], config_declarations_exact)

	local_symbols: dict[str, set[str]] = defaultdict(set)
	for runtime_path in list((root / "runtime/hydradx/src").glob("**/*.rs")) + \
		list((root / "runtime/adapters/src").glob("**/*.rs")):
		if source_excluded(runtime_path):
			continue
		runtime_text = runtime_path.read_text(errors="replace")
		runtime_owner, _ = owner(runtime_path, root)
		inactive_ranges = inactive_cfg_ranges(runtime_text)
		for symbol in re.finditer(r"(?m)^pub\s+(?:type|struct)\s+([A-Z][A-Za-z0-9_]*)", runtime_text):
			if not any(start <= symbol.start() < end for start, end in inactive_ranges):
				local_symbols[symbol.group(1)].add(runtime_owner)
	for path, forced_owner, forced_domain, forced_rel in sorted(source_specs, key=lambda item: item[3] or item[0].as_posix()):
		text = path.read_text(errors="replace")
		src, domain = (forced_owner, forced_domain) if forced_owner else owner(path, root)
		g.node(src, domain, domain=domain)
		rel = forced_rel or path.relative_to(root).as_posix()
		config_data = source_config_data[rel]
		imports = config_data["imports"]
		default_trait = config_data["default_trait"]
		type_aliases = config_data["type_aliases"]
		selector_bindings = selector_type_bindings(text, rel, selector_definitions)
		inactive_ranges = inactive_cfg_ranges(text)
		for assoc, bounds in config_associated_types(text).items():
			associated = associated_identity(default_trait, None, assoc) if default_trait else f"associated:{src}:{assoc}"
			configured_owner = config_owner(default_trait, src) if default_trait else src
			g.node(associated, "associated-type", associated_type=assoc,
				config_trait=default_trait or f"{src}::Config", config_instance=None,
				trait_bounds=bounds, associated_role=associated_role(assoc, bounds), unresolved=True,
				declaration_file=rel, owner=configured_owner)
			g.edge(configured_owner, associated, "declares-associated-type", file=rel)

		for m, config_body in runtime_config_blocks(text):
			if any(start <= m.start() < end for start, end in inactive_ranges):
				continue
			raw_config = f"{m.group(1)}::Config"
			if m.group(2):
				raw_config += f"<{m.group(2)}>"
			config_reference_value = config_reference(raw_config, imports, default_trait, type_aliases)
			if config_reference_value:
				config_trait, config_instance = config_reference_value
				configured = config_owner(config_trait, src)
			else:
				config_trait, config_instance = f"{m.group(1)}::Config", None
				configured = component_id(m.group(1).split("::", 1)[0])
			for assignment in config_type_items(config_body, "="):
				if any(start <= m.end() + int(assignment["start"]) < end for start, end in inactive_ranges):
					continue
				assoc, value = str(assignment["name"]), str(assignment["value"])
				target = f"component:binding:{assoc}:{re.sub(r'\s+', '', value)[:100]}"
				g.node(target, "runtime-binding", associated_type=assoc, value=value.strip())
				g.edge(configured, target, "config-binding", file=rel, line=line_of(text, m.start()),
					config_trait=config_trait, config_instance=config_instance)
				associated = associated_identity(config_trait, config_instance, assoc)
				bounds = g.nodes.get(associated, {}).get("trait_bounds") \
					or config_declarations_exact.get(config_trait, {}).get(assoc) \
					or config_declarations.get(configured, {}).get(assoc)
				runtime_instance = runtime_instances_by_config.get((config_trait, config_instance))
				g.node(associated, "associated-type", associated_type=assoc, config_trait=config_trait,
					config_instance=config_instance, runtime_instance=runtime_instance,
					associated_role=associated_role(assoc, bounds), trait_bounds=bounds, unresolved=True,
					owner=configured)
				g.edge(associated, target, "configured-as", file=rel, line=line_of(text, m.start()),
					value=value.strip(), config_trait=config_trait, config_instance=config_instance)
				resolved = config_callback_targets(value, aliases, local_symbols)
				role = g.nodes[associated].get("associated_role")
				resolved = [item for item in resolved if item != configured and role == "callback"
					and assoc != "BenchmarkHelper"]
				if not resolved and role in {"callback", "unknown"}:
					resolved.append(target)
				for concrete in sorted(set(resolved)):
					g.edge(associated, concrete, "binding-resolves-to", file=rel,
						line=line_of(text, m.start()), value=value.strip())

		helper_ranges = helper_module_ranges(text)
		scopes = scope_ranges(text)
		function_ranges: list[tuple[int, int]] = []
		call_index_targets = attribute_targets(text, re.compile(r"#\[pallet::call_index\((\d+)\)\]"))
		selector_targets = attribute_target_lists(text, PRECOMPILE_PUBLIC)
		runtime_api_match = re.search(r"\bimpl_runtime_apis!\s*\{", text)
		runtime_api_end = body_end(text, runtime_api_match.end() - 1) if runtime_api_match else None
		for fm in FN.finditer(text):
			end = function_body_end(text, fm.end())
			if not end:
				continue
			if any(start <= fm.start() < stop for start, stop in helper_ranges + inactive_ranges):
				continue
			nested_function = any(start < fm.start() < stop for start, stop in function_ranges)
			function_ranges.append((fm.start(), end))
			body = text[fm.start():end]
			fid = function_source_id(text, fm, rel, scopes)
			attrs = text[max(0, fm.start() - 500):fm.start()]
			impl_header = impl_context(fm.start(), scopes)
			trait_impl = implemented_trait(impl_header)
			function_opening = text.find("{", fm.end())
			function_header = text[fm.start():function_opening] if function_opening >= 0 else ""
			config_context = f"{impl_header or ''} {function_header}"
			associated_candidates = {src} | config_trait_components(config_context, src)
			exact_config_candidates = config_references(
				config_context, imports, default_trait, type_aliases)
			lifecycle = fm.group(1) in LIFECYCLE_HOOKS and bool(trait_impl and re.search(r"\bHooks\b", trait_impl))
			transactional = "#[transactional]" in attrs or "with_transaction" in body
			can_enter = entrypoint_eligible(path, fm.start(), helper_ranges) and not nested_function
			local_pallet_calls = [{"method": call.group(1),
				"line": line_of(text, fm.start() + call.start())}
				for call in LOCAL_PALLET_CALL.finditer(body)] if migration_source(rel) else []
			g.node(fid, "function", name=fm.group(1), file=rel, line=line_of(text, fm.start()), owner=src,
				 domain=domain, lifecycle=lifecycle, transactional=transactional, entrypoint_eligible=can_enter,
				 impl_header=impl_header, implemented_trait=trait_impl, source_config_trait=default_trait,
				 local_pallet_calls=local_pallet_calls,
				 required_config_traits=[{"trait": trait, "instance": instance}
					for trait, instance in sorted(exact_config_candidates,
						key=lambda item: (item[0], item[1] or ""))])
			g.edge(src, fid, "defines")
			call_index = call_index_targets.get(fm.start())
			if can_enter and call_index:
				add_entrypoint(g, fid, "extrinsic", call_index=int(call_index.group(1)))
			if can_enter and lifecycle:
				add_entrypoint(g, fid, "runtime-hook", hook=fm.group(1))
			special_active = fm.group(1) not in {"on_runtime_upgrade", "pre_upgrade", "post_upgrade"} or lifecycle
			if can_enter and fm.group(1) in SPECIAL_ENTRYPOINTS and trait_impl and special_active:
				add_entrypoint(g, fid, SPECIAL_ENTRYPOINTS[fm.group(1)], method=fm.group(1))
			if can_enter and rel == "runtime/hydradx/src/lib.rs" and runtime_api_match and runtime_api_end \
				and runtime_api_match.start() < fm.start() < runtime_api_end:
				add_entrypoint(g, fid, "runtime-api", method=fm.group(1))
			resolved_selector_uses = []
			for selector_use in SELECTOR_VARIANT.finditer(body):
				selector_type, variant = selector_use.groups()
				variants = selector_bindings.get(selector_type)
				if not variants or variant not in variants:
					continue
				signature = variants[variant]
				_, selector_node = ensure_evm_selector(g, signature)
				resolved_selector_uses.append((selector_type, variant, signature, selector_node,
					line_of(text, fm.start() + selector_use.start())))
			precompile_execute = can_enter and domain == "precompile" and src != "precompile:utils" \
				and fm.group(1) == "execute" and trait_impl in {"Precompile", "PrecompileSet"}
			selectors = selector_targets.get(fm.start(), [])
			if can_enter and selectors:
				add_precompile_selector_entrypoints(g, fid,
					[(selector.group(1), line_of(text, selector.start())) for selector in selectors], rel)
			elif precompile_execute:
				manual_selectors = {}
				for _, _, signature, _, line in resolved_selector_uses:
					manual_selectors.setdefault(signature, line)
				if manual_selectors:
					add_precompile_selector_entrypoints(g, fid, sorted(manual_selectors.items()), rel)
				else:
					add_entrypoint(g, fid, "precompile-dispatch")
			if can_enter and domain == "evm-adapter" and fm.group(1) in {"call", "execute", "transfer", "deposit", "withdraw"}:
				add_entrypoint(g, fid, "evm-adapter", method=fm.group(1))
			if can_enter and rel == "runtime/hydradx/src/xcm.rs" and fm.group(1) in {"execute", "process_message"}:
				entrypoint = add_entrypoint(g, fid, "xcm-inbound", method=fm.group(1))
				g.edge("boundary:xcm-inbound", entrypoint, "receives-xcm")
			if not precompile_execute:
				for selector_type, variant, _, selector_node, selector_line in resolved_selector_uses:
					g.edge(fid, selector_node, "encodes-evm-selector", selector_type=selector_type,
						variant=variant, file=rel, line=selector_line)
			for check, origin_kind in ORIGIN_CHECKS.items():
				if not re.search(rf"\b{check}\s*\(", body):
					continue
				origin_node = f"origin:{origin_kind}"
				g.node(origin_node, "origin", domain="authorization", origin_kind=origin_kind)
				g.edge(origin_node, fid, "authorizes-entry", check=check, file=rel, line=line_of(text, fm.start()))
			for origin_type in sorted(set(re.findall(
				r"(?:T::|<T\s+as\s+[^>]+>::)([A-Z][A-Za-z0-9_]*Origin)::ensure_origin\s*\(", body))):
				origin_node = f"origin:runtime-config:{src}:{origin_type}"
				g.node(origin_node, "origin", domain="authorization", origin_kind="runtime-config",
					associated_type=origin_type)
				references = associated_config_references(exact_config_candidates, origin_type,
					config_declarations_exact, config_parents_exact)
				expanded = {item for reference in references
					for item in active_config_references(reference, active_configs)}
				if not expanded:
					expanded = {(f"{src}::Config", None)}
				for config_trait, config_instance in sorted(expanded, key=lambda item: (item[0], item[1] or "")):
					associated = associated_identity(config_trait, config_instance, origin_type) \
						if config_trait in config_declarations_exact else f"associated:{src}:{origin_type}"
					g.node(associated, "associated-type", associated_type=origin_type,
						associated_role="callback", unresolved=True, owner=config_owner(config_trait, src),
						config_trait=config_trait, config_instance=config_instance)
					g.edge(origin_node, associated, "configured-origin", file=rel,
						line=line_of(text, fm.start()), config_trait=config_trait,
						config_instance=config_instance)
				g.edge(origin_node, fid, "authorizes-entry", check=f"{origin_type}::ensure_origin", file=rel,
					line=line_of(text, fm.start()))

			storage_matches = [match for match in STORAGE.finditer(body) if is_storage_match(match)]
			write_positions = [x.start() for x in storage_matches if x.group(2) in STORAGE_WRITES]
			external_matches = list(EXTERNAL.finditer(body))
			external_positions = [x.start() for x in external_matches]
			operations = []
			for sm in storage_matches:
				operations.append({"offset": sm.start(), "line": line_of(text, fm.start() + sm.start()),
					"kind": "storage-write" if sm.group(2) in STORAGE_WRITES else "storage-read",
					"operation": sm.group(2), "storage": sm.group(1)})
			for em in external_matches:
				operations.append({"offset": em.start(), "line": line_of(text, fm.start() + em.start()),
					"kind": "external-call", "operation": em.group(1).strip()})
			g.nodes[fid]["operations"] = sorted(operations, key=lambda item: item["offset"])
			for cm in PALLET_CALL.finditer(body):
				target = component_id(cm.group(1))
				g.node(target, "pallet", domain="frame")
				g.edge(fid, target, "direct-call", method=cm.group(2), file=rel,
					line=line_of(text, fm.start() + cm.start()))
			for cm in INTERNAL_EVM_EXECUTOR.finditer(body):
				target = "component:evm:executor"
				g.node(target, "evm-adapter", domain="evm-adapter")
				g.edge(fid, target, "direct-call", method=cm.group(1), file=rel,
					line=line_of(text, fm.start() + cm.start()))
			for call in associated_calls(mask_rust_comments(body)):
				associated_type = str(call["associated_type"])
				method = str(call["method"])
				references: set[tuple[str, str | None]] = set()
				if call["trait_path"]:
					explicit = config_reference(str(call["trait_path"]), imports, default_trait, type_aliases)
					if not explicit:
						continue
					references = associated_config_references({explicit}, associated_type,
						config_declarations_exact, config_parents_exact) or {explicit}
				else:
					references = associated_config_references(exact_config_candidates, associated_type,
						config_declarations_exact, config_parents_exact)
				expanded = {item for reference in references
					for item in active_config_references(reference, active_configs)}
				targets = []
				for config_trait, config_instance in sorted(expanded, key=lambda item: (item[0], item[1] or "")):
					targets.append((associated_identity(config_trait, config_instance, associated_type),
						config_owner(config_trait, src), config_trait, config_instance,
						config_declarations_exact.get(config_trait, {}).get(associated_type)))
				if not targets:
					target_owner = associated_config_owner(associated_candidates, associated_type,
						config_declarations, config_parents) or src
					target = f"associated:{target_owner}:{associated_type}"
					existing = g.nodes.get(target, {})
					targets.append((target, target_owner, existing.get("config_trait"),
						existing.get("config_instance"), existing.get("trait_bounds")
						or config_declarations.get(target_owner, {}).get(associated_type)))
				for target, target_owner, config_trait, config_instance, bounds in targets:
					target_metadata = {"associated_type": associated_type,
						"associated_role": associated_role(associated_type, bounds), "trait_bounds": bounds,
						"unresolved": True, "owner": target_owner}
					if config_trait is not None:
						target_metadata.update({"config_trait": config_trait, "config_instance": config_instance})
					g.node(target, "associated-type", **target_metadata)
					g.edge(fid, target, "dynamic-call", method=method, file=rel,
						line=line_of(text, fm.start() + int(call["start"])), config_trait=config_trait,
						config_instance=config_instance)
					if associated_type in {"Currency", "Currencies", "MultiCurrency", "AssetTransactor"}:
						for backend in asset_backends:
							g.edge(target, f"asset-backend:{backend}", "may-resolve-to")
				if method in ASSET_METHODS:
					operation = f"asset-operation:{ASSET_METHODS[method]}"
					g.node(operation, "asset-operation", domain="asset", operation=ASSET_METHODS[method])
					g.edge(fid, operation, "asset-operation", method=method, file=rel,
						line=line_of(text, fm.start() + int(call["start"])), backend_type=associated_type)
			for cm in RUNTIME_CALL.finditer(body):
				if cm.group(1) in aliases:
					target = f"pallet:{aliases[cm.group(1)].removeprefix('pallet_').replace('_', '-')}"
					g.edge(fid, target, "runtime-alias-call", method=cm.group(2), file=rel,
						line=line_of(text, fm.start() + cm.start()))

			for sm in storage_matches:
				storage_id = f"storage:{src}:{sm.group(1)}"
				g.node(storage_id, "storage", owner=src, storage_name=sm.group(1))
				g.edge(fid, storage_id, "storage-access", operation=sm.group(2),
					line=line_of(text, fm.start() + sm.start()))
			if external_matches:
				g.node("boundary:external-execution", "execution-boundary", domain="dynamic",
					description="Conservative call, dispatch, transfer, mint, or burn boundary")
			for em in external_matches:
				g.edge(fid, "boundary:external-execution", "external-execution",
					expression=em.group(1).strip(), file=rel, line=line_of(text, fm.start() + em.start()))
			for em in EVM_ENTRY.finditer(body):
				g.edge(fid, "boundary:evm-execution", "enters-evm", expression=em.group(0).strip(),
					file=rel, line=line_of(text, fm.start() + em.start()))
			if domain == "precompile":
				for dm in FRAME_DISPATCH.finditer(body):
					g.edge(fid, "boundary:frame-dispatch", "dispatches-frame", expression=dm.group(0).strip(),
						file=rel, line=line_of(text, fm.start() + dm.start()))
			elif FRAME_DISPATCH.search(body):
				g.node("boundary:nested-frame-dispatch", "execution-boundary", domain="frame",
					description="A runtime component dispatching a nested RuntimeCall")
				g.edge(fid, "boundary:nested-frame-dispatch", "nested-dispatch", file=rel,
					line=line_of(text, fm.start() + FRAME_DISPATCH.search(body).start()))
			for xm in XCM_SEND.finditer(body):
				g.edge(fid, "component:xcm:router", "sends-xcm", expression=xm.group(0).strip(),
					file=rel, line=line_of(text, fm.start() + xm.start()))
			for xm in XCM_ASSET.finditer(body):
				g.edge(fid, "component:xcm:asset-transactor", "moves-xcm-asset", expression=xm.group(0).strip(),
					file=rel, line=line_of(text, fm.start() + xm.start()))
			if external_positions:
				before = any(s < external_positions[0] for s in write_positions)
				after = any(s > external_positions[0] for s in write_positions)
				if before or after:
					g.node(f"candidate:{fid}", "audit-candidate", rule="external-call-amid-state",
						storage_before=before, storage_after=after, transactional=transactional,
						first_external_line=line_of(text, fm.start() + external_positions[0]),
						external_call_count=len(external_positions), storage_write_count=len(write_positions))
					g.edge(fid, f"candidate:{fid}", "flagged-by")

	enrich_migration_calls(g)
	normalize_config_value_calls(g)
	enrich_resolutions(g)
	enrich_configured_origins(g)
	enrich_callback_entrypoints(g)
	add_configured_migrations(g, root)
	enrich_runtime_instances(g, runtime_entries)
	classify_runtime_activity(g, runtime_components, active_configs, runtime_inventory_complete,
		set(config_declarations_exact))
	merge_integration_tests(g, root, aliases)
	add_state_semantics(g)
	merge_semantic_inventory(g, root)
	classify_unresolved(g)
	return g


def write_outputs(g: Graph, output: Path) -> None:
	output.mkdir(parents=True, exist_ok=True)
	payload = {"schema_version": 2, "nodes": sorted(g.nodes.values(), key=lambda x: x["id"]),
		"edges": sorted(g.edges, key=lambda x: (x["source"], x["target"], x["kind"]))}
	(output / "interaction-graph.json").write_text(json.dumps(payload, indent=2) + "\n")
	projections = {name: sorted(projected_edges(g, name, active_only=False),
		key=lambda edge: (edge["source"], edge["target"], edge["kind"])) for name in EDGE_PROJECTIONS}
	active_projections = {name: [edge for edge in edges if edge_runtime_active(g, edge)]
		for name, edges in projections.items()}
	projection_output = output / "projections"
	projection_output.mkdir(exist_ok=True)
	active_projection_output = output / "projections-active"
	active_projection_output.mkdir(exist_ok=True)
	for directory, selected, activity in ((projection_output, projections, "raw-inventory"),
		(active_projection_output, active_projections, "runtime-active")):
		for name, edges in selected.items():
			node_ids = {endpoint for edge in edges for endpoint in (edge["source"], edge["target"])}
			projection_payload = {"schema_version": 1, "projection": name, "activity": activity,
				"nodes": sorted((g.nodes[ident] for ident in node_ids), key=lambda node: node["id"]),
				"edges": edges}
			(directory / f"{name}.json").write_text(json.dumps(projection_payload, indent=2) + "\n")
	all_components = resolved_component_edges(g, component_edges(g))
	components = [edge for edge in all_components if edge["kind"] in EDGE_PROJECTIONS["execution"]]
	cycles = strongly_connected(components)
	execution_paths, path_search = bounded_paths_with_metadata(
		components, {"boundary:evm-execution", "boundary:frame-dispatch"})
	dangerous = dangerous_interactions(g, cycles, execution_paths)
	(output / "query-packs.json").write_text(json.dumps(
		query_packs(g, components, projections, cycles, execution_paths, path_search, dangerous), indent=2) + "\n")
	(output / "component-graph.json").write_text(json.dumps(
		{"schema_version": 2, "projection": "execution", "edges": components, "cycles": cycles,
			"execution_paths": execution_paths, "path_search": path_search,
			"available_projections": sorted(projections), "dangerous_interactions": dangerous}, indent=2) + "\n")
	gaps = prioritize_gaps(g, components)
	gap_lines = ["# Prioritized static test gaps", "",
		"Priorities combine missing source linkage with privilege, asset impact, and execution-domain crossings.", ""]
	for item in gaps[:250]:
		gap_lines.append(f"- **{item['score']}** `{item['id']}` — {', '.join(item['reasons'])}")
	(output / "prioritized-test-gaps.md").write_text("\n".join(gap_lines) + "\n")
	with (output / "interaction-graph.dot").open("w") as f:
		f.write("digraph runtime_interactions {\n  rankdir=LR;\n")
		for node in payload["nodes"]:
			f.write(f'  "{node["id"]}" [label="{node["id"]}"];\n')
		for edge in payload["edges"]:
			f.write(f'  "{edge["source"]}" -> "{edge["target"]}" [label="{edge["kind"]}"];\n')
		f.write("}\n")
	candidates = [n for n in payload["nodes"] if n["kind"] == "audit-candidate"]
	unresolved = [n for n in payload["nodes"] if n.get("unresolved")]
	inventory_only_associated = [n for n in payload["nodes"]
		if n.get("roles", [n.get("kind")]) and (n.get("kind") == "associated-type"
			or "associated-type" in n.get("roles", [])) and n.get("runtime_active") is False]
	lines = ["# Runtime interaction audit candidates", "", f"- Nodes: {len(payload['nodes'])}",
		f"- Edges: {len(payload['edges'])}", f"- State/external-call candidates: {len(candidates)}",
		f"- Unresolved associated-type targets: {len(unresolved)}",
		f"- Inventory-only associated targets: {len(inventory_only_associated)}", "", "## Candidates", ""]
	for candidate in candidates:
		fid = candidate["id"].removeprefix("candidate:")
		function = g.nodes.get(fid, {})
		high = function.get("domain") in {"evm-adapter", "precompile"} or (
			candidate.get("storage_before") and candidate.get("storage_after") and not candidate.get("transactional")
		)
		lines.append(
			f"- **{'HIGH' if high else 'REVIEW'}** `{function.get('file')}:{function.get('line')}` "
			f"`{function.get('name')}` — domain: `{function.get('domain')}`, transactional: `{candidate['transactional']}`, "
			f"writes before/after first external call: `{candidate.get('storage_before')}/{candidate.get('storage_after')}`, "
			f"calls/writes: `{candidate.get('external_call_count')}/{candidate.get('storage_write_count')}`"
		)
	(output / "audit-candidates.md").write_text("\n".join(lines) + "\n")
	resolution_counts = defaultdict(int)
	for node in unresolved:
		resolution_counts[node.get("ambiguity_reason", "unclassified")] += 1
	resolution_lines = ["# Runtime target resolution coverage", "",
		f"- Unresolved associated targets: {len(unresolved)}",
		f"- Inventory-only associated targets: {len(inventory_only_associated)}", ""]
	resolution_lines.extend(f"- `{reason}`: {count}" for reason, count in sorted(resolution_counts.items()))
	activity_counts = Counter(node.get("runtime_activity_reason", "inventory-only")
		for node in inventory_only_associated)
	if activity_counts:
		resolution_lines.extend(["", "## Inventory-only classifications", ""])
		resolution_lines.extend(f"- `{reason}`: {count}" for reason, count in sorted(activity_counts.items()))
	resolution_lines.extend(["", "Every unresolved node retains its associated type and candidate runtime targets.", ""])
	(output / "resolution-coverage.md").write_text("\n".join(resolution_lines) + "\n")
	cycle_lines = ["# Component interaction cycles", "",
		"Cycles use only the execution projection. Configuration, state, authorization, and deployment edges are excluded.", ""]
	for number, cycle in enumerate(cycles, 1):
		domains = {g.nodes.get(node, {}).get("domain") for node in cycle}
		crosses_evm = bool(domains & {"evm-adapter", "precompile"}) or any("evm" in node.lower() or "precompile" in node.lower() for node in cycle)
		members = set(cycle)
		evidence = [edge for edge in components if edge["source"] in members and edge["target"] in members]
		migration_only_return = any("/migrations/" in (edge.get("file") or "") for edge in evidence)
		severity = "HIGH " if crosses_evm else ("MIGRATION " if migration_only_return else "")
		cycle_lines.extend([f"## {number}. {severity}cycle ({len(cycle)} nodes)", ""])
		cycle_lines.extend(f"- `{node}`" for node in cycle)
		cycle_lines.extend(["", "Evidence:", ""])
		for edge in evidence:
			location = f"{edge.get('file')}:{edge.get('line')}" if edge.get("file") else "runtime binding"
			method = f"::{edge['method']}" if edge.get("method") else ""
			cycle_lines.append(f"- `{edge['source']}` → `{edge['target']}{method}` ({edge['kind']}, `{location}`)")
		cycle_lines.append("")
	(output / "interaction-cycles.md").write_text("\n".join(cycle_lines) + "\n")
	boundary_edges = [edge for edge in payload["edges"]
		if edge["kind"] in {"enters-evm", "dispatches-frame"} and edge_runtime_active(g, edge)]
	boundary_lines = ["# Native and EVM execution boundaries", "",
		"These are syntactic boundary crossings for audit review, not proof of an exploitable path.", ""]
	for edge in boundary_edges:
		function = g.nodes.get(edge["source"], {})
		boundary_lines.append(
			f"- `{function.get('file')}:{edge.get('line')}` `{function.get('name')}` "
			f"→ `{edge['target']}` (`{edge['kind']}`, `{edge.get('expression')}`)"
		)
	(output / "execution-boundaries.md").write_text("\n".join(boundary_lines) + "\n")
	path_lines = ["# Cross-domain execution paths", "",
		"Bounded paths use runtime-resolved callbacks and syntactic execution boundaries.",
		f"Search depth: {path_search['max_depth']}; per-start limit: {path_search['per_start_limit']}; "
		f"limit-truncated starts: {len(path_search['limit_truncated_starts'])}; "
		f"depth-truncated starts: {len(path_search['depth_truncated_starts'])}.", ""]
	for path in execution_paths:
		path_lines.append("- " + " → ".join(f"`{node}`" for node in path))
	(output / "execution-paths.md").write_text("\n".join(path_lines) + "\n")
	danger_lines = ["# Dangerous interaction candidates", "",
		"Rule matches are review targets and retain their operation or path evidence.", ""]
	for item in dangerous:
		location = item.get("function") or " → ".join(item["path"])
		danger_lines.append(f"- **{item['severity'].upper()}** `{item['rule']}` — `{location}`")
		if item.get("operations"):
			sequence = " → ".join(f"{operation['kind']}@{operation.get('line', '?')}"
				for operation in item["operations"] if operation["kind"] != "storage-read")
			danger_lines.append(f"  - ordered evidence: `{sequence}`")
	(output / "dangerous-interactions.md").write_text("\n".join(danger_lines) + "\n")
	coverage = g.nodes.get("semantic-analysis:rapx-coverage")
	if coverage:
		coverage_lines = ["# RAPx semantic coverage", "",
			f"- Callgraph success: {coverage['callgraph_success']}/{coverage['total_packages']}", "",
			"| Package | Callgraph | MIR | Dataflow |", "|---|---|---|---|"]
		for package in coverage["packages"]:
			coverage_lines.append(f"| `{package['package']}` | {package['callgraph']} | {package['mir']} | {package['dataflow']} |")
		(output / "semantic-coverage.md").write_text("\n".join(coverage_lines) + "\n")
	mir_coverage = sorted((node for node in g.nodes.values() if node.get("tool") == "rustc-mir"),
		key=lambda node: node["owner"])
	if mir_coverage:
		mir_lines = ["# rustc MIR semantic coverage", "",
			"Control-flow ordering comes from genuine `rustc -Zunpretty=mir` output.", "",
			"| Owner | Source functions | Matched functions | MIR instances | Coverage | Relevant operations | Artifact |",
			"|---|---:|---:|---:|---:|---:|---|"]
		for item in mir_coverage:
			ratio = item.get("source_function_coverage")
			ratio_text = f"{ratio:.1%}" if ratio is not None else "n/a"
			mir_lines.append(f"| `{item['owner']}` | {item.get('source_functions_total', 0)} | "
				f"{item['matched_functions']} | {item.get('matched_instances', 0)} | {ratio_text} | "
				f"{item['operation_count']} | `{item['artifact']}` |")
		(output / "mir-coverage.md").write_text("\n".join(mir_lines) + "\n")
	kind_counts = defaultdict(int)
	for node in payload["nodes"]:
		kind_counts[node["kind"]] += 1
	edge_counts = defaultdict(int)
	for edge in payload["edges"]:
		edge_counts[edge["kind"]] += 1
	component_nodes = {node["id"] for node in payload["nodes"] if node["kind"] in COMPONENT_NODE_KINDS}
	coverage_lines = ["# Graph coverage", "", f"- Nodes: {len(payload['nodes'])}",
		f"- Edges: {len(payload['edges'])}", f"- Components: {len(component_nodes)}",
		f"- Unresolved targets: {len(unresolved)}", "", "## Node kinds", ""]
	coverage_lines.extend(f"- `{kind}`: {count}" for kind, count in sorted(kind_counts.items()))
	coverage_lines.extend(["", "## Edge kinds", ""])
	coverage_lines.extend(f"- `{kind}`: {count}" for kind, count in sorted(edge_counts.items()))
	coverage_lines.extend(["", "## Projection edges", ""])
	coverage_lines.extend(f"- `{name}`: {len(edges)}" for name, edges in sorted(projections.items()))
	coverage_lines.extend(["", "## Runtime-active projection edges", ""])
	coverage_lines.extend(f"- `{name}`: {len(edges)}" for name, edges in sorted(active_projections.items()))
	(output / "graph-coverage.md").write_text("\n".join(coverage_lines) + "\n")
	owners = sorted({node.get("owner") for node in payload["nodes"]
		if node["kind"] == "function" and node.get("owner") and node.get("file")})
	entrypoint_counts = defaultdict(int)
	active_entrypoint_counts = defaultdict(int)
	for node in payload["nodes"]:
		if node["kind"] == "entrypoint":
			entrypoint_counts[node.get("entrypoint_kind", "unknown")] += 1
			if node_runtime_active(g, node):
				active_entrypoint_counts[node.get("entrypoint_kind", "unknown")] += 1
	compiler_owners = {node.get("owner") for node in payload["nodes"] if node.get("tool") == "rustc-mir"}
	historical_status = historical_property_status(g, all_components)
	completeness = {"schema_version": 1, "source_components": len(owners),
		"source_components_without_entrypoints": sorted(owner for owner in owners if not any(
			node["kind"] == "entrypoint" and node.get("owner") == owner for node in payload["nodes"])),
		"entrypoint_kinds": dict(sorted(entrypoint_counts.items())),
		"compiler_enriched_owners": sorted(owner for owner in compiler_owners if owner),
		"unresolved_targets": len(unresolved), "state_invariants": kind_counts["state-invariant"],
		"asset_operations": kind_counts["asset-operation"], "deployed_contracts": kind_counts["deployed-contract"],
		"projection_edges": {name: len(edges) for name, edges in sorted(projections.items())},
		"path_search": path_search, "historical_properties": historical_status}
	(output / "completeness.json").write_text(json.dumps(completeness, indent=2) + "\n")
	complete_lines = ["# Static graph completeness", "", f"- Source components: {len(owners)}",
		f"- Compiler-enriched owners: {len(compiler_owners)}", f"- Unresolved targets: {len(unresolved)}",
		f"- State invariant classes: {kind_counts['state-invariant']}",
		f"- Typed asset operations: {kind_counts['asset-operation']}",
		f"- Deployed contracts: {kind_counts['deployed-contract']}", "", "## Entrypoint classes", ""]
	complete_lines.extend(f"- `{kind}`: {count}" for kind, count in sorted(entrypoint_counts.items()))
	complete_lines.extend(["", "## Historical interaction properties", ""])
	complete_lines.extend(f"- `{'ok' if present else 'missing'}` — `{name}`"
		for name, present in sorted(historical_status.items()))
	complete_lines.extend(["", "## Components without discovered entrypoints", ""])
	complete_lines.extend(f"- `{owner}`" for owner in completeness["source_components_without_entrypoints"])
	(output / "completeness.md").write_text("\n".join(complete_lines) + "\n")
	workspace_mir = g.nodes.get("semantic-analysis:rustc-mir-workspace", {})
	mir_coverage_nodes = [node for node in g.nodes.values() if node.get("tool") == "rustc-mir"]
	deployment_coverage = g.nodes.get("semantic-analysis:contract-deployments", {})
	coverage_payload = {"nodes": len(payload["nodes"]), "edges": len(payload["edges"]),
		"components": len(component_nodes), "unresolved_targets": len(unresolved),
		"inventory_only_targets": len(inventory_only_associated),
		"inventory_only_target_ids": sorted(node["id"] for node in inventory_only_associated),
		"inventory_only_targets_by_reason": dict(sorted(Counter(
			node.get("runtime_activity_reason", "inventory-only") for node in inventory_only_associated).items())),
		"entrypoints": sum(active_entrypoint_counts.values()),
		"entrypoint_kinds": len(active_entrypoint_counts),
		"inventory_entrypoints": kind_counts["entrypoint"],
		"state_invariants": kind_counts["state-invariant"], "asset_operations": kind_counts["asset-operation"],
		"deployed_contracts": kind_counts["deployed-contract"],
		"mir_packages_success": workspace_mir.get("success"),
		"mir_packages_total": workspace_mir.get("total"),
		"mir_packages_failed": (workspace_mir.get("total") - workspace_mir.get("success"))
			if workspace_mir.get("total") is not None and workspace_mir.get("success") is not None else None,
		"mir_functions_matched": sum(node.get("matched_functions", 0) for node in mir_coverage_nodes),
		"mir_instances_matched": sum(node.get("matched_instances", 0) for node in mir_coverage_nodes),
		"mir_source_functions_total": sum(node.get("source_functions_total", 0) for node in mir_coverage_nodes),
		"mir_operation_count": sum(node.get("operation_count", 0) for node in mir_coverage_nodes),
		"runtime_contract_configurations": deployment_coverage.get("runtime_configurations", 0),
		"runtime_registry_erc20_configurations": deployment_coverage.get(
			"asset_registry_erc20_configurations", 0)}
	coverage_payload["projection_edges"] = {name: len(edges) for name, edges in sorted(projections.items())}
	coverage_payload.update({f"{name}_projection_edges": len(edges) for name, edges in projections.items()})
	coverage_payload["active_projection_edges"] = {
		name: len(edges) for name, edges in sorted(active_projections.items())}
	coverage_payload.update({f"active_{name}_projection_edges": len(edges)
		for name, edges in active_projections.items()})
	(output / "coverage.json").write_text(json.dumps(coverage_payload, indent=2) + "\n")
	test_components: dict[str, set[str]] = defaultdict(set)
	test_functions: set[str] = set()
	for edge in g.edges:
		if edge["kind"] == "test-covers-component":
			test_components[edge["source"]].add(edge["target"])
		elif edge["kind"] == "test-covers-entrypoint":
			test_functions.add(edge["target"])
	raw_coverable_components = {ident for ident in component_nodes if g.nodes.get(ident, {}).get("kind") in {
		"pallet", "frame", "precompile", "evm-adapter", "xcm-component", "execution-boundary", "asset-component"}}
	coverable_components = {ident for ident in raw_coverable_components if node_runtime_active(g, g.nodes[ident])}
	all_test_components = set().union(*test_components.values()) if test_components else set()
	raw_covered_components = all_test_components & raw_coverable_components
	covered_components = all_test_components & coverable_components
	covered_entrypoints_all = {edge["source"] for edge in g.edges
		if edge["kind"] == "enters-function" and edge["target"] in test_functions}
	raw_entrypoints = {node["id"] for node in g.nodes.values() if node["kind"] == "entrypoint"}
	entrypoints = {ident for ident in raw_entrypoints if node_runtime_active(g, g.nodes[ident])}
	raw_covered_entrypoints = covered_entrypoints_all & raw_entrypoints
	covered_entrypoints = covered_entrypoints_all & entrypoints
	active_tests = {test for test, covered in test_components.items() if covered & coverable_components}
	covered_interactions = []
	for edge in components:
		tests = sorted(test for test, covered in test_components.items()
			if edge["source"] in covered and edge["target"] in covered)
		if tests:
			covered_interactions.append({"edge": edge, "tests": tests})
	integration_coverage = {"method": "source-reference co-occurrence; not behavioral proof",
		"integration_tests": len(active_tests), "components_total": len(coverable_components),
		"components_test_linked": len(covered_components),
		"components_graph_only": sorted(coverable_components - covered_components),
		"entrypoints_total": len(entrypoints), "entrypoints_test_linked": len(covered_entrypoints),
		"entrypoints_graph_only": sorted(entrypoints - covered_entrypoints),
		"raw_inventory": {"integration_tests": len(test_components),
			"components_total": len(raw_coverable_components),
			"components_test_linked": len(raw_covered_components),
			"entrypoints_total": len(raw_entrypoints),
			"entrypoints_test_linked": len(raw_covered_entrypoints)},
		"interactions_total": len(components), "interactions_test_linked": len(covered_interactions),
		"test_confidence": dict(sorted(__import__("collections").Counter(node.get("confidence", "reference")
			for node in g.nodes.values() if node["kind"] == "integration-test").items())),
		"test_linked_interactions": covered_interactions}
	(output / "integration-test-coverage.json").write_text(json.dumps(integration_coverage, indent=2) + "\n")
	integration_lines = ["# Integration-test graph coverage", "",
		"Source linkage and component co-occurrence indicate test attention, not behavioral coverage proof.", "",
		f"- Tests linked: {integration_coverage['integration_tests']}",
		f"- Components linked: {len(covered_components)}/{len(coverable_components)}",
		f"- Entrypoints linked: {len(covered_entrypoints)}/{len(entrypoints)}",
		f"- Interactions linked by endpoint co-occurrence: {len(covered_interactions)}/{len(components)}", "",
		"## Confidence tiers", ""]
	integration_lines.extend(f"- `{tier}`: {count}" for tier, count in integration_coverage["test_confidence"].items())
	integration_lines.extend(["",
		"## Graph-only components", ""])
	integration_lines.extend(f"- `{ident}`" for ident in sorted(coverable_components - covered_components))
	(output / "integration-test-coverage.md").write_text("\n".join(integration_lines) + "\n")
	coverage_payload.update({"integration_tests_linked": len(test_components),
		"integration_components_linked": len(raw_covered_components),
		"integration_entrypoints_linked": len(raw_covered_entrypoints),
		"active_integration_tests_linked": len(active_tests),
		"active_integration_components_linked": len(covered_components),
		"active_integration_entrypoints_linked": len(covered_entrypoints)})
	(output / "coverage.json").write_text(json.dumps(coverage_payload, indent=2) + "\n")
	contracts = [node for node in payload["nodes"] if node["kind"] == "deployed-contract"]
	if contracts:
		contract_lines = ["# Deployed contract coverage", "",
			"Classification is block-pinned; artifact-only records are not assumed to remain deployed.", "",
			"| Network | Address | Classification | Names |", "|---|---|---|---|"]
		for contract in contracts:
			contract_lines.append(f"| `{contract.get('network')}` | `{contract.get('address')}` | "
				f"{contract.get('classification', 'artifact-only')} | {', '.join(contract.get('names', []))} |")
		(output / "contract-coverage.md").write_text("\n".join(contract_lines) + "\n")
	write_overview_dot(g, components, output)
	write_overview_svg(g, components, output)
	write_graph_scale_svg(g, output)
	focus = output / "focused"
	focus.mkdir(exist_ok=True)
	for stale in focus.glob("*.svg"):
		stale.unlink()
	for index, cycle in enumerate(cycles, 1):
		write_focus_svg(cycle, focus / f"cycle-{index:02}.svg", f"component cycle {index}", cyclic=True)
	for index, path_nodes in enumerate(execution_paths, 1):
		write_focus_svg(path_nodes, focus / f"path-{index:03}.svg", f"execution path {index}")
	router_edges = [edge for edge in all_components
		if edge["kind"] in EDGE_PROJECTIONS["execution"] | EDGE_PROJECTIONS["callback"]]
	write_router_svg(router_edges, focus / "router-amm.svg")
	execution_edges = [edge for edge in components if edge["source"].startswith(("boundary:", "precompile:"))
		or edge["target"].startswith(("boundary:", "precompile:"))]
	dependency_edges = [edge for edge in components if edge["kind"] in {
		"direct-call", "runtime-alias-call", "resolved-call", "rapx-call", "mir-component-call",
		"mir-resolved-call"}]
	write_layer_svg(g, dependency_edges, focus / "component-dependencies.svg", "Runtime component dependencies")
	write_layer_svg(g, execution_edges, focus / "execution-boundaries.svg", "FRAME, EVM, and precompile execution")
	write_layer_svg(g, active_projections["asset"], focus / "token-flows.svg",
		"Balances, Tokens, ERC-20, pool shares, and asset routes")
	write_layer_svg(g, active_projections["state"], focus / "state-invariants.svg",
		"Storage, ledgers, pool state, guards, and invariants")
	contract_edges = [edge for edge in active_projections["deployment"] if edge["kind"] in {
		"uses-deployed-contract", "runtime-configures-contract", "proxy-implementation",
		"bytecode-embeds-address", "deployment-aliases-contract", "deployment-step-produces-alias",
		"deployment-step-references-address"}]
	write_layer_svg(g, contract_edges, focus / "deployed-contracts.svg",
		"Runtime, EVM adapters, deployed contracts, and implementations")
	write_interactive_html(g, all_components, output)


def check_coverage(output: Path, thresholds_path: Path) -> None:
	coverage = json.loads((output / "coverage.json").read_text())
	thresholds = json.loads(thresholds_path.read_text())
	failures = []
	for field, minimum in thresholds.get("minimum", {}).items():
		if coverage.get(field) is None or coverage[field] < minimum:
			failures.append(f"{field}={coverage.get(field)} below {minimum}")
	for field, maximum in thresholds.get("maximum", {}).items():
		if coverage.get(field) is None or coverage[field] > maximum:
			failures.append(f"{field}={coverage.get(field)} above {maximum}")
	for field, maximum in thresholds.get("optional_maximum", {}).items():
		if coverage.get(field) is not None and coverage[field] > maximum:
			failures.append(f"{field}={coverage.get(field)} above {maximum}")
	for field, expected in thresholds.get("exact", {}).items():
		if coverage.get(field) != expected:
			failures.append(f"{field}={coverage.get(field)!r} does not equal {expected!r}")
	if failures:
		raise SystemExit("coverage check failed: " + "; ".join(failures))


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
	parser.add_argument("--output", type=Path, default=Path("target/runtime-interaction-graph"))
	parser.add_argument("--rapx-output", type=Path)
	parser.add_argument("--rapx-owner")
	parser.add_argument("--rapx-manifest", type=Path)
	parser.add_argument("--rustc-mir", action="append", default=[], metavar="OWNER=PATH")
	parser.add_argument("--rustc-mir-manifest", type=Path)
	parser.add_argument("--contracts-manifest", type=Path)
	parser.add_argument("--coverage-thresholds", type=Path)
	args = parser.parse_args()
	g = scan(args.root.resolve())
	if args.rapx_output:
		if not args.rapx_owner:
			parser.error("--rapx-owner is required with --rapx-output")
		merge_rapx(g, args.rapx_output, args.rapx_owner)
	if args.rapx_manifest:
		merge_rapx_manifest(g, args.rapx_manifest, args.root.resolve())
	for specification in args.rustc_mir:
		if "=" not in specification:
			parser.error("--rustc-mir must use OWNER=PATH")
		owner_name, mir_path = specification.split("=", 1)
		merge_rustc_mir(g, Path(mir_path), owner_name)
	if args.rustc_mir_manifest:
		merge_rustc_mir_manifest(g, args.rustc_mir_manifest, args.root.resolve())
	if args.contracts_manifest:
		merge_contracts(g, args.contracts_manifest)
	normalize_config_value_calls(g)
	add_state_semantics(g)
	classify_unresolved(g)
	write_outputs(g, args.output)
	if args.coverage_thresholds:
		check_coverage(args.output, args.coverage_thresholds)


if __name__ == "__main__":
	main()
