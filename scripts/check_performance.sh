#!/bin/bash

# Need to run from the top-level node directory
[ -d ".maintain" ] || { echo "This script must be executed from the top level node directory"; exit 1; }

echo "HydraDX node - Simple Performance check"
echo "---------------------------------------"

echo
echo "Prerequisites"

echo -n "Python version >= 3.8 ..... "

# Need python3
command -v python3 >/dev/null 2>&1 || { echo "python3 required. Please install first"; exit 1; }

if ! $PYTHON -c 'import sys; assert sys.version_info >= (3,8)' > /dev/null 2>&1; then
  echo "Python version 3.8 or higher required."
  exit 1
fi

echo "OK"

echo -n "Toolchain ...... "
TOOLCHAIN=`rustup default`

if [[ $TOOLCHAIN = "nightly"* ]]
then
        echo "OK"
else
        echo "Nightly toolchain required"
        echo "Current toolchain $TOOLCHAIN"
        exit 1
fi

echo

# Run the check
$PYTHON .maintain/bench-check/bench_check.py $*
