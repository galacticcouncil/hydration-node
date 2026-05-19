# Circuit Breaker Dashboard

Single-page dashboard showing live circuit-breaker state on Hydration.

Displays:

- **Global withdraw limit** — total egress cap per time window, current accumulator with decay, lockdown status
- **Per-asset deposit lockdown** — lockdown state, issuance baseline, current issuance, increase since window start
- **Asset classification** — local vs external from `globalAssetOverrides`

## Local use

Open `index.html` in a browser. It connects to `wss://rpc.hydradx.cloud` by default. Change the RPC URL in the input field to point at a fork (e.g. `wss://node5.lark.hydration.cloud`) and click Reconnect.

Or serve it with any static file server:

```bash
python3 -m http.server 8080
# open http://localhost:8080
```

## Deploy to lark (Docker Swarm)

The HTML is stored as a Swarm config. Create the config first, then deploy the stack:

```bash
# Create/update the config from the HTML file
docker config create circuit_breaker_dashboard index.html

# Deploy the stack
docker stack deploy -c stack.yml circuit-breaker
```

To update after changing `index.html`, re-create the config with a new name, update `stack.yml` to reference it, and redeploy (Docker configs are immutable once created).

Exposed at `https://circuit-breaker.lark.hydration.cloud` via traefik.

### Via Swarmpit MCP

```
mcp__swarmpit-lark__create_config(configName="circuit_breaker_dashboard", data="<index.html contents>")
mcp__swarmpit-lark__create_stack(name="circuit-breaker", compose="<stack.yml contents>")
```

## Notes

- Auto-refreshes every 6s (one block)
- Deposit limit values are set via runtime trait, not storage — dashboard shows issuance increase since window start, not % of limit (limit not exposed on-chain)
- Global withdraw limit uses linear decay over the window
