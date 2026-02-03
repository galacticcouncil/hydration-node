import {ApiPromise, WsProvider} from '@polkadot/api';

/* ========= CONFIG ========= */

const RPC = 'wss://hydration.ibp.network';

/* ========= MAIN ========= */

(async () => {
    try {
        const provider = new WsProvider(RPC);
        const api = await ApiPromise.create({provider, noInitWarn: true});

        console.log('Fetching all assets from registry...\n');

        const entries = await api.query.assetRegistry.assets.entries();

        const stableShares = [];

        for (const [key, value] of entries) {
            const assetId = key.args[0].toString();
            const meta = value.unwrap();
            const assetType = meta.assetType.toString();

            if (assetType === 'StableSwap') {
                stableShares.push({
                    id: assetId,
                    name: meta.name.toHuman(),
                    symbol: meta.symbol.toHuman(),
                });
            }
        }

        // Sort by ID numerically
        stableShares.sort((a, b) => Number(a.id) - Number(b.id));

        console.log('Stable Share Assets:\n');
        console.log('ID\t\tSymbol\t\tName');
        console.log('─'.repeat(60));

        for (const asset of stableShares) {
            const idPadded = asset.id.padEnd(12);
            const symbolPadded = (asset.symbol || 'N/A').padEnd(12);
            console.log(`${idPadded}\t${symbolPadded}\t${asset.name || 'N/A'}`);
        }

        console.log('─'.repeat(60));
        console.log(`Total: ${stableShares.length} stable share assets\n`);

        await api.disconnect();
        process.exit(0);
    } catch (e) {
        console.error('ERROR:', e.message || e);
        process.exit(1);
    }
})();
