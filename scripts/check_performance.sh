#!/bin/sh

# Need to run from the top-level node directory
[ -d ".maintain" ] || { echo "This script must be executed from the top level node directory"; exit 1; }

# Need python3
command -v python3 >/dev/null 2>&1 || { echo "python3 required. Please install first"; exit 1; }

if ! python3 -c 'import sys; assert sys.version_info >= (3,8)' > /dev/null 2>&1; then
  echo "Python version 3.8 or higher required."
  exit 1
fi

# Run the check
python3 .maintain/bench-check/bench_check.py $*
