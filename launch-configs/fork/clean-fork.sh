#!/usr/bin/env bash
# Clear zombienet's per-run artifacts under data/ so a restart picks up the
# updated chainspec / freshly injected wasm. Keeps the source files
# (data/forked-chainspec.json, data/state.json, the *.yaml templates,
# zombie-wrapper.sh).
set -euo pipefail
cd "$(dirname "$0")"

rm -rf \
	data/alice data/alice-1 \
	data/bob data/bob-1 \
	data/charlie data/dave \
	data/cfg data/2034 \
	data/temp data/temp-1 data/temp-collator \
	data/export-genesis-state data/zombie.json \
	data/local-2034-rococo-local*.json data/rococo-local*.json \
	data/logs data/namespace data/finished.txt \
	data/*.log

echo "cleaned data/ — keeping forked-chainspec.json, state.json, *.yaml"
