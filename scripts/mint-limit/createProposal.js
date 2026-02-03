import {ApiPromise, WsProvider} from '@polkadot/api';

import {createSdkContext} from '@galacticcouncil/sdk';

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
const TC_THRESHOLD = 4;

const ASSETS = [
    // Stable shares
    100, // "4-Pool"
    101, // "2-Pool"
    102, // "2-Pool-Stbl"
    103, // "3-Pool"
    //104, // "2-Pool-WETH"
    105, // "3-Pool-MRL"
    110, // "2-Pool-HUSDC"
    111, // "2-Pool-HUSDT"
    112, // "2-Pool-HUSDS"
    113, // "2-Pool-HUSDe"
    690, // "2-Pool-GDOT"
    4200, // "2-Pool-GETH"
];

// Overwrites for specific assets (asset_id -> mint_limit_value)
// When an asset ID is present here, use this value instead of calculating
const MINT_LIMIT_OVERWRITES = {
    5: BigInt("50000000000000000"),      // Polkadot
    15: BigInt("9000000000000000"), //VDOT
    10: BigInt("5000000000000"), //USDT
    22: BigInt("5000000000000"), //UDSC
};

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
        let finalLimit;
        let isOverwrite = false;

        if (MINT_LIMIT_OVERWRITES[assetId]) {
            finalLimit = MINT_LIMIT_OVERWRITES[assetId];
            isOverwrite = true;
        } else {
            finalLimit = await fetchTwoXMax(assetId, fromIso, toIso);
        }

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
            (Number(finalLimit) / 10 ** assetDecimals) * (parseFloat(priceAmount) / 10 ** priceDecimals)
        );

        // Enforce minimum $50k USD equivalent
        const MIN_USD_LIMIT = 50000;
        let adjustedLimit = finalLimit;
        let wasAdjusted = false;

        if (tmaxUsd < MIN_USD_LIMIT) {
            // Safety check to prevent division by zero
            if (parseFloat(priceAmount) <= 0) {
                throw new Error(`Invalid price for asset ${assetId}: ${priceAmount}`);
            }

            // Calculate the asset amount needed for $50k USD
            const usdToAssetRate = (parseFloat(priceAmount) / 10 ** priceDecimals);
            const minAssetAmount = MIN_USD_LIMIT / usdToAssetRate;
            adjustedLimit = BigInt(Math.round(minAssetAmount * (10 ** assetDecimals)));
            wasAdjusted = true;
        }

        const finalUsdAmount = Math.round(
            (Number(adjustedLimit) / 10 ** assetDecimals) * (parseFloat(priceAmount) / 10 ** priceDecimals)
        );

        const assetName = meta.unwrap().name.toHuman();
        const statusFlags = [
            isOverwrite ? 'OVERWRITE' : null,
            wasAdjusted ? 'ADJUSTED TO MIN' : null
        ].filter(Boolean).join(', ');

        console.log(`${assetId} (${assetName}) -> mint limit = $${finalUsdAmount.toLocaleString()} | amount = ${adjustedLimit} ${statusFlags ? ` (${statusFlags})` : ''}`);
        calls.push(buildUpdateCall(api, assetId, adjustedLimit));
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
