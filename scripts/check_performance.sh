#!/bin/bash

# Need to run from the top-level node directory
[ -d ".maintain" ] || {
  echo "This script must be executed from the top level node directory"
  exit 1
}

echo "HydraDX node - Simple Performance check"
echo "---------------------------------------"

echo
echo "Prerequisites"

echo -n "Python version >= 3.8 ..... "

PYTHON=python3

command -v $PYTHON >/dev/null 2>&1 || {
  echo "python3 required. Please install first"
  exit 1
}

if ! $PYTHON -c 'import sys; assert sys.version_info >= (3,8)' >/dev/null 2>&1; then
  echo "Python version 3.8 or higher required."
  exit 1
fi

echo "OK ($($PYTHON --version))"

echo -n "Toolchain ...... "
TOOLCHAIN=$(rustup show active-toolchain)

if [[ $TOOLCHAIN == "nightly"* ]]; then
  echo "OK ($TOOLCHAIN)"
else
  echo "Nightly toolchain required"
  echo "Current toolchain $TOOLCHAIN"
  exit 1
fi

EXPECTED_BENCHWIZARD_VERSION="0.5.2"

echo -n "benchwizard >= $EXPECTED_BENCHWIZARD_VERSION ..... "

$PYTHON -m bench_wizard >/dev/null 2>&1 || {
  echo "benchwizard required. benchwizard is cli tool developed by HydraDX dev to streamline substrate benchmark process."
  echo "Installation: pip3 install bench-wizard"
  echo
  read -p "Do you want to install it now? [Y/n] " -n 1 -r
  echo # move to a new line
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi

  pip3 install bench-wizard >/dev/null || {
    echo "benchwizard installation failed."
    exit 1
  }
}

CURRENT_BENCH_VERSION=$($PYTHON -m bench_wizard version | tr -d '\n')

if [[ $EXPECTED_BENCHWIZARD_VERSION > $CURRENT_BENCH_VERSION ]]; then
  echo "Please upgrade benchwizard (current version $CURRENT_BENCH_VERSION): pip3 install bench-wizard --upgrade"
  read -p "Do you want to upgrade it now? [Y/n] " -n 1 -r
  echo # move to a new line
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi

  pip3 install bench-wizard --upgrade >/dev/null || {
    echo "benchwizard upgrade failed."
    exit 1
  }
fi

echo "OK ($($PYTHON -m bench_wizard version))"

echo

# Run the check
# shellcheck disable=SC2086
$PYTHON -m bench_wizard pc -p pallet-claims -c local -rf .maintain/bench-check/hydradx-bench-data.json

echo

# Run DB performance check
echo "Running DB disk performance"
if [ ! -d ./substrate ];then
  echo "Cloning substrate ... "
  git clone  --branch=polkadot-v0.9.16 https://github.com/paritytech/substrate.git ./substrate >/dev/null 2>&1
fi
$PYTHON -m bench_wizard db -d ./substrate

echo
