#!/usr/bin/env bash

# Write the JS code to a file in the ephemeral environment
cat <<EOF > /tmp/query_asset_state.bundle.js 
console.log("Sending normal transaction...");

EOF

node /tmp/query_asset_state.bundle.js
if [ $? -ne 0 ]; then
  echo "Failed to query asset state"
  exit 1
fi
exit 0
