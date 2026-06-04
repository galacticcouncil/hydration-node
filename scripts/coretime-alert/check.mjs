#!/usr/bin/env node
// Coretime renewal watchdog.
//
// For each configured parachain it inspects the broker pallet on the relevant
// coretime chain and decides whether the para's bulk cores are at risk of
// lapsing in the current sale. It alerts a Discord webhook when, with cores
// still un-renewed for the upcoming region, either:
//   * we are inside the lead-in period with < LEADIN_ALERT_DAYS left, or
//   * < TOTAL_ALERT_DAYS remain until the region begins (the hard deadline,
//     after which the renewal right is lost).
//
// Designed to be run periodically (cron / systemd timer). State is persisted so
// a persisting condition only re-pings every ALERT_COOLDOWN_HOURS.

import { ApiPromise, WsProvider } from '@polkadot/api';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

// --- timeslice geometry (identical on Polkadot & Kusama) ---
const BLOCKS_PER_TIMESLICE = 80;
const SECONDS_PER_BLOCK = 6;
const SECS_PER_DAY = 86400;
const relativeDays = (relayBlocksAway) => (relayBlocksAway * SECONDS_PER_BLOCK) / SECS_PER_DAY;

// --- config (overridable via env) ---
const LEADIN_ALERT_DAYS = Number(process.env.LEADIN_ALERT_DAYS ?? 3);
const TOTAL_ALERT_DAYS = Number(process.env.TOTAL_ALERT_DAYS ?? 7);
const ALERT_COOLDOWN_HOURS = Number(process.env.ALERT_COOLDOWN_HOURS ?? 12);
const STATE_FILE = process.env.STATE_FILE ?? new URL('./.state.json', import.meta.url).pathname;
// Webhook may be given directly (env) or via a file (e.g. a mounted Swarm secret).
function resolveWebhook() {
  if (process.env.DISCORD_WEBHOOK_URL) return process.env.DISCORD_WEBHOOK_URL.trim();
  const f = process.env.DISCORD_WEBHOOK_URL_FILE;
  if (f) {
    try {
      return readFileSync(f, 'utf8').trim();
    } catch (e) {
      console.error('warn: cannot read DISCORD_WEBHOOK_URL_FILE:', e.message);
    }
  }
  return undefined;
}
const WEBHOOK = resolveWebhook();

const CHAINS = [
  {
    name: 'Hydration',
    relay: 'Polkadot',
    task: 2034,
    desiredCores: Number(process.env.HYDRATION_DESIRED_CORES ?? 3),
    coretime: [
      'wss://sys.ibp.network/coretime-polkadot',
      'wss://coretime-polkadot.dotters.network',
      'wss://polkadot-coretime-rpc.polkadot.io',
    ],
  },
  {
    name: 'Basilisk',
    relay: 'Kusama',
    task: 2090,
    desiredCores: Number(process.env.BASILISK_DESIRED_CORES ?? 3),
    coretime: [
      'wss://sys.ibp.network/coretime-kusama',
      'wss://coretime-kusama.dotters.network',
      'wss://kusama-coretime-rpc.polkadot.io',
    ],
  },
];

const argv = new Set(process.argv.slice(2));
const DRY_RUN = argv.has('--dry-run');
const FORCE = argv.has('--force');
const TEST = argv.has('--test');

const log = (...a) => console.log(new Date().toISOString(), ...a);

async function connectWithFallback(endpoints) {
  let lastErr;
  for (const ep of endpoints) {
    try {
      const api = await ApiPromise.create({ provider: new WsProvider(ep), throwOnConnect: true, noInitWarn: true });
      return { api, endpoint: ep };
    } catch (e) {
      lastErr = e;
    }
  }
  throw new Error(`all endpoints failed: ${lastErr?.message ?? lastErr}`);
}

function formatToken(planck, decimals, symbol) {
  const v = Number(BigInt(planck)) / 10 ** decimals;
  return `${v.toLocaleString('en-US', { maximumFractionDigits: 4 })} ${symbol}`;
}

async function assess(chain) {
  const { api, endpoint } = await connectWithFallback(chain.coretime);
  try {
    const decimals = api.registry.chainDecimals[0] ?? 10;
    const symbol = api.registry.chainTokens[0] ?? '';

    const status = (await api.query.broker.status()).toJSON();
    const sale = (await api.query.broker.saleInfo()).toJSON();
    const cfg = (await api.query.broker.configuration()).toJSON();

    const nowTs = status.lastTimeslice;
    const nowRelay = nowTs * BLOCKS_PER_TIMESLICE;
    const regionBegin = sale.regionBegin; // timeslice the next region starts
    const regionBeginRelay = regionBegin * BLOCKS_PER_TIMESLICE;
    const leadinStart = sale.saleStart; // relay block where interlude ends / lead-in begins
    const leadinEnd = sale.saleStart + sale.leadinLength;

    const phase =
      nowRelay < leadinStart ? 'interlude'
      : nowRelay < leadinEnd ? 'leadin'
      : nowRelay < regionBeginRelay ? 'fixed'
      : 'region-open';

    const daysToLeadinEnd = relativeDays(leadinEnd - nowRelay);
    const daysToRegionBegin = relativeDays(regionBeginRelay - nowRelay);

    // Active cores serving the para right now.
    const activeCores = [];
    for (const [k, v] of await api.query.broker.workload.entries()) {
      if (JSON.stringify(v.toJSON()).includes(`"task":${chain.task}`)) activeCores.push(k.args[0].toNumber());
    }

    // Cores already secured for the upcoming region (regionBegin) via workplan.
    const securedCores = [];
    for (const [k, v] of await api.query.broker.workplan.entries()) {
      const [ts, core] = k.args[0].toJSON();
      if (ts === regionBegin && JSON.stringify(v.toJSON()).includes(`"task":${chain.task}`)) securedCores.push(core);
    }

    // Un-exercised renewal rights for the upcoming region (when === regionBegin).
    const pendingRenewals = [];
    for (const [k, v] of await api.query.broker.potentialRenewals.entries()) {
      const key = k.args[0].toJSON();
      const val = v.toJSON();
      if (key.when === regionBegin && JSON.stringify(val).includes(`"task":${chain.task}`)) {
        pendingRenewals.push({ core: key.core, price: val.price, priceFmt: formatToken(val.price, decimals, symbol) });
      }
    }

    const shortfall = Math.max(0, chain.desiredCores - securedCores.length);

    // Encoded renew calls for the pending cores (handy to paste into a signer).
    const renewCalls = pendingRenewals.map((p) => ({
      core: p.core,
      callHex: api.tx.broker.renew(p.core).method.toHex(),
    }));

    return {
      ok: true,
      endpoint,
      decimals,
      symbol,
      phase,
      nowTs,
      regionBegin,
      regionEnd: sale.regionEnd,
      daysToLeadinEnd,
      daysToRegionBegin,
      activeCores: activeCores.sort((a, b) => a - b),
      securedCores: securedCores.sort((a, b) => a - b),
      pendingRenewals,
      renewCalls,
      shortfall,
    };
  } finally {
    await api.disconnect();
  }
}

function decideAlert(chain, a) {
  // Only a shortfall (cores not yet secured for next region) is actionable.
  if (a.shortfall <= 0) return null;
  const reasons = [];
  let severity = null;
  if (a.daysToRegionBegin < TOTAL_ALERT_DAYS) {
    severity = 'URGENT';
    reasons.push(`only ${a.daysToRegionBegin.toFixed(1)}d until the region begins — renewal right is lost after that`);
  }
  if (a.phase === 'leadin' && a.daysToLeadinEnd < LEADIN_ALERT_DAYS) {
    severity = severity ?? 'WARNING';
    reasons.push(`lead-in ends in ${a.daysToLeadinEnd.toFixed(1)}d`);
  }
  if (!severity) return null;
  return { severity, reasons };
}

function buildEmbed(chain, a, alert) {
  const color = alert.severity === 'URGENT' ? 0xe01e1e : 0xe0a020;
  const pending = a.pendingRenewals.length
    ? a.pendingRenewals.map((p) => `\`renew(${p.core})\` — ${p.priceFmt}`).join('\n')
    : '*(none recorded — may require a fresh market purchase)*';
  const calls = a.renewCalls.length
    ? a.renewCalls.map((c) => `core ${c.core}: \`${c.callHex}\``).join('\n')
    : '—';
  return {
    title: `⚠️ ${chain.name} coretime renewal — ${alert.severity}`,
    description: alert.reasons.map((r) => `• ${r}`).join('\n'),
    color,
    fields: [
      { name: 'Para / relay', value: `task ${chain.task} on ${chain.relay} coretime`, inline: true },
      { name: 'Sale phase', value: a.phase, inline: true },
      { name: 'Cores', value: `active: ${a.activeCores.length} (${a.activeCores.join(', ') || '—'})\nsecured next region: ${a.securedCores.length} (${a.securedCores.join(', ') || '—'})\ntarget: ${chain.desiredCores} → **shortfall ${a.shortfall}**`, inline: false },
      { name: 'Time left', value: `lead-in ends: ${a.daysToLeadinEnd > 0 ? a.daysToLeadinEnd.toFixed(1) + 'd' : 'passed'}\nregion begins: ${a.daysToRegionBegin.toFixed(1)}d`, inline: false },
      { name: 'Pending renewals', value: pending, inline: false },
      { name: 'Encoded calls (sign on the coretime chain)', value: calls.slice(0, 1024), inline: false },
    ],
    footer: { text: `coretime watchdog • ts ${a.nowTs} • ${a.endpoint}` },
    timestamp: new Date().toISOString(),
  };
}

async function sendDiscord(embeds) {
  if (DRY_RUN || !WEBHOOK) {
    log(DRY_RUN ? '[dry-run] would POST to Discord:' : '[no DISCORD_WEBHOOK_URL] payload:');
    console.log(JSON.stringify({ embeds }, null, 2));
    return;
  }
  const res = await fetch(WEBHOOK, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ username: 'coretime-watchdog', embeds }),
  });
  if (!res.ok) throw new Error(`discord webhook ${res.status}: ${await res.text()}`);
  log(`posted ${embeds.length} alert(s) to Discord`);
}

function loadState() {
  try {
    return JSON.parse(readFileSync(STATE_FILE, 'utf8'));
  } catch {
    return {};
  }
}
function saveState(state) {
  try {
    mkdirSync(dirname(STATE_FILE), { recursive: true });
    writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
  } catch (e) {
    log('warn: could not persist state:', e.message);
  }
}

// Re-alert only when severity/shortfall changes or the cooldown has elapsed.
function shouldSend(state, key, fingerprint) {
  if (FORCE) return true;
  const prev = state[key];
  if (!prev) return true;
  if (prev.fingerprint !== fingerprint) return true;
  const ageH = (Date.now() - prev.lastSentMs) / 3_600_000;
  return ageH >= ALERT_COOLDOWN_HOURS;
}

async function main() {
  if (TEST) {
    await sendDiscord([{ title: '✅ coretime watchdog test', description: 'webhook reachable', color: 0x2ecc71, timestamp: new Date().toISOString() }]);
    return;
  }
  if (!WEBHOOK && !DRY_RUN) {
    log('error: DISCORD_WEBHOOK_URL is not set (use --dry-run to print without sending)');
    process.exit(2);
  }

  const state = loadState();
  const toSend = [];
  let hadError = false;

  for (const chain of CHAINS) {
    try {
      const a = await assess(chain);
      const alert = decideAlert(chain, a);
      log(`${chain.name}: phase=${a.phase} active=${a.activeCores.length} secured=${a.securedCores.length}/${chain.desiredCores} shortfall=${a.shortfall} leadinEnd=${a.daysToLeadinEnd.toFixed(1)}d regionBegin=${a.daysToRegionBegin.toFixed(1)}d -> ${alert ? alert.severity : 'ok'}`);
      if (!alert) {
        if (state[chain.name]) delete state[chain.name]; // condition cleared
        continue;
      }
      const fingerprint = `${alert.severity}:${a.shortfall}`;
      if (shouldSend(state, chain.name, fingerprint)) {
        toSend.push(buildEmbed(chain, a, alert));
        state[chain.name] = { fingerprint, lastSentMs: Date.now() };
      } else {
        log(`${chain.name}: alert active but within cooldown — skipping resend`);
      }
    } catch (e) {
      hadError = true;
      log(`${chain.name}: assessment FAILED: ${e.message}`);
      const fp = `error:${e.message}`.slice(0, 80);
      if (shouldSend(state, `${chain.name}:error`, fp)) {
        toSend.push({
          title: `🔌 ${chain.name} coretime watchdog — check failed`,
          description: `Could not assess renewal status: \`${e.message}\``,
          color: 0x808080,
          timestamp: new Date().toISOString(),
        });
        state[`${chain.name}:error`] = { fingerprint: fp, lastSentMs: Date.now() };
      }
    }
  }

  if (toSend.length) await sendDiscord(toSend);
  else log('no alerts to send');

  saveState(state);
  if (hadError) process.exitCode = 1;
}

main().catch((e) => {
  log('fatal:', e.message);
  process.exit(1);
});
