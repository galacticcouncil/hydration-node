#!/usr/bin/env bash

# Write the JS code to a file in the ephemeral environment
cat <<EOF > /tmp/send_operational_extrinsic.bundle.js 
console.log("Sending normal transaction...");

EOF

# Now run it
node /tmp/send_operational_extrinsic.bundle.js 