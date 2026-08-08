#!/usr/bin/env python3

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


RUNTIME_ENTRY = re.compile(
	r"(?m)^\s*(?P<alias>[A-Z][A-Za-z0-9_]*)\s*:\s*"
	r"(?P<crate>[a-zA-Z0-9_]+)(?:::<(?P<instance>[^>]+)>)?"
	r"(?P<options>[^=\n]*)=\s*(?P<index>\d+)\s*,"
)
H160_HEX = re.compile(
	r"pub\s+const\s+(?P<name>[A-Z][A-Z0-9_]*)\s*:\s*H160\s*=\s*H160\(hex!\(\"(?P<hex>[0-9a-fA-F]{40})\"\)\)"
)
H160_ADDR = re.compile(
	r"pub\s+const\s+(?P<name>[A-Z][A-Z0-9_]*)\s*:\s*H160\s*=\s*addr\((?P<value>\d+)\)"
)
INNER_CFG = re.compile(r"(?m)^#!\s*\[\s*cfg\s*\((?P<expression>[^\]]+)\)\s*\]")
MODULE_DECLARATION = re.compile(
	r"(?m)(?P<attributes>(?:^#\s*\[[^\]\n]+\][ \t]*\n)*)"
	r"^(?:pub(?:\([^\n)]*\))?\s+)?mod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*;",
)
OUTER_CFG = re.compile(r"#\s*\[\s*cfg\s*\((?P<expression>[^\]]+)\)\s*\]")
CFG_TOKEN = re.compile(
	r'\s*(?:(?P<identifier>[A-Za-z_][A-Za-z0-9_-]*)|(?P<string>"(?:\\.|[^"\\])*")|(?P<symbol>[(),=]))'
)
INACTIVE_ANALYSIS_FEATURES = {"runtime-benchmarks", "test-utils", "testing"}


class CfgParser:
	def __init__(self, expression: str) -> None:
		self.tokens = []
		offset = 0
		while offset < len(expression):
			if not expression[offset:].strip():
				break
			match = CFG_TOKEN.match(expression, offset)
			if not match:
				self.tokens = []
				break
			self.tokens.append(match.group("identifier") or match.group("string") or match.group("symbol"))
			offset = match.end()
		self.index = 0

	def parse(self) -> bool | None:
		value = self.predicate()
		return value if self.index == len(self.tokens) else None

	def predicate(self) -> bool | None:
		if self.index >= len(self.tokens) or self.tokens[self.index].startswith('"'):
			return None
		name = self.tokens[self.index]
		self.index += 1
		if self.consume("="):
			if self.index >= len(self.tokens) or not self.tokens[self.index].startswith('"'):
				return None
			value = json.loads(self.tokens[self.index])
			self.index += 1
			return False if name == "feature" and value in INACTIVE_ANALYSIS_FEATURES else None
		if not self.consume("("):
			return False if name == "test" else None
		values = []
		if not self.consume(")"):
			while True:
				values.append(self.predicate())
				if self.consume(")"):
					break
				if not self.consume(","):
					return None
		if name == "all":
			return False if False in values else (True if all(value is True for value in values) else None)
		if name == "any":
			return True if True in values else (False if all(value is False for value in values) else None)
		if name == "not" and len(values) == 1:
			return None if values[0] is None else not values[0]
		return None

	def consume(self, token: str) -> bool:
		if self.index >= len(self.tokens) or self.tokens[self.index] != token:
			return False
		self.index += 1
		return True


def cfg_value(expression: str) -> bool | None:
	return CfgParser(expression).parse()


def cfg_attributes_value(attributes: str, pattern: re.Pattern[str]) -> bool | None:
	values = [cfg_value(match.group("expression")) for match in pattern.finditer(attributes)]
	if False in values:
		return False
	return True if values and all(value is True for value in values) else None


def module_source_path(parent: Path, name: str) -> Path | None:
	base = parent.parent if parent.name in {"lib.rs", "main.rs", "mod.rs"} else parent.parent / parent.stem
	candidates = (base / f"{name}.rs", base / name / "mod.rs")
	matches = [candidate for candidate in candidates if candidate.is_file()]
	return matches[0] if len(matches) == 1 else None


def active_external_sources(source_root: Path) -> list[Path]:
	paths = sorted(source_root.rglob("*.rs"))
	declarations: dict[Path, list[tuple[Path, bool | None]]] = {}
	inner_cfg = {}
	for parent in paths:
		text = parent.read_text(errors="replace")
		inner_cfg[parent.resolve()] = cfg_attributes_value(text, INNER_CFG)
		for declaration in MODULE_DECLARATION.finditer(text):
			child = module_source_path(parent, declaration.group("name"))
			if child is None:
				continue
			value = cfg_attributes_value(declaration.group("attributes"), OUTER_CFG)
			if value is None and not declaration.group("attributes"):
				value = True
			declarations.setdefault(child.resolve(), []).append((parent.resolve(), value))

	def potentially_active(path: Path, visiting: set[Path]) -> bool:
		path = path.resolve()
		if inner_cfg[path] is False:
			return False
		parents = declarations.get(path)
		if not parents or path in visiting:
			return True
		return any(state is not False and potentially_active(parent, visiting | {path})
			for parent, state in parents)

	return [path for path in paths if potentially_active(path, set())]


def construct_runtime_entries(text: str) -> list[dict]:
	entries = []
	for match in RUNTIME_ENTRY.finditer(text):
		options = match.group("options")
		excluded = []
		exclude = re.search(r"exclude_parts\s*\{([^}]*)\}", options)
		if exclude:
			excluded = re.findall(r"[A-Z][A-Za-z0-9_]*", exclude.group(1))
		entries.append({
			"alias": match.group("alias"),
			"crate": match.group("crate"),
			"instance": match.group("instance"),
			"index": int(match.group("index")),
			"excluded_parts": excluded,
		})
	return entries


def _h160(value: int) -> str:
	return "0x" + value.to_bytes(20, "big").hex()


def precompile_inventory(text: str) -> list[dict]:
	addresses = {match.group("name"): "0x" + match.group("hex").lower() for match in H160_HEX.finditer(text)}
	addresses.update({match.group("name"): _h160(int(match.group("value"))) for match in H160_ADDR.finditer(text)})
	result = []
	for constant, address in sorted(addresses.items(), key=lambda item: item[1]):
		condition = re.search(rf"(?:if|else\s+if)\s+address\s*==\s*{re.escape(constant)}\s*\{{", text)
		if not condition:
			continue
		end = text.find("} else", condition.end())
		branch = text[condition.end():end if end >= 0 else condition.end() + 1000]
		execute = re.search(r"([a-zA-Z0-9_:<>.,\s]+?)::execute\s*\(", branch)
		target = re.sub(r"\s+", "", execute.group(1)).split("Some(")[-1] if execute else None
		result.append({"route": constant.lower().replace("_", "-"), "constant": constant,
			"address": address, "target": target, "dynamic": False})
	for predicate, target, route in (
		("is_asset_address", "MultiCurrencyPrecompile", "asset-address"),
		("is_oracle_address", "ChainlinkOraclePrecompile", "oracle-address"),
	):
		if re.search(rf"else\s+if\s+{predicate}\(address\)", text):
			result.append({"route": route, "predicate": predicate, "target": target, "dynamic": True})
	return result


def cargo_runtime_dependencies(root: Path) -> dict[str, dict]:
	completed = subprocess.run(
		["cargo", "metadata", "--format-version", "1", "--locked"],
		cwd=root,
		check=True,
		text=True,
		capture_output=True,
	)
	metadata = json.loads(completed.stdout)
	packages = {package["id"]: package for package in metadata["packages"]}
	runtime = next(package for package in packages.values()
		if Path(package["manifest_path"]).resolve() == (root / "runtime/hydradx/Cargo.toml").resolve())
	runtime_node = next(node for node in metadata["resolve"]["nodes"] if node["id"] == runtime["id"])
	return {
		dependency["name"]: {
			"package": packages[dependency["pkg"]]["name"],
			"version": packages[dependency["pkg"]]["version"],
			"manifest": packages[dependency["pkg"]]["manifest_path"],
			"source": packages[dependency["pkg"]].get("source"),
		}
		for dependency in runtime_node["deps"]
	}


def runtime_source_inventory(root: Path, entries: list[dict]) -> list[dict]:
	dependencies = cargo_runtime_dependencies(root)
	result = []
	for entry in entries:
		dependency = dependencies.get(entry["crate"])
		if not dependency:
			continue
		manifest = Path(dependency["manifest"])
		source_root = manifest.parent / "src"
		if not source_root.is_dir():
			continue
		result.append({**entry, **dependency, "source_root": source_root.as_posix(),
			"external": not manifest.is_relative_to(root)})
	return result
