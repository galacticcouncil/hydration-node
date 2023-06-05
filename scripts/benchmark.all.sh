#!/bin/bash

pallets=("frame-system:system"
"pallet-balances:balances"
"pallet-collator-selection:collator_selection"
"pallet-timestamp:timestamp"
"pallet-democracy:democracy"
"pallet-treasury:treasury"
"pallet-scheduler:scheduler"
"pallet-utility:utility"
"pallet-identity:identity"
"pallet-tips:tips"
"pallet-proxy:proxy"
"council:council"
"tech:technical_committee"
"pallet-xcm:xcm"
"cumulus-pallet-xcmp-queue:xcmp_queue"
"pallet-currencies:currencies"
"orml-tokens:tokens"
"orml-vesting:vesting"
"pallet-duster:duster"
"pallet-transaction-multi-payment:payment"
"pallet-omnipool:omnipool"
"pallet-omnipool-liquidity-mining:omnipool_lm"
"pallet-circuit-breaker:circuit_breaker"
"pallet-claims:claims"
"pallet-transaction-pause:transaction_pause"
"pallet-dca:dca"
"pallet-asset-registry:registry"
"pallet-ema-oracle:ema_oracle"
"pallet-otc:otc"
"pallet-route-executor:route_executor"
)

command="cargo run --bin hydradx --release --features=runtime-benchmarks -- benchmark pallet --pallet=[pallet] --extra --chain=dev --extrinsic='*' --steps=5 --repeat=20 --output [output].rs --template .maintain/pallet-weight-template-no-back.hbs"

for string in "${pallets[@]}"; do

  IFS=':' read -ra subvalues <<< "$string"

  pallet="${subvalues[0]}"
  output="${subvalues[1]}"

  echo "Running benchmark for ${pallet}"

  replaced_command="${command/\[pallet\]/$pallet}"
  replaced_command="${replaced_command/\[output\]/$output}"

  eval "$replaced_command"
done
