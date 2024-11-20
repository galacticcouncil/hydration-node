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

# Define values for key groups
AURA_AUTHORITIES_VALUE="0x08be4f21c56d926b91f020b5071f14935cb93f001f1127c53d3eac6eed23ffea64dc4d79aad5a9d01a359995838830a80733a0bff7e4eb087bfc621ef1873fec49"
COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE="0x04d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
SYSTEM_ACCOUNT_VALUE="0x000000000000000003000000000000000000e8890423c78a0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080"

# Keys to delete
KEYS_TO_DELETE=(
  "0x45323df7cc47150b3930e2666b0aa313911a5dd3f1155f5b7d0c5aa102a757f9" # ParachainSystem.lastDmqMqcHead
  "0x45323df7cc47150b3930e2666b0aa3133dca42deb008c6559ee789c9b9f70a2c" # ParachainSystem.lastHrmpMqcHeads
  "0x45323df7cc47150b3930e2666b0aa313a2bca190d36bd834cc73a38fc213ecbd" # ParachainSystem.lastRelayChainBlockNumber
  "0x7cda3cfa86b349fdafce4979b197118f948ece45793d7f15c9c0b9574ddbc665" # Elections.CandidateQueue
  "0x7cda3cfa86b349fdafce4979b197118f7657ad2ff3a6742e1071bbb898ce5431" # Elections.Members
  "0x7cda3cfa86b349fdafce4979b197118fba7fb8745735dc3be2a2c61a72c39e78" # Elections.RunnersUp
  "0x7cda3cfa86b349fdafce4979b197118f40982df579bdf1315224f41e5f482063" # Elections.Votes
  "0x5258a12472693b34a3ed25509781e55f3ffefddfbe00a43e565ba6114d1589ea" # Elections.StakeOf
  "0xcec5070d609dd3497f72bde07fc96ba0e0cdd062e6eaf24295ad4ccfc41d4609" # Session.queuedKeys
  "0xcec5070d609dd3497f72bde07fc96ba072763800a36a99fdfc7c10f6415f6ee6" # Session.currentIndex
)

# Key prefixes to delete
PREFIXES_TO_DELETE=(
  "0x7cda3cfa86b349fdafce4979b197118f71cd3068e6118bfb392b798317f63a89" # Elections Specific Voter Entries
  "0x5258a12472693b34a3ed25509781e55fb79" # Elections Additional Stake Mappings
  "0xcec5070d609dd3497f72bde07fc96ba04c014e6bf8b8c2c011e7290b85696bb3" # Session.nextKeys
)

# Keys to replace
REPLACEMENTS=(
  "0x57f8dc2f5ab09467896f47300f0424385e0621c4869aa60c02be9adcc98a0d1d:$AURA_AUTHORITIES_VALUE" # aura.authorities
  "0x3c311d57d4daf52904616cf69648081e5e0621c4869aa60c02be9adcc98a0d1d:$AURA_AUTHORITIES_VALUE" # auraExt.authorities
  "0xcec5070d609dd3497f72bde07fc96ba088dcde934c658227ee1dfafcd6e16903:$AURA_AUTHORITIES_VALUE" # Session validators
  "0x15464cac3378d46f113cd5b7a4d71c845579297f4dfb9609e7e4c2ebab9ce40a:$AURA_AUTHORITIES_VALUE" # CollatorSelection.invulnerables
  "0xaebd463ed9925c488c112434d61debc0ba7fb8745735dc3be2a2c61a72c39e78:$COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE" # Council.members
  "0xed25f63942de25ac5253ba64b5eb64d1ba7fb8745735dc3be2a2c61a72c39e78:$COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE" # TechnicalCommittee.members
  "0x26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9de1e86a9a8c739864cf3cc5ec2bea59fd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d:$SYSTEM_ACCOUNT_VALUE" # System account
)

# Load JSON data
JSON_DATA=$(cat "$INPUT_FILE")

# Remove specific keys using del(.[$key])
for key in "${KEYS_TO_DELETE[@]}"; do
  JSON_DATA=$(jq --arg key "$key" '.genesis.raw.top |= del(.[$key])' <<< "$JSON_DATA")
done

# Remove keys with specified prefixes
for prefix in "${PREFIXES_TO_DELETE[@]}"; do
  JSON_DATA=$(jq --arg prefix "$prefix" \
    '.genesis.raw.top |= with_entries(select(.key | startswith($prefix) | not))' <<< "$JSON_DATA")
done

# Update keys with new values
for replacement in "${REPLACEMENTS[@]}"; do
  key=$(echo "$replacement" | cut -d':' -f1)
  value=$(echo "$replacement" | cut -d':' -f2)
  JSON_DATA=$(jq --arg key "$key" --arg value "$value" \
    '.genesis.raw.top[$key] = $value' <<< "$JSON_DATA")
done

# Update metadata fields
JSON_DATA=$(jq \
  --arg new_name "$NEW_NAME" \
  --arg new_id "$NEW_ID" \
  --arg new_relay_chain "$NEW_RELAY_CHAIN" \
  '.name = $new_name | .id = $new_id | .relay_chain = $new_relay_chain' <<< "$JSON_DATA")

# Save updated JSON to the output file
echo "$JSON_DATA" > "$OUTPUT_FILE"
echo "Chainspec updated successfully and saved to $OUTPUT_FILE"