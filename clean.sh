#!/bin/bash

echo "Searching for running cargo processes..."
# Find all cargo processes
CARGO_PROCESSES=$(ps aux | grep "[c]argo" | awk '{print $2}')

if [ -z "$CARGO_PROCESSES" ]; then
    echo "No running cargo processes found."
else
    echo "Killing the following cargo processes:"
    echo "$CARGO_PROCESSES"
    # Kill all cargo processes
    echo "$CARGO_PROCESSES" | xargs kill -9
    echo "All cargo processes terminated."
fi

echo "Removing cargo lock files..."
# Find and remove any lock files in the target directories
find . -type f -name ".cargo-lock" -exec rm -f {} \;
find . -type f -name ".crates.lock" -exec rm -f {} \;

echo "Clearing cargo target directory locks..."
# Check and remove lingering locks in target directories
find . -type f -name ".lock" -exec rm -f {} \;

echo "Done. You can now run cargo commands without interference."

