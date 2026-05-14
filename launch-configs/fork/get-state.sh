#!/bin/bash

# Define state file path
STATE_FILE="./data/state.json"

# Check if state file already exists
if [ -f "$STATE_FILE" ]; then
  echo "State file already exists at $STATE_FILE. Skipping download/scrape."
  exit 0
fi

# Ensure data directory exists
mkdir -p ./data

# Determine which command to run based on environment variables
if [ -n "$STATE_RPC" ]; then
  # STATE_RPC is already an environment variable used by state:scrape
  echo "Using RPC endpoint: $STATE_RPC"

  if [ -n "$STATE_BLOCK" ]; then
    echo "Using block: $STATE_BLOCK"
  fi

  COMMAND="npm run state:scrape"
elif [ -n "$STATE_SOURCE" ]; then
  # STATE_SOURCE is already an environment variable used by state:download
  echo "Using custom source: $STATE_SOURCE"
  COMMAND="npm run state:download"
else
  # Default to scraping with default RPC endpoint
  echo "Using default RPC endpoint"
  COMMAND="npm run state:scrape"
fi

echo "Executing: $COMMAND"

# Execute the command
if eval $COMMAND; then
  if [ -f "$STATE_FILE" ]; then
    echo "Successfully created state file at $STATE_FILE"
  else
    echo "Command completed but state file was not created!"
    exit 1
  fi
else
  echo "Command execution failed"
  exit 1
fi
