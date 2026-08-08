#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path


SOURCE_ROOTS = ("pallets", "precompiles", "runtime", "traits", "primitives", "math", "integration-tests")
ROOT_INPUTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", ".cargo/config.toml")


def file_sha256(path: Path) -> str:
	digest = hashlib.sha256()
	with path.open("rb") as stream:
		for chunk in iter(lambda: stream.read(1024 * 1024), b""):
			digest.update(chunk)
	return digest.hexdigest()


def source_inputs(root: Path) -> list[Path]:
	paths = [root / relative for relative in ROOT_INPUTS if (root / relative).is_file()]
	for directory in SOURCE_ROOTS:
		base = root / directory
		if not base.is_dir():
			continue
		paths.extend(path for path in base.rglob("*.rs") if path.is_file())
		paths.extend(path for path in base.rglob("Cargo.toml") if path.is_file())
	return sorted(set(paths), key=lambda path: path.relative_to(root).as_posix())


def tree_fingerprint(root: Path) -> dict:
	digest = hashlib.sha256()
	paths = source_inputs(root)
	for path in paths:
		relative = path.relative_to(root).as_posix().encode()
		digest.update(len(relative).to_bytes(4, "big"))
		digest.update(relative)
		with path.open("rb") as stream:
			for chunk in iter(lambda: stream.read(1024 * 1024), b""):
				digest.update(chunk)
	return {"sha256": digest.hexdigest(), "file_count": len(paths)}


def git_commit(root: Path) -> str | None:
	result = subprocess.run(
		["git", "rev-parse", "HEAD"],
		cwd=root,
		text=True,
		capture_output=True,
	)
	return result.stdout.strip() if result.returncode == 0 else None


def tool_input_fingerprint(paths: list[Path]) -> dict:
	files = {}
	for path in sorted({path.resolve() for path in paths}, key=lambda item: item.name):
		name = path.name
		if name in files:
			raise ValueError(f"duplicate tool input name: {name}")
		files[name] = file_sha256(path)
	return {"sha256": tool_input_digest(files), "files": files}


def tool_input_digest(files: dict[str, str]) -> str:
	digest = hashlib.sha256()
	for name, checksum in sorted(files.items()):
		if not isinstance(name, str) or not name or "/" in name or "\\" in name:
			raise ValueError(f"invalid tool input name: {name!r}")
		if not isinstance(checksum, str) or not re.fullmatch(r"[0-9a-f]{64}", checksum):
			raise ValueError(f"invalid tool input checksum: {name}")
		digest.update(len(name.encode()).to_bytes(4, "big"))
		digest.update(name.encode())
		digest.update(bytes.fromhex(checksum))
	return digest.hexdigest()


def valid_tool_input_fingerprint(value: object) -> bool:
	if not isinstance(value, dict) or not isinstance(value.get("files"), dict):
		return False
	try:
		return value.get("sha256") == tool_input_digest(value["files"])
	except ValueError:
		return False


def collector_provenance(root: Path, collector: Path, toolchain: str,
	tool_inputs: list[Path] | None = None) -> dict:
	inputs = [collector, *(tool_inputs or [])]
	return {
		"git_commit": git_commit(root),
		"source_inputs": tree_fingerprint(root),
		"collector_sha256": file_sha256(collector),
		"tool_inputs": tool_input_fingerprint(inputs),
		"toolchain": toolchain,
	}


def command_fingerprint(provenance: dict, package: dict, command: list[str]) -> str:
	payload = {
		"provenance": provenance,
		"package": package,
		"command": command,
	}
	encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
	return hashlib.sha256(encoded).hexdigest()


def reusable_artifact(entry: dict | None, fingerprint: str, artifact: Path) -> bool:
	return bool(
		entry
		and entry.get("status") == "ok"
		and entry.get("input_fingerprint") == fingerprint
		and artifact.is_file()
		and artifact.stat().st_size
		and entry.get("artifact_sha256") == file_sha256(artifact)
	)
