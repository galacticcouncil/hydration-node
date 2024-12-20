#!/usr/bin/env bash

# Write the JS code to a file in the ephemeral environment
cat <<EOF > /tmp/send_normal_tx.js
console.log("Sending normal transaction...");
// ... rest of your JS code here ...
EOF

# Now run it
node /tmp/send_normal_tx.js
