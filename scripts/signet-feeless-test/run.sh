#!/usr/bin/env bash
# Launch a Chopsticks fork with the local runtime wasm, run the e2e suite, tear down.
# Prereqs: build the wasm + `yarn install` (see README).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WASM="$ROOT/target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm"
CHOPLOG="$(mktemp -t chopsticks-signet.XXXXXX.log)"
PORT=8000

if [ ! -f "$WASM" ]; then
  echo "missing runtime wasm: $WASM"
  echo "build it first: (cd $ROOT && cargo build --release -p hydradx-runtime)"
  exit 1
fi

echo "[run] freeing port $PORT if held..."
lsof -ti "tcp:$PORT" 2>/dev/null | xargs -r kill -9 2>/dev/null || true

echo "[run] launching chopsticks (fork hydradx mainnet, wasm-override, db-less)..."
( cd "$ROOT" && npx -y @acala-network/chopsticks@latest \
    --endpoint=wss://rpc.hydradx.cloud \
    --wasm-override "$WASM" \
    --mock-signature-host \
    --build-block-mode Manual \
    --port "$PORT" ) > "$CHOPLOG" 2>&1 &
CHOPID=$!
trap 'kill $CHOPID 2>/dev/null; lsof -ti "tcp:'"$PORT"'" 2>/dev/null | xargs -r kill -9 2>/dev/null; wait $CHOPID 2>/dev/null' EXIT

echo "[run] waiting for chopsticks to listen (log: $CHOPLOG)..."
READY=0
for i in $(seq 1 90); do
  if grep -qi "listening" "$CHOPLOG" 2>/dev/null; then READY=1; break; fi
  if ! kill -0 "$CHOPID" 2>/dev/null; then echo "[run] chopsticks exited early:"; tail -30 "$CHOPLOG"; exit 1; fi
  sleep 2
done
[ "$READY" = "1" ] || { echo "[run] chopsticks not ready; log:"; tail -30 "$CHOPLOG"; exit 1; }
echo "[run] chopsticks listening after ~$((i*2))s"

echo "[run] running jest..."
cd "$HERE"
WS_URL="ws://localhost:$PORT" ./node_modules/.bin/jest --runInBand --forceExit --verbose
