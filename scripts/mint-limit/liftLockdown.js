import {ApiPromise, WsProvider} from '@polkadot/api';

/* ========= CONFIG ========= */

const RPC = 'wss://hydration.ibp.network';
const TC_THRESHOLD = 4;
const ASSET_ID = 40;

/* ========= MAIN ========= */

(async () => {
    try {
        const provider = new WsProvider(RPC);
        const api = await ApiPromise.create({provider, noInitWarn: true});

        // Build the circuitBreaker.forceLiftLockdown call
        const forceLiftCall = api.tx.circuitBreaker.forceLiftLockdown(ASSET_ID);

        // Calculate lengthBound for the proposal
        const lengthBound = forceLiftCall.method.encodedLength ?? forceLiftCall.method.toU8a().length;

        // Wrap in technicalCommittee.propose
        const tcProposal = api.tx.technicalCommittee.propose(
            TC_THRESHOLD,
            forceLiftCall.method,
            lengthBound
        );

        console.log('--- forceLiftLockdown call (human) ---\n', forceLiftCall.method.toHuman());
        console.log('\n--- TC propose (human) ---\n', tcProposal.method.toHuman());
        console.log('\n--- TC propose HEX (submit as call) ---\n', tcProposal.method.toHex());
        console.log('\n--- Length bound ---\n', lengthBound);

        await api.disconnect();
    } catch (e) {
        console.error('\nERROR:', e.message || e);
        process.exit(1);
    }
})();
