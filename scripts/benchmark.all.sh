#!/bin/bash

pallets=(
"pallet-balances:balances"
"pallet-bonds:bonds"
"pallet-circuit-breaker:circuit_breaker"
"pallet-claims:claims"
"pallet-collator-selection:collator_selection"
"council:council"
"pallet-currencies:currencies"
"pallet-dca:dca"
"pallet-democracy:democracy"
"pallet-duster:duster"
"pallet-ema-oracle:ema_oracle"
"pallet-identity:identity"
"pallet-lbp:lbp"
"pallet-omnipool:omnipool"
"pallet-omnipool-liquidity-mining:omnipool_lm"
"pallet-otc:otc"
"pallet-transaction-multi-payment:payment"
"pallet-preimage:preimage"
"pallet-proxy:proxy"
"pallet-asset-registry:registry"
"pallet-route-executor:route_executor"
"pallet-scheduler:scheduler"
"pallet-stableswap:stableswap"
"pallet-staking:staking"
"frame-system:system"
"tech:technical_committee"
"pallet-timestamp:timestamp"
"orml-tokens:tokens"
"pallet-transaction-pause:transaction_pause"
"pallet-treasury:treasury"
"pallet-utility:utility"
"orml-vesting:vesting"
"pallet-xcm:xcm"
"cumulus-pallet-xcmp-queue:xcmp_queue"
"pallet-xyk:xyk"
"pallet-referrals:referrals"
)

command="cargo run --bin hydradx --release --features=runtime-benchmarks -- benchmark pallet --pallet=[pallet] --wasm-execution=compiled --heap-pages=4096 --chain=dev --extrinsic='*' --steps=5 --repeat=20 --output [output].rs --template .maintain/pallet-weight-template-no-back.hbs"

for string in "${pallets[@]}"; do

  IFS=':' read -ra subvalues <<< "$string"

  pallet="${subvalues[0]}"
  output="${subvalues[1]}"

  echo "Running benchmark for ${pallet}"

  replaced_command="${command/\[pallet\]/$pallet}"
  replaced_command="${replaced_command/\[output\]/$output}"

  eval "$replaced_command"
done
