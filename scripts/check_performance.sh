#!/bin/bash

# Need to run from the top-level node directory
[ -d ".maintain" ] || { echo "This script must be executed from the top level node directory"; exit 1; }

echo "HydraDX node - Simple Performance check"
echo "---------------------------------------"

echo
echo "Prerequisites"

echo -n "Python version >= 3.8 ..... "

# Need python3
if command -v python3.8 >/dev/null 2>&1
then
  PYTHON=python3.8
else
  PYTHON=python3
fi
command -v $PYTHON >/dev/null 2>&1 || { echo "python3 required. Please install first"; exit 1; }

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

EXPECTED_BENCHWIZARD_VERSION="0.2.1"

echo -n "benchwizard >= $EXPECTED_BENCHWIZARD_VERSION ..... "

$PYTHON -m bench_wizard >/dev/null 2>&1 || {
  echo "benchwizard required. benchwizard is cli tool developed by HydraDX dev to streamline substrate benchmark process.";
  echo "Installation: pip3 install bench-wizard";
  exit 1;
  }

CURRENT_BENCH_VERSION=`$PYTHON -m bench_wizard version | tr -d '\n'`

if [[ $EXPECTED_BENCHWIZARD_VERSION > $CURRENT_BENCH_VERSION ]]
then
	echo "Please upgrade benchwizard (current version $CURRENT_BENCH_VERSION): pip3 install bench-wizard --upgrade";
  exit 1;
fi

echo
echo

# Run the check
$PYTHON -m bench_wizard benchmark -pc $*
