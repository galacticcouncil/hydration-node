# coretime renewal watchdog

Alerts a Discord webhook when Hydration's (Polkadot, task `2034`) or Basilisk's
(Kusama, task `2090`) bulk coretime cores are at risk of lapsing in the current
sale.

Bulk cores are **not leases** — they must be renewed every ~28-day sale cycle or
they fall back to the open market. This watchdog watches the `broker` pallet on
each coretime chain and pings before the renewal window closes.

## what triggers an alert

An alert fires for a chain only when it is **short of its target core count for
the upcoming region** (i.e. some cores are still un-renewed) **and** the deadline
is near:

| condition | default | severity |
|-----------|---------|----------|
| `< TOTAL_ALERT_DAYS` until the region begins (hard deadline — renewal right is lost after) | 7 days | `URGENT` |
| inside the lead-in period with `< LEADIN_ALERT_DAYS` left in it | 3 days | `WARNING` |

"Short" = `secured cores for next region (workplan) < desired`. The alert lists the
pending `broker.renew(core)` calls (with encoded hex) needed to close the gap.

> Note: the cheapest, priority window to renew is actually the **interlude**
> (before lead-in). These thresholds, as requested, only warn once the lead-in is
> closing / the hard deadline is near — so treat an alert as "renew now, you're
> past the ideal window". Lower the thresholds if you want earlier warnings.

## setup

```sh
cd scripts/coretime-alert
npm install
export DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/XXX/YYY'
node check.mjs            # one-shot check; alerts if within thresholds
```

### useful invocations

```sh
node check.mjs --dry-run   # print the payload instead of POSTing (no webhook needed)
node check.mjs --test      # POST a "webhook reachable" message and exit
node check.mjs --force     # ignore the cooldown and (re)send any active alert
```

## configuration (env vars)

| var | default | meaning |
|-----|---------|---------|
| `DISCORD_WEBHOOK_URL` | — | **required** (unless `--dry-run`) |
| `DISCORD_WEBHOOK_URL_FILE` | — | alternative source: read the webhook from a file (e.g. a mounted secret) |
| `CHECK_INTERVAL_SECONDS` | `3600` | (docker image only) seconds between checks in the built-in loop |
| `LEADIN_ALERT_DAYS` | `3` | warn when this many days are left in the lead-in period |
| `TOTAL_ALERT_DAYS` | `7` | warn when this many days are left until the region begins |
| `ALERT_COOLDOWN_HOURS` | `12` | minimum gap between repeat alerts for the same condition |
| `HYDRATION_DESIRED_CORES` | `3` | target core count for task 2034 |
| `BASILISK_DESIRED_CORES` | `3` | target core count for task 2090 |
| `STATE_FILE` | `./.state.json` | where the throttle state is persisted |

State persists across runs so a standing condition only re-pings every
`ALERT_COOLDOWN_HOURS`; it re-alerts immediately if severity or shortfall changes,
and clears itself once the cores are renewed.

## running it as a service

It's a one-shot check — schedule it. Hourly is plenty (the relevant deadlines move
in days).

### cron

```cron
0 * * * * cd /path/to/hydration-node/scripts/coretime-alert && DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/XXX/YYY' /usr/bin/node check.mjs >> /var/log/coretime-alert.log 2>&1
```

### systemd timer

`/etc/systemd/system/coretime-alert.service`

```ini
[Unit]
Description=Coretime renewal watchdog
After=network-online.target

[Service]
Type=oneshot
WorkingDirectory=/path/to/hydration-node/scripts/coretime-alert
Environment=DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/XXX/YYY
ExecStart=/usr/bin/node check.mjs
```

`/etc/systemd/system/coretime-alert.timer`

```ini
[Unit]
Description=Run coretime renewal watchdog hourly

[Timer]
OnCalendar=hourly
Persistent=true

[Install]
WantedBy=timers.target
```

```sh
systemctl enable --now coretime-alert.timer
```

## docker image / lark deployment

A `Dockerfile` builds `galacticcouncil/coretime-alert`; the image runs the check
on an hourly loop (`CHECK_INTERVAL_SECONDS`).

```sh
docker build -t galacticcouncil/coretime-alert:latest .
docker push galacticcouncil/coretime-alert:latest
# smoke test: one-shot dry-run inside the image
docker run --rm galacticcouncil/coretime-alert:latest node check.mjs --dry-run
```

On the **lark** Swarm it's deployed as the `coretime-alert` stack from
`lark-stack.yml` (single replica, `state` volume, `autoredeploy` on). The webhook
is injected as `DISCORD_WEBHOOK_URL` via `$env` at deploy time and is never stored
in the repo — see the safe-forwarding options below.

### forwarding the webhook safely

`$env:DISCORD_WEBHOOK_URL` resolves against the **swarmpit MCP server's**
environment (defined in `~/.claude.json`), not your interactive shell. Pick one:

- **local deploy file** — copy `lark-stack.yml` outside the repo with the real URL
  inlined (`chmod 600`), and deploy that file. Value never enters the repo or chat.
- **MCP server env** — add `DISCORD_WEBHOOK_URL` to the `swarmpit-lark` `env` block
  in `~/.claude.json`, then restart so the server inherits it.

Do **not** paste the URL into a chat/agent prompt — it ends up in transcripts.
Rotate the webhook in Discord if it ever leaks.

## notes

- Endpoints have built-in fallbacks per chain; a failed assessment posts a (throttled)
  `🔌 check failed` notice so the watchdog never dies silently.
- Renewed cores get **new core indices** each cycle (the assignment to the task is
  preserved), so the listed core numbers change after every renewal — expected.
- Adding another para: append an entry to `CHAINS` in `check.mjs` with its task id,
  relay, and coretime endpoints.
