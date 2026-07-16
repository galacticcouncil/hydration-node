#!/usr/bin/env python3

import argparse
import json
from pathlib import Path


def keyed(payload: dict) -> dict:
	return {(item["project"], item["network"], item["address"]): item for item in payload.get("observations", [])}


def diff(before: dict, after: dict) -> dict:
	left, right = keyed(before), keyed(after)
	changed = []
	for key in sorted(left.keys() & right.keys()):
		fields = {field: {"before": left[key].get(field), "after": right[key].get(field)}
			for field in ("has_code", "bytecode_sha256", "implementation", "embedded_addresses")
			if left[key].get(field) != right[key].get(field)}
		if fields:
			changed.append({"project": key[0], "network": key[1], "address": key[2], "changes": fields})
	return {"before": before.get("rpc_snapshot"), "after": after.get("rpc_snapshot"),
		"added": [right[key] for key in sorted(right.keys() - left.keys())],
		"removed": [left[key] for key in sorted(left.keys() - right.keys())], "changed": changed}


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("before", type=Path)
	parser.add_argument("after", type=Path)
	parser.add_argument("--output", type=Path)
	args = parser.parse_args()
	result = json.dumps(diff(json.loads(args.before.read_text()), json.loads(args.after.read_text())), indent=2) + "\n"
	if args.output:
		args.output.write_text(result)
	else:
		print(result, end="")


if __name__ == "__main__":
	main()
