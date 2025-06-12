
import { ApiPromise, WsProvider, Keyring} from "@polkadot/api";
import {hexToU8a} from '@polkadot/util';

// ===== CONFIGURATION - MODIFY THESE VALUES =====
const ENDPOINT = 'ws://127.0.0.1:8000';    // Replace with your node endpoint
const ACCOUNT_URI = '//Alice';             // Replace with your account URI
// ===============================================

async function submitExtrinsic() {
    console.log('Starting extrinsic submission...');

    // Connect to the node
    console.log(`Connecting to ${ENDPOINT}...`);
    const provider = new WsProvider(ENDPOINT);
    const api = await ApiPromise.create({
        provider,
        throwOnConnect: false,
        throwOnError: false
    });

    // Get chain information
    const chain = await api.rpc.system.chain();
    console.log(`Connected to chain ${chain}`);

    try {
        // Create account to sign the transaction
        const keyring = new Keyring({ type: 'sr25519' });
        const account = keyring.addFromUri(ACCOUNT_URI);
        console.log(`Using account: ${account.address}`);

        await routerSell(api, account );

    } catch (error) {
        console.error(`Error: ${error.message}`);
    } finally {
        console.log('Disconnecting from node...');
        await api.disconnect();
        console.log('Disconnected.');
    }
}

// Execute the script
submitExtrinsic()
    .catch(console.error)
    .finally(() => {
        console.log('Script execution completed');
        process.exit(0);
    });


async function routerSell(api, user) {
    try {
        const trade0 ={
            pool: {
                Omnipool: null
            },
            assetIn: 0,   // Asset in from the first trade
            assetOut: 102 // Asset out from the first trade
        };

        // Create the second trade (index 1)
        const trade1 = {
            pool: {
                Stableswap: 102  // Stableswap pool ID is 102
            },
            assetIn: 102,  // Asset in from the second trade
            assetOut: 10   // Asset out from the second trade
        };

        await api.tx.router
            .sell(0, 10, 100000000000000, 0, [trade0, trade1])
            .signAndSend(user);
    } catch (error) {
        console.log("Error while sending DCA - Sent transaction counter when signing fails: ", error);
    }
}