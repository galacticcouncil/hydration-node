
import { ApiPromise, WsProvider } from '@polkadot/api';

import { createSdkContext } from '@galacticcouncil/sdk';

/* ========= CONFIG ========= */

// Grafana endpoint + datasource UID (from Inspect → Query)
const GRAFANA_URL = 'https://grafana.play.hydration.cloud/api/ds/query';
const GRAFANA_DS = {type: 'postgres', uid: 'OulRfMKVz'};
// Optional auth (only if required by your Grafana env)
const GRAFANA_TOKEN = process.env.GRAFANA_TOKEN || null;

// Chain RPC (Hydration)
const RPC = 'wss://hydration.ibp.network'
    .replace(/^http:\/\//, 'ws://')
    .replace(/^https:\/\//, 'wss://');

const provider = new WsProvider(
    RPC.replace(/^http:\/\//, 'ws://').replace(/^https:\/\//, 'wss://')
);
const api = await ApiPromise.create({provider, noInitWarn: true});
const sdk = await createSdkContext(api);


// Time range (days) for Grafana query
const RANGE_DAYS = 90;

// Technical Committee threshold (how many approvals needed to start motion)
const TC_THRESHOLD = 1;

const ASSETS =  [
    5, // "DOT"
    8, // "Phala"
    9, // "Astar"
    10, // "USDT"
    12, // "Zeitgeist"
    14, // "Bifrost Native Coin"
    15, // "Bifrost Voucher DOT"
    16, // "Glimmer"
    17, // "Interlay"
    19, // "Wrapped BTC (Moonbeam Wormhole)"
    20, // "Wrapped ETH (Moonbeam Wormhole)"
    22, // "USDC"
    27, // "Crust"
    31, // "Darwinia Network RING"
    33, // "Voucher ASTR"
    35, // "OriginTrail"
    1000624, // "AAVE"
    1000752, // "Solana (Moonbeam Wormhole)"
    1000765, // "Threshold BTC"
    1000771, // "Kusama"
    1000794, // "Chainlink"
    1000795, // "SKY"
    1000796, // "Lido"
];

/* ========= HELPERS ========= */

// Lossless string → BigInt (handles ints/decimals/exponent)
function toBigIntLossless(raw) {
    if (typeof raw === 'bigint') return raw;
    const s = (typeof raw === 'string' ? raw : String(raw)).trim();
    if (/^[+-]?\d+$/.test(s)) return BigInt(s);
    if (/^[+-]?\d+\.\d+$/.test(s)) return BigInt(s.split('.')[0]);
    const m = s.match(/^([+-]?)(\d+(?:\.\d+)?)[eE]([+-]?\d+)$/);
    if (m) {
        const sign = m[1] === '-' ? '-' : '';
        const [intPart, fracPart = ''] = m[2].split('.');
        const exp = parseInt(m[3], 10);
        const digits = intPart + fracPart;
        const shift = exp - fracPart.length;
        if (shift >= 0) return BigInt(sign + digits + '0'.repeat(shift));
        return BigInt(0);
    }
    throw new Error(`Cannot convert to BigInt: ${s}`);
}

/* ========= GRAFANA ========= */

function buildTwoXMaxQuery(currencyId, fromIso, toIso) {
    // Cast to integer TEXT to avoid float/exp notation in JSON
    return `
        WITH daily_data AS (SELECT DATE_TRUNC('day', b.timestamp) AS day_ts,
                                   SUM(
                                           CASE
                                               WHEN e.name = 'Tokens.Deposited' THEN (e.args ->>'amount'):: numeric
                                           WHEN e.name = 'Tokens.Withdrawn' THEN -(e.args ->>'amount') ::numeric
                                           ELSE 0
                                           END
                                   )                              AS net_deposits
                            FROM event e
                                     JOIN block b ON e.block_id = b.id
                            WHERE e.name IN ('Tokens.Deposited', 'Tokens.Withdrawn')
                              AND (e.args ->>'currencyId')
            :: int = ${currencyId}
            AND b.timestamp >= '${fromIso}'
            AND b.timestamp
           < '${toIso}'
        GROUP BY 1
            ),
            percentiles AS (
        SELECT
            PERCENTILE_CONT(0.01) WITHIN
        GROUP (ORDER BY net_deposits) AS p5,
            PERCENTILE_CONT(0.99) WITHIN
        GROUP (ORDER BY net_deposits) AS p95
        FROM daily_data
            ), wins AS (
        SELECT
            day_ts, CASE
            WHEN net_deposits < p.p5 THEN p.p5
            WHEN net_deposits > p.p95 THEN p.p95
            ELSE net_deposits
            END AS net_deposits_winsorized
        FROM daily_data d
            CROSS JOIN percentiles p
            )
        SELECT floor(2 * MAX(net_deposits_winsorized)) ::numeric(78,0)::text AS two_x_max
        FROM wins;
    `;
}

async function fetchTwoXMax(currencyId, fromIso, toIso) {
    const rawSql = buildTwoXMaxQuery(currencyId, fromIso, toIso);
    const res = await fetch(GRAFANA_URL, {
        method: 'POST',
        headers: {
            Accept: 'application/json',
            'Content-Type': 'application/json',
            ...(GRAFANA_TOKEN ? {Authorization: `Bearer ${GRAFANA_TOKEN}`} : {})
        },
        body: JSON.stringify({
            range: {from: fromIso, to: toIso, raw: {from: fromIso, to: toIso}},
            queries: [{
                refId: 'twoxmax',
                rawSql,
                format: 'table',
                datasource: GRAFANA_DS
            }]
        })
    });

    if (!res.ok) {
        const txt = await res.text().catch(() => '');
        throw new Error(`Grafana HTTP ${res.status}: ${txt}`);
    }

    const data = await res.json();
    const frame = data?.results?.twoxmax?.frames?.[0];
    if (!frame) throw new Error(`Grafana returned no frames for currencyId=${currencyId}`);

    const fields = frame.schema?.fields ?? [];
    const idx = fields.findIndex(f => f.name === 'two_x_max');
    const col = frame.data?.values?.[(idx >= 0 ? idx : 0)] ?? [];
    const raw = col[0];
    if (raw == null) throw new Error(`two_x_max not found for currencyId=${currencyId}`);

    return toBigIntLossless(raw); // BigInt
}

/* ========= CHAIN / PROPOSAL ========= */
/**
 * assetRegistry.update(
 *   asset_id: u32,
 *   name: Option<Bytes>,
 *   asset_type: Option<PalletAssetRegistryAssetType>,
 *   existential_deposit: Option<u128>,
 *   xcm_rate_limit: Option<u128>,
 *   is_sufficient: Option<bool>,
 *   symbol: Option<Bytes>,
 *   decimals: Option<u8>,
 *   location: Option<HydradxRuntimeXcmAssetLocation>
 * )
 * We set only xcm_rate_limit (Some(value)), others = None.
 */
function buildUpdateCall(api, assetId, twoXMaxBig) {
    return api.tx.assetRegistry.update(
        assetId,
        null,                      // name
        null,                      // asset_type
        null,                      // existential_deposit
        twoXMaxBig.toString(),     // xcm_rate_limit (Some)
        null,                      // is_sufficient
        null,                      // symbol
        null,                      // decimals
        null                       // location
    );
}

async function buildBatchCall({rpc, assetIds, rangeDays}) {
    const now = new Date();
    const from = new Date(now.getTime() - (rangeDays ?? RANGE_DAYS) * 24 * 3600 * 1000);
    const fromIso = from.toISOString();
    const toIso = now.toISOString();

    const calls = [];
    for (const assetId of assetIds) {
        const twoX = await fetchTwoXMax(assetId, fromIso, toIso);
        const price = await sdk.api.router.getBestSpotPrice(assetId.toString(), '10');
        const meta = await api.query.assetRegistry.assets(assetId);
        const assetDecimals = meta.unwrap().decimals

        // Handle special case for asset 10 (USDT) where spot price might be undefined
        let priceAmount, priceDecimals;
        if (assetId === 10 && (!price || price.amount === undefined)) {
            // Use 1 USD as fallback price for USDT
            priceAmount = '1';
            priceDecimals = 0;
        } else {
            priceAmount = price.amount;
            priceDecimals = price.decimals;
        }

        const tmaxUsd = Math.round(
            (Number(twoX) / 10 ** assetDecimals) * (parseFloat(priceAmount) / 10 ** priceDecimals)
        );

        const assetName = meta.unwrap().name.toHuman();
        console.log(`assetId=${assetId} (${assetName}) -> 2×max=${twoX.toString()} | amount=$${tmaxUsd}`);
        calls.push(buildUpdateCall(api, assetId, twoX));
    }

    const batch = api.tx.utility.batchAll(calls);
    return {api, batch};
}

function buildTechnicalCommitteePropose(api, call, threshold) {
    const len = call.method.encodedLength ?? call.method.toU8a().length;
    // collective pallet expects (threshold, proposal, lengthBound)
    return api.tx.technicalCommittee.propose(threshold, call.method, len);
}

/* ========= MAIN ========= */
(async () => {
    try {
        if (!ASSETS.length) throw new Error('ASSETS is empty.');

        const {api, batch} = await buildBatchCall({
            rpc: RPC,
            assetIds: ASSETS,
            rangeDays: RANGE_DAYS
        });

        // Wrap in technicalCommittee.propose
        const tcProposal = buildTechnicalCommitteePropose(api, batch, TC_THRESHOLD);

        console.log('\n--- utility.batchAll (human) ---\n', batch.method.toHuman());
        console.log('\n--- TC propose (human) ---\n', tcProposal.method.toHuman());
        console.log('\n--- TC propose HEX (submit as call/preimage) ---\n', tcProposal.method.toHex());

        // If you want to create a preimage to attach to the motion:
        // const preimage = api.tx.preimage.notePreimage(batch.method.toHex());
        // console.log('\n--- preimage hex ---\n', preimage.method.toHex());
        sdk.destroy();
        api.disconnect();
        await api.disconnect();
    } catch (e) {
        console.error('\nERROR:', e.message || e);
        process.exit(1);
    }
})();
