#!/usr/bin/env python3

import argparse
import json
import os
import subprocess
from pathlib import Path

from analysis_provenance import collector_provenance, command_fingerprint, file_sha256, reusable_artifact
from collect_rapx import workspace_packages


DEFAULT_PACKAGES = {"pallet-hsm", "pallet-omnipool", "pallet-route-executor", "pallet-stableswap"}
TOOLCHAIN = "nightly-2025-12-06"


def mir_command(package: dict) -> list[str]:
	return ["cargo", f"+{TOOLCHAIN}", "rustc", "-p", package["package"], "--lib",
		"--features", "std", "--locked", "--", "-Zunpretty=mir"]


def failure_reason(log: Path) -> str:
	text = log.read_text(errors="replace")
	if any(marker in text for marker in ("to_substrate_wasm_fn_return_value", "cumulus-primitives-proof-size-hostfunction",
		"could not compile `staging-xcm`", "could not compile `pallet-democracy`")):
		return "pinned-nightly-dependency-incompatibility"
	if "could not compile" in text:
		return "compile-error"
	return "unknown"


def run(root: Path, output: Path, packages: set[str], timeout: int, force: bool = False) -> dict:
	output.mkdir(parents=True, exist_ok=True)
	environment = os.environ.copy()
	environment["CXXFLAGS"] = "-include cstdint"
	environment.pop("RUSTFLAGS", None)
	manifest_path = output / "manifest.json"
	previous = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
	provenance = collector_provenance(root, Path(__file__), TOOLCHAIN, [
		Path(__file__).with_name("analysis_provenance.py"),
		Path(__file__).with_name("collect_rapx.py"),
	])
	selected = {package["package"]: package for package in workspace_packages(root)
		if package["package"] in packages}
	existing = {package["package"]: package for package in previous.get("packages", [])
		if package["package"] in selected}
	for entry in existing.values():
		if entry.get("status") == "failed" and entry.get("log"):
			log = output / entry["log"]
			if log.exists():
				entry["failure_reason"] = failure_reason(log)
	manifest = {
		"schema_version": 2,
		"tool": "rustc-mir",
		"toolchain": TOOLCHAIN,
		"provenance": provenance,
		"requested_packages": sorted(selected),
		"timeout_seconds": timeout,
		"packages": [],
	}
	workspace = sorted(selected.values(), key=lambda item: (item["owner"] == "runtime:hydradx", item["package"]))
	for package in workspace:
		artifact = output / f"{package['package']}.mir"
		log = output / f"{package['package']}.log"
		command = mir_command(package)
		fingerprint = command_fingerprint(provenance, package, command)
		if not force and reusable_artifact(existing.get(package["package"]), fingerprint, artifact):
			continue
		entry = dict(package)
		try:
			with artifact.open("w") as stdout, log.open("w") as stderr:
				completed = subprocess.run(command, cwd=root, env=environment, text=True, stdout=stdout,
					stderr=stderr, timeout=timeout)
			entry.update({"status": "ok" if completed.returncode == 0 and artifact.stat().st_size else "failed",
				"returncode": completed.returncode, "artifact": artifact.name, "log": log.name,
				"command": command, "input_fingerprint": fingerprint})
			if artifact.is_file() and artifact.stat().st_size:
				entry["artifact_sha256"] = file_sha256(artifact)
			if entry["status"] == "failed":
				entry["failure_reason"] = failure_reason(log)
		except subprocess.TimeoutExpired:
			entry.update({"status": "timeout", "artifact": artifact.name, "log": log.name,
				"command": command, "input_fingerprint": fingerprint})
		existing[package["package"]] = entry
		manifest["packages"] = sorted(existing.values(), key=lambda item: item["package"])
		manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
	manifest["packages"] = sorted(existing.values(), key=lambda item: item["package"])
	manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
	return manifest


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
	parser.add_argument("--output", type=Path, default=Path("target/runtime-interaction-graph/mir"))
	parser.add_argument("--package", action="append", default=[])
	parser.add_argument("--all", action="store_true")
	parser.add_argument("--force", action="store_true")
	parser.add_argument("--timeout", type=int, default=900)
	args = parser.parse_args()
	available = {package["package"] for package in workspace_packages(args.root.resolve())}
	packages = available if args.all else (set(args.package) or DEFAULT_PACKAGES)
	run(args.root.resolve(), args.output.resolve(), packages, args.timeout, args.force)


if __name__ == "__main__":
	main()
