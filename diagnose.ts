/**
 * Read-only diagnostic for a failed Asset Hub -> Hydration reserve transfer.
 *
 * Distinguishes:
 *   State 1: trapped & claimable on Hydration (AssetsTrapped + live assetTraps entry)
 *   State 2: backed but never minted (no Hydration-side execution; value stranded in AH sovereign)
 *   State 3: delivered late/partial (beneficiary already credited)
 *
 * STRICTLY READ-ONLY. Constructs/sends no transactions.
 *
 *   npm i @polkadot/api
 *   npx tsx diagnose.ts
 */
import { ApiPromise, WsProvider } from '@polkadot/api';

// ---- OPERATOR INPUTS ----------------------------------------------------
// From the XCSCAN journey: 0x2814d48967cdb849ee5702540974962883a0e06ee63a8b682721531dd1a89c43
const MSG_HASH = '0x2814d48967cdb849ee5702540974962883a0e06ee63a8b682721531dd1a89c43';

const BENEFICIARY_HDX = 'FILL_ME';          // beneficiary SS58 on Hydration
const AH_ASSET_ID: number | null = 1984;    // AH assets-pallet id: 1984 USDT, 1337 USDC; null => native DOT
const HDX_ASSET_ID: number | null = null;   // null => auto-resolve from the AssetsTrapped MultiLocation / registry
const HDX_ASSET_ID_FALLBACK = 10;           // used only if auto-resolution fails (DOT=5, USDT=10)

// Narrow the scan. If you know the destination block from XCSCAN, set HDX_BLOCK_NEAR and SCAN_RADIUS.
const HDX_BLOCK_NEAR: number | null = null; // approx Hydration destination block, if known
const SCAN_RADIUS = 25;                      // blocks scanned either side of HDX_BLOCK_NEAR
const HDX_BLOCK_FROM: number | null = null;  // explicit range start (overrides _NEAR); null => head-2000
const HDX_BLOCK_TO: number | null = null;    // explicit range end;   null => head
// -------------------------------------------------------------------------

const AH_RPC = 'wss://polkadot-asset-hub-rpc.polkadot.io';
const HDX_RPC = 'wss://rpc.hydradx.cloud';
const HDX_SOVEREIGN_ON_AH = '13cKp89Uh2yWgTG28JA1QEvPUMjEPKejqkjHKf9zqLiFKjH6';

const connect = (url: string) => ApiPromise.create({ provider: new WsProvider(url) });
const norm = (h: string) => h.toLowerCase().replace(/^0x/, '');
const hashMatches = (s: string) => s && norm(s).includes(norm(MSG_HASH));

// Walk a decoded value looking for our message hash / topic id anywhere in it.
function mentionsHash(v: any): boolean {
  try {
    return JSON.stringify(v).toLowerCase().includes(norm(MSG_HASH));
  } catch {
    return false;
  }
}

async function scanHydrationForXcm(hdx: ApiPromise) {
  const head = (await hdx.rpc.chain.getHeader()).number.toNumber();
  let from: number, to: number;
  if (HDX_BLOCK_FROM != null) {
    from = HDX_BLOCK_FROM;
    to = HDX_BLOCK_TO ?? head;
  } else if (HDX_BLOCK_NEAR != null) {
    from = Math.max(0, HDX_BLOCK_NEAR - SCAN_RADIUS);
    to = Math.min(head, HDX_BLOCK_NEAR + SCAN_RADIUS);
  } else {
    to = HDX_BLOCK_TO ?? head;
    from = Math.max(0, to - 2000);
  }
  console.log(`Scanning Hydration blocks ${from}..${to} (${to - from + 1} blocks) for XCM activity`);

  const trapped: any[] = [];
  for (let n = from; n <= to; n++) {
    const hash = await hdx.rpc.chain.getBlockHash(n);
    const apiAt = await hdx.at(hash);
    const events = await apiAt.query.system.events();
    for (const { event } of events) {
      const { section, method } = event;
      const human: any = event.data.toHuman();

      const isTrap = section === 'polkadotXcm' && method === 'AssetsTrapped';
      const isXcmpFail = section === 'xcmpQueue' && /Fail|BadVersion|Overweight/.test(method);
      const isMqProcessed = section === 'messageQueue' && method === 'Processed';
      const isMqFail = section === 'messageQueue' && /Fail|Overweight/.test(method);

      if (isMqProcessed && human?.success === true && !mentionsHash(human)) continue;

      if (isTrap || isXcmpFail || isMqProcessed || isMqFail) {
        const tag = mentionsHash(human) ? '  <== MATCHES MSG_HASH' : '';
        console.log(`  [block ${n}] ${section}.${method}${tag}`, JSON.stringify(human));
        if (isTrap) trapped.push({ block: n, data: human });
      }
    }
  }
  return trapped;
}

async function resolveHdxAssetId(hdx: ApiPromise, trappedMultiLocation: any): Promise<number> {
  if (HDX_ASSET_ID != null) return HDX_ASSET_ID;
  const keys = Object.keys(hdx.query.assetRegistry);
  console.log('  assetRegistry storage keys:', keys.join(', '));
  // Location->asset reverse map name has drifted across runtimes (e.g. locationAssets).
  const mapName = keys.find((k) => /location/i.test(k) && /asset/i.test(k));
  if (mapName && trappedMultiLocation) {
    try {
      const got: any = await (hdx.query.assetRegistry as any)[mapName](trappedMultiLocation);
      if (got?.isSome) {
        const id = got.unwrap().toString();
        console.log(`  resolved HDX asset id via assetRegistry.${mapName} = ${id}`);
        return Number(id);
      }
    } catch (e) {
      console.log(`  reverse-map lookup failed: ${(e as Error).message}`);
    }
  }
  console.log(`  falling back to HDX_ASSET_ID_FALLBACK=${HDX_ASSET_ID_FALLBACK}`);
  return HDX_ASSET_ID_FALLBACK;
}

async function checkAssetTraps(hdx: ApiPromise) {
  const entries = await hdx.query.polkadotXcm.assetTraps.entries();
  console.log(`\nLive asset traps on Hydration: ${entries.length}`);
  for (const [key, count] of entries) {
    console.log(`  trapHash=${key.args[0].toHex()} count=${count.toString()}`);
  }
  console.log('  (trapHash = blake2 of (origin, versioned assets); match against the AssetsTrapped event above)');
}

async function reconcile(ah: ApiPromise, hdx: ApiPromise, hdxAssetId: number) {
  let ahBal: bigint;
  if (AH_ASSET_ID == null) {
    const acct: any = await ah.query.system.account(HDX_SOVEREIGN_ON_AH);
    ahBal = acct.data.free.toBigInt();
  } else {
    const a: any = await ah.query.assets.account(AH_ASSET_ID, HDX_SOVEREIGN_ON_AH);
    ahBal = a.isSome ? a.unwrap().balance.toBigInt() : 0n;
  }
  const ti: any = await hdx.query.tokens.totalIssuance(hdxAssetId);
  const totalIssuance = ti.toBigInt();
  const delta = ahBal - totalIssuance;

  console.log('\n--- Reconciliation (aggregate; NOT per-transfer) ---');
  console.log(`AH sovereign balance (backing) : ${ahBal}`);
  console.log(`Hydration total issuance       : ${totalIssuance}`);
  console.log(`Delta (backing - issuance)     : ${delta}`);
  console.log(
    delta > 0n
      ? '  => Positive delta: backing exceeds minted supply (consistent with unminted/trapped value).'
      : '  => ~Zero/negative delta: backing reconciles with minted supply.'
  );
}

async function checkBeneficiary(hdx: ApiPromise, hdxAssetId: number) {
  const acct: any = await hdx.query.tokens.accounts(BENEFICIARY_HDX, hdxAssetId);
  console.log('\n--- Beneficiary balance on Hydration ---');
  console.log(`free=${acct.free.toString()} reserved=${acct.reserved.toString()} frozen=${acct.frozen.toString()}`);
}

(async () => {
  const [ah, hdx] = await Promise.all([connect(AH_RPC), connect(HDX_RPC)]);
  console.log(`AH:  ${(await ah.rpc.system.chain()).toString()}`);
  console.log(`HDX: ${(await hdx.rpc.system.chain()).toString()}`);
  console.log(`Tracking message hash: ${MSG_HASH}\n`);

  const trapped = await scanHydrationForXcm(hdx);
  const trappedLoc = trapped[0]?.data?.[0]; // first arg of AssetsTrapped is usually the trap hash; loc may be in [1]
  const hdxAssetId = await resolveHdxAssetId(hdx, trapped[0]?.data?.[1] ?? trappedLoc);

  await checkAssetTraps(hdx);
  await reconcile(ah, hdx, hdxAssetId);
  if (BENEFICIARY_HDX !== 'FILL_ME') await checkBeneficiary(hdx, hdxAssetId);
  else console.log('\n(Set BENEFICIARY_HDX to also check the beneficiary balance — State 3 ruling.)');

  await ah.disconnect();
  await hdx.disconnect();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
