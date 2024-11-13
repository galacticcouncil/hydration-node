#!/bin/bash
# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "Error: jq is not installed. Please install jq to run this script."
    exit 1
fi

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <input_file> <output_file>"
    exit 1
fi

INPUT_FILE="$1"
OUTPUT_FILE="$2"

NEW_NAME="Hydration Local Testnet"
NEW_ID="local_testnet"
NEW_RELAY_CHAIN="rococo_local_testnet"

# List of keys to delete from genesis.raw.top
KEYS_TO_DELETE=("0x45323df7cc47150b3930e2666b0aa313911a5dd3f1155f5b7d0c5aa102a757f9" "0x45323df7cc47150b3930e2666b0aa3133dca42deb008c6559ee789c9b9f70a2c" "0x45323df7cc47150b3930e2666b0aa313a2bca190d36bd834cc73a38fc213ecbd" "0x111111111111111111")

# List of key-value pairs to update in genesis.raw.top

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <input_file> <output_file>"
    exit 1
fi

INPUT_FILE="$1"
OUTPUT_FILE="$2"

NEW_NAME="Hydration Local Testnet"
NEW_ID="local_testnet"
NEW_RELAY_CHAIN="rococo_local_testnet"

# List of keys to delete from genesis.raw.top
KEYS_TO_DELETE=("0x45323df7cc47150b3930e2666b0aa313911a5dd3f1155f5b7d0c5aa102a757f9" "0x45323df7cc47150b3930e2666b0aa3133dca42deb008c6559ee789c9b9f70a2c" "0x45323df7cc47150b3930e2666b0aa313a2bca190d36bd834cc73a38fc213ecbd" "0x111111111111111111")

# List of key-value pairs to update in genesis.raw.top
GENESIS_UPDATES=("0x57f8dc2f5ab09467896f47300f0424385e0621c4869aa60c02be9adcc98a0d1d:0x08be4f21c56d926b91f020b5071f14935cb93f001f1127c53d3eac6eed23ffea64dc4d79aad5a9d01a359995838830a80733a0bff7e4eb087bfc621ef1873fec49" "0x3c311d57d4daf52904616cf69648081e5e0621c4869aa60c02be9adcc98a0d1d:0x08be4f21c56d926b91f020b5071f14935cb93f001f1127c53d3eac6eed23ffea64dc4d79aad5a9d01a359995838830a80733a0bff7e4eb087bfc621ef1873fec49" "0x15464cac3378d46f113cd5b7a4d71c845579297f4dfb9609e7e4c2ebab9ce40a:0x08be4f21c56d926b91f020b5071f14935cb93f001f1127c53d3eac6eed23ffea64dc4d79aad5a9d01a359995838830a80733a0bff7e4eb087bfc621ef1873fec49" "0xaebd463ed9925c488c112434d61debc0ba7fb8745735dc3be2a2c61a72c39e78:0x04d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d" "0xed25f63942de25ac5253ba64b5eb64d1ba7fb8745735dc3be2a2c61a72c39e78:0x04d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d" "0x26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9de1e86a9a8c739864cf3cc5ec2bea59fd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d:0x000000000000000003000000000000000000e8890423c78a0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080")

# Load the JSON data from the file
JSON_DATA=$(cat "$INPUT_FILE")

# Create a temporary file to hold the modified JSON
TMP_FILE=$(mktemp)

# Step 1: Remove keys to delete from genesis.raw.top
jq \
  --argjson keys_to_delete "$(echo ${KEYS_TO_DELETE[@]} | jq -R -s 'split(" ")')" \
  '.genesis.raw.top |= with_entries(select(.key | IN($keys_to_delete[]) | not))' \
  <<< "$JSON_DATA" > "$TMP_FILE"

# Step 2: Update values in genesis.raw.top based on GENESIS_UPDATES
for update in "${GENESIS_UPDATES[@]}"; do
  key=$(echo "$update" | cut -d':' -f1)
  value=$(echo "$update" | cut -d':' -f2)

  # Update the value for the matching key
  jq --arg key "$key" --arg value "$value" \
    '.genesis.raw.top[$key] = $value' \
    "$TMP_FILE" > "$TMP_FILE.tmp" && mv "$TMP_FILE.tmp" "$TMP_FILE"
done

# Step 3: Update other fields (new_name, new_id, new_relay_chain)
jq \
  --arg new_name "$NEW_NAME" \
  --arg new_id "$NEW_ID" \
  --arg new_relay_chain "$NEW_RELAY_CHAIN" \
  '. |
    .name = $new_name |
    .id = $new_id |
    .relay_chain = $new_relay_chain
  ' \
  "$TMP_FILE" > "$TMP_FILE.tmp" && mv "$TMP_FILE.tmp" "$TMP_FILE"

# Step 4: Save the updated JSON to the output file
cp "$TMP_FILE" "$OUTPUT_FILE"
