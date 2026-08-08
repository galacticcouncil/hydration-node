#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import re


MIR_FUNCTION = re.compile(r"^fn\s+(.+?)\(.*\)\s*->\s*.+\s*\{$")
SOURCE_LOCATION = re.compile(r"(?:<impl|\{closure)\s+at\s+([^:]+(?:/[^:]+)*\.rs):(\d+):\d+")
BLOCK = re.compile(r"^bb(\d+):\s*\{")
CALL_TARGET = re.compile(r"(?:^|=\s*)([A-Za-z_<][^=;]*?)\s*\(.*\)\s*->\s*\[")


def symbol_name(symbol: str) -> str:
	without_closures = symbol.split("::{closure", 1)[0]
	name = without_closures.rsplit("::", 1)[-1]
	return name.split("{", 1)[0].strip()


def source_location(symbol: str) -> tuple[str | None, int | None]:
	match = SOURCE_LOCATION.search(symbol)
	return (match.group(1), int(match.group(2))) if match else (None, None)


def instance_id(symbol: str) -> str:
	return hashlib.sha256(symbol.encode()).hexdigest()[:16]


def call_target(statement: str) -> str | None:
	match = CALL_TARGET.search(statement)
	if not match:
		return None
	target = match.group(1).strip()
	if target.startswith(("switchInt", "assert", "drop")):
		return None
	return target


def operation(statement: str) -> dict | None:
	target = call_target(statement)
	if "start_transaction" in statement or "with_transaction" in statement:
		kind = "transaction-start"
	elif "::commit_transaction(" in statement:
		kind = "transaction-commit"
	elif "::rollback_transaction(" in statement:
		kind = "transaction-rollback"
	elif re.search(r"(?:Counted)?Storage(?:Map|NMap|Value|DoubleMap).*::(?:insert|remove|take|put|mutate|try_mutate|append|clear)", statement):
		kind = "storage-write"
	elif re.search(r"(?:Counted)?Storage(?:Map|NMap|Value|DoubleMap).*::(?:get|contains_key|iter)", statement):
		kind = "storage-read"
	elif "hydradx_traits::evm::EVM" in statement and ">::call" in statement:
		kind = "evm-call"
	elif target and re.search(r"::(?:dispatch|try_dispatch)$", target):
		kind = "frame-dispatch"
	elif target and re.search(r"::(?:transfer|deposit|withdraw|mint_into|burn_from)$", target):
		kind = "external-call"
	elif target:
		kind = "call"
	else:
		return None
	return {"kind": kind, "statement": statement, "callee": target}


def successors(statement: str) -> tuple[list[int], list[int]]:
	all_targets = [int(value) for value in re.findall(r"\bbb(\d+)\b", statement)]
	unwind_match = re.search(r"unwind:\s*bb(\d+)", statement)
	unwind = [int(unwind_match.group(1))] if unwind_match else []
	normal = []
	removed_unwind = False
	for target in all_targets:
		if unwind and target == unwind[0] and not removed_unwind:
			removed_unwind = True
			continue
		normal.append(target)
	return list(dict.fromkeys(normal)), unwind


def parse(text: str) -> list[dict]:
	instances = []
	current = None
	current_block = None
	statement_parts: list[str] = []

	def consume_statement() -> None:
		nonlocal statement_parts
		if current is None or current_block is None or not statement_parts:
			statement_parts = []
			return
		statement = " ".join(statement_parts)
		statement_parts = []
		item = operation(statement)
		if item:
			current["blocks"][current_block]["operations"].append(item)
		normal, unwind = successors(statement)
		current["blocks"][current_block]["normal_successors"].extend(normal)
		current["blocks"][current_block]["unwind_successors"].extend(unwind)

	def finish() -> None:
		nonlocal current, current_block
		consume_statement()
		if current is not None:
			for block in current["blocks"].values():
				block["normal_successors"] = list(dict.fromkeys(block["normal_successors"]))
				block["unwind_successors"] = list(dict.fromkeys(block["unwind_successors"]))
			instances.append(current)
		current = None
		current_block = None

	for raw_line in text.splitlines():
		function = MIR_FUNCTION.match(raw_line)
		if function:
			finish()
			symbol = function.group(1)
			file, impl_line = source_location(symbol)
			current = {"id": instance_id(symbol), "symbol": symbol, "name": symbol_name(symbol),
				"source_file": file, "impl_line": impl_line, "blocks": {}}
			continue
		if current is None:
			continue
		block = BLOCK.match(raw_line.strip())
		if block:
			consume_statement()
			current_block = int(block.group(1))
			current["blocks"][current_block] = {"operations": [], "normal_successors": [],
				"unwind_successors": []}
			continue
		if raw_line == "}":
			finish()
			continue
		if current_block is None:
			continue
		line = raw_line.strip()
		if not line or line == "}":
			continue
		statement_parts.append(line)
		if line.endswith(";"):
			consume_statement()
	finish()
	return instances


def order_flags(blocks: dict[int, dict], successor_kind: str = "normal_successors") -> tuple[bool, bool]:
	before = after = False
	queue = [(0, False, False)]
	seen = set()
	while queue:
		block_id, wrote, external = queue.pop(0)
		state = (block_id, wrote, external)
		if state in seen or block_id not in blocks:
			continue
		seen.add(state)
		for item in blocks[block_id]["operations"]:
			if item["kind"] in {"external-call", "evm-call", "frame-dispatch"}:
				before |= wrote
				external = True
			elif item["kind"] == "storage-write":
				after |= external
				wrote = True
		for target in blocks[block_id].get(successor_kind, []):
			queue.append((target, wrote, external))
	return before, after
