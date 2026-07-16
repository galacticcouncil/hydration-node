#!/usr/bin/env python3

import argparse
import json
import os
import subprocess
from pathlib import Path

from analysis_provenance import collector_provenance, command_fingerprint, file_sha256, reusable_artifact


ANALYSIS_MARKERS = {"callgraph": "CallGraph:", "mir": "MIR:", "dataflow": "DataFlow:"}
TOOLCHAIN = "nightly-2025-12-06"


def analysis_command(package: dict, analysis: str, timeout: int) -> list[str]:
	return ["cargo", f"+{TOOLCHAIN}", "rapx", "--timeout", str(timeout),
		"analyze", analysis, "--", "-p", package["package"], "--lib", "--features", "std", "--locked"]


def workspace_packages(root: Path) -> list[dict]:
	result = subprocess.run(
		["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
		cwd=root,
		check=True,
		text=True,
		capture_output=True,
	)
	packages = []
	for package in json.loads(result.stdout)["packages"]:
		manifest = Path(package["manifest_path"])
		relative = manifest.relative_to(root).as_posix()
		if not relative.startswith(("pallets/", "precompiles/", "runtime/hydradx/")):
			continue
		parts = relative.split("/")
		if parts[0] in {"pallets", "precompiles"} and len(parts) != 3:
			continue
		if parts[0] == "runtime" and relative != "runtime/hydradx/Cargo.toml":
			continue
		if parts[0] == "pallets":
			owner = f"pallet:{parts[1]}"
		elif parts[0] == "precompiles":
			owner = f"precompile:{parts[1]}"
		else:
			owner = "runtime:hydradx"
		packages.append({"package": package["name"], "owner": owner, "manifest": relative})
	return sorted(packages, key=lambda item: item["package"])


def run(root: Path, output: Path, analyses: list[str], timeout: int, package_filter: set[str]) -> dict:
	output.mkdir(parents=True, exist_ok=True)
	environment = os.environ.copy()
	environment["RAPX_CLEAN"] = "false"
	environment["CXXFLAGS"] = "-include cstdint"
	environment.pop("RUSTFLAGS", None)
	manifest_path = output / "manifest.json"
	previous = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
	provenance = collector_provenance(root, Path(__file__), TOOLCHAIN,
		[Path(__file__).with_name("analysis_provenance.py")])
	selected = {package["package"]: package for package in workspace_packages(root)
		if not package_filter or package["package"] in package_filter}
	existing = {package["package"]: package for package in previous.get("packages", [])
		if package["package"] in selected}
	manifest = {
		"schema_version": 2,
		"tool": "rapx",
		"toolchain": TOOLCHAIN,
		"provenance": provenance,
		"requested_packages": sorted(selected),
		"requested_analyses": sorted(analyses),
		"timeout_seconds": timeout,
		"packages": [],
	}
	for package in selected.values():
		entry = existing.get(package["package"], dict(package))
		entry.setdefault("analyses", {})
		entry["analyses"] = {name: value for name, value in entry["analyses"].items() if name in analyses}
		for analysis in analyses:
			path = output / f"{package['package']}.{analysis}.txt"
			command = analysis_command(package, analysis, timeout)
			fingerprint = command_fingerprint(provenance, {**package, "analysis": analysis}, command)
			previous_analysis = entry["analyses"].get(analysis)
			cache_entry = ({"status": previous_analysis.get("status"),
				"input_fingerprint": previous_analysis.get("input_fingerprint"),
				"artifact_sha256": previous_analysis.get("artifact_sha256")}
				if previous_analysis else None)
			if reusable_artifact(cache_entry, fingerprint, path):
				continue
			try:
				with path.open("w") as stream:
					completed = subprocess.run(command, cwd=root, env=environment, text=True,
						stdout=stream, stderr=subprocess.STDOUT, timeout=timeout + 30)
				status = "ok" if completed.returncode == 0 else "failed"
				if status == "ok" and ANALYSIS_MARKERS[analysis] not in path.read_text(errors="replace"):
					status = "invalid-output"
				result = {"path": path.relative_to(output).as_posix(), "status": status,
					"returncode": completed.returncode, "command": command, "input_fingerprint": fingerprint}
				if path.is_file() and path.stat().st_size:
					result["artifact_sha256"] = file_sha256(path)
				entry["analyses"][analysis] = result
			except subprocess.TimeoutExpired:
				entry["analyses"][analysis] = {"path": path.relative_to(output).as_posix(),
					"status": "timeout", "command": command, "input_fingerprint": fingerprint}
		existing[package["package"]] = entry
		manifest["packages"] = sorted(existing.values(), key=lambda item: item["package"])
		manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
	manifest["packages"] = sorted(existing.values(), key=lambda item: item["package"])
	manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
	return manifest


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
	parser.add_argument("--output", type=Path, default=Path("target/runtime-interaction-graph/rapx"))
	parser.add_argument("--analysis", action="append", choices=["callgraph", "mir", "dataflow"], default=[])
	parser.add_argument("--package", action="append", default=[])
	parser.add_argument("--timeout", type=int, default=300)
	args = parser.parse_args()
	analyses = args.analysis or ["callgraph"]
	run(args.root.resolve(), args.output.resolve(), analyses, args.timeout, set(args.package))


if __name__ == "__main__":
	main()
