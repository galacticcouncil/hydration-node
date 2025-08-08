const fetch = require("node-fetch"); // install with `npm install node-fetch@2`

//TEST URL: https://galacticcouncil.squids.live/hydration-pools:whale-prod/api/graphiql

const endpoint = "https://galacticcouncil.squids.live/hydration-pools:whale-prod/api/graphql";
const assetId = "0";
const endBlock = 8633390;
const blocksPerDay = 14400;
const days = 30;
const startBlock = endBlock - (blocksPerDay * days);

function buildQuery() {
    let fields = "";
    for (let i = 0; i < days; i++) {
        const block = startBlock + i * blocksPerDay;
        fields += `
      day${i + 1}: assetHistoricalData(
        filter: {
          assetRegistryId: { equalTo: $assetId }
          paraBlockHeight: { lessThanOrEqualTo: ${block} }
        }
        orderBy: PARA_BLOCK_HEIGHT_DESC
        first: 1
      ) {
        nodes {
          paraBlockHeight
          totalIssuance
        }
      }
    `;
    }

    return `
    query IssuanceHistory($assetId: String!) {
      ${fields}
    }
  `;
}

async function main() {
    const query = buildQuery();

    const res = await fetch(endpoint, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            query,
            variables: { assetId },
        }),
    });

    const json = await res.json();

    const data = json.data;

    const results = [];

    for (let i = 0; i < days; i++) {
        const dayKey = `day${i + 1}`;
        const node = data[dayKey]?.nodes?.[0];
        if (!node) continue;

        results.push({
            day: i + 1,
            block: node.paraBlockHeight,
            issuance: BigInt(node.totalIssuance),
        });
    }

    console.log("Day | Block     | Total Issuance         | Daily Change");
    console.log("----|-----------|-------------------------|--------------");

    const diffs = [];
    for (let i = 0; i < results.length; i++) {
        const r = results[i];
        const prev = results[i - 1];
        const diff = i === 0 ? 0n : r.issuance - prev.issuance;
        const diffStr = i === 0 ? "-" : diff.toString();
        console.log(
            `${r.day.toString().padStart(2)}  | ${r.block.toString().padEnd(9)} | ${r.issuance.toString().padEnd(23)} | ${diffStr}`
        );
        if (i > 0) diffs.push(Number(diff));
    }

    // Filter for mint-only days (positive changes) for circuit breaker analysis
    const mintOnlyDiffs = diffs.filter(diff => diff > 0);
    const burnDays = diffs.filter(diff => diff < 0).length;
    const neutralDays = diffs.filter(diff => diff === 0).length;

    console.log("\n" + "=".repeat(60));
    console.log("DAILY ISSUANCE CHANGE ANALYSIS");
    console.log("=".repeat(60));
    console.log(`Total days analyzed: ${diffs.length}`);
    console.log(`Days with minting (positive): ${mintOnlyDiffs.length}`);
    console.log(`Days with burning (negative): ${burnDays}`);
    console.log(`Days with no change: ${neutralDays}`);

    // Calculate percentiles for mint-only days
    if (mintOnlyDiffs.length > 0) {
        const sorted = [...mintOnlyDiffs].sort((a, b) => a - b);
        const p50 = sorted[Math.floor(sorted.length * 0.5)];
        const p80 = sorted[Math.floor(sorted.length * 0.8)];
        const p90 = sorted[Math.floor(sorted.length * 0.9)];
        const p95 = sorted[Math.floor(sorted.length * 0.95)];

        console.log("\nMINT-ONLY PERCENTILES:");
        console.log(`50th percentile: ${p50.toLocaleString()}`);
        console.log(`80th percentile: ${p80.toLocaleString()}`);
        console.log(`90th percentile: ${p90.toLocaleString()}`);
        console.log(`95th percentile: ${p95.toLocaleString()}`);

        console.log("\nSUGGESTED CIRCUIT BREAKER LIMITS:");
        console.log(`Conservative (80th × 2): ${(p80 * 2).toLocaleString()}`);
        console.log(`Moderate (80th × 3):     ${(p80 * 3).toLocaleString()}`);
        console.log(`Aggressive (90th × 2):   ${(p90 * 2).toLocaleString()}`);
    }

    // Create histogram of mint-only daily changes
    console.log("\n" + "=".repeat(50));
    console.log("HISTOGRAM OF DAILY MINTING (POSITIVE CHANGES ONLY)");
    console.log("=".repeat(50));

    if (mintOnlyDiffs.length > 0) {
        const min = Math.min(...mintOnlyDiffs);
        const max = Math.max(...mintOnlyDiffs);
        const range = max - min;
        const bins = 10;
        const binSize = range / bins;

        const histogram = new Array(bins).fill(0);

        mintOnlyDiffs.forEach(diff => {
            const binIndex = Math.min(Math.floor((diff - min) / binSize), bins - 1);
            histogram[binIndex]++;
        });

        console.log(`Range: ${min.toLocaleString()} to ${max.toLocaleString()}`);
        console.log(`Bin size: ${Math.round(binSize).toLocaleString()}\n`);

        for (let i = 0; i < bins; i++) {
            const binStart = Math.round(min + i * binSize);
            const binEnd = Math.round(min + (i + 1) * binSize);
            const count = histogram[i];
            const bar = "█".repeat(count) + "░".repeat(Math.max(0, 5 - count));
            console.log(`${binStart.toString().padStart(12)} - ${binEnd.toString().padEnd(12)} | ${bar} (${count})`);
        }
    } else {
        console.log("No minting days found in the analyzed period.");
    }
}

main().catch((err) => {
    console.error("Error:", err);
});
