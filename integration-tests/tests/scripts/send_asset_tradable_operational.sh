#!/usr/bin/env bash
set -e

# Write the JS code to a file in the ephemeral environment.
# Adjust assetId, desiredState, and PORT as needed here too if you prefer inline substitutions.

cat <<EOF > /tmp/send_asset_tradable_operational.js
$(cat send_asset_tradable_operational.js)
EOF

node /tmp/send_asset_tradable_operational.js
