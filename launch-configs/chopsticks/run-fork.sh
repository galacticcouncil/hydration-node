#!/usr/bin/env bash
# One-shot: boot the chopsticks lark fork, run $1 (a probe/deploy script via curl
# or node), then tear down. Designed to run as a single background task because
# the agent harness reaps daemons left between tool calls.
set -uo pipefail
cd /home/mrq/git/hydration-node/launch-configs/chopsticks
pkill -f "chopsticks" 2>/dev/null; pkill -f "chopsticks.cjs" 2>/dev/null; sleep 1
rm -f /tmp/chopf.log
# GC fork of chopsticks (adds Frontier eth RPC). Falls back to local acala build.
GC=/home/mrq/git/chopsticks/packages/chopsticks/chopsticks.cjs
if [ -f "$GC" ]; then node "$GC" --config propeller-lark.yml > /tmp/chopf.log 2>&1 &
else ./node_modules/.bin/chopsticks --config propeller-lark.yml > /tmp/chopf.log 2>&1 & fi
CHOP=$!
ready=0
for i in $(seq 1 100); do
  if grep -qi "listening" /tmp/chopf.log 2>/dev/null; then ready=1; break; fi
  if ! kill -0 "$CHOP" 2>/dev/null; then echo "CHOPSTICKS DIED EARLY"; break; fi
  sleep 3
done
echo "READY=$ready"
echo "UPSTREAM: $(grep -o 'wss://[a-z0-9.-]*' /tmp/chopf.log | sort -u | tr '\n' ' ')"
if [ "$ready" = "1" ]; then
  echo "CHAIN: $(curl -s --max-time 10 -X POST http://127.0.0.1:8011 -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"system_chain","params":[]}')"
  if [ -n "${1:-}" ]; then
    echo "=== running $1 ==="
    bash "$1"
    echo "=== $1 exit $? ==="
  fi
fi
echo "=== chopf.log tail ==="; tail -6 /tmp/chopf.log
kill "$CHOP" 2>/dev/null
echo "TEARDOWN done"
