#!/usr/bin/env bash


cat <<EOF > /tmp/send_many_normal_txs.bundle.js
console.log("Sending normal transaction...");

EOF
node /tmp/send_many_normal_txs.bundle.js
if [ $? -ne 0 ]; then
  echo "Failed to send many normal txs"
  exit 1
fi
exit 0
