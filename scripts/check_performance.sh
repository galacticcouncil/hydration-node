#!/bin/sh

# Need to run from the top-level node directory
[ -d ".maintain" ] || { echo "This script must be executed from the top level node directory"; exit 1; }

# Need python3
command -v python3 >/dev/null 2>&1 || { echo "python3 required. Please install first"; exit 1; }

# Run the check
python3 .maintain/bench-check/bench_check.py $*
