// Simple HydraDX Extrinsic Executor with hardcoded values
// Modify the values at the top of the script as needed

import { ApiPromise, WsProvider, Keyring} from "@polkadot/api";
import {hexToU8a} from '@polkadot/util';

// ===== CONFIGURATION - MODIFY THESE VALUES =====
//const ENCODED_CALL = '0xa10284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01f4bbaeca10a5371f776ecf1edc35dc320ed67a50776880f343328a0fad15916efbf012aabcc67afa066cbe653827cdc9705720e0b93b345000895f85e691108d94000800004300000000000a00000000407a10f35a0000000000000000000000000000000000000000000000000000080300000000660000000266000000660000000a000000'; // Replace with your encoded extrinsic
const ENCODED_CALL = '0xa10284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01020c8d749763991e91833a416adf1772380e97e23deaaafaac357767da4ca83b4e3e8ae841ff6e039b3ee77de6d6346a363d2bf0ab8956d03081cf1399e5fe8d74000000004300000000000a00000000407a10f35a0000000000000000000000000000000000000000000000000000080300000000660000000266000000660000000a000000';
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

        // Prepare the call data
        const callData = hexToU8a(ENCODED_CALL);

        // Create an extrinsic from the encoded call data
        const extrinsic = api.tx(callData);

        console.log(`Extrinsic details:`);
        console.log(`  Method: ${extrinsic.method.section}.${extrinsic.method.method}`);
        console.log(`  Arguments: ${JSON.stringify(extrinsic.args.map(arg => arg.toString()), null, 2)}`);

        // Just sign and send the transaction without waiting for events
        console.log('Submitting extrinsic...');
        const hash = await extrinsic.signAndSend(account);
        console.log(`Transaction submitted with hash: ${hash.toHex()}`);
        console.log('Extrinsic submitted successfully!');

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