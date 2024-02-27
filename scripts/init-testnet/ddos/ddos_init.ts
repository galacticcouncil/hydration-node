// Required imports
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { hexToU8a } = require('@polkadot/util');

const { Keyring } = require('@polkadot/keyring');

async function main () {
    // Initialise the provider to connect to the local node
    //const provider = new WsProvider('wss://rpc.nice.hydration.cloud');
    const provider = new WsProvider('ws://127.0.0.1:9988');

    // Create the API and wait until ready
    const api = await ApiPromise.create({ provider });

    // Retrieve the chain & node information information via rpc calls
    const [chain, nodeName, nodeVersion] = await Promise.all([
        api.rpc.system.chain(),
        api.rpc.system.name(),
        api.rpc.system.version()
    ]);

    console.log(`You are connected to chain ${chain} using ${nodeName} v${nodeVersion}`);

    const keyring = new Keyring({ type: 'sr25519' });
    const alice = keyring.addFromUri('//Alice');

    let transactions = [];
    assetRegistry(api, transactions);
    mintForAlice(api, transactions);
    mintUsersWithHDX(api, transactions);
    initOmnipool(api, transactions)

    let batch = api.tx.utility.batchAll(transactions);
    await api.tx.preimage.notePreimage(batch.method.toHex()).signAndSend(alice);
}

main().catch(console.error).finally(() => process.exit());

function mintForTreasuryDca(api, txs) {
    let treasury = "7KQx4f7yU3hqZHfvDVnSfe6mpgAT8Pxyr67LXHV6nsbZo3Tm";
    //weth
    txs.push(
        api.tx.currencies.updateBalance(treasury, 5, "5000000000000000")
    );

}

function mintForAlice(api, txs) {
    let alice = "7NPoMQbiA6trJKkjB35uk96MeJD4PGWkLQLH7k7hXEkZpiba";
    txs.push(
        api.tx.currencies.updateBalance(alice, 0, "5000000000000000000000000000")
    );

    txs.push(api.tx.currencies.updateBalance(alice, 1, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 2, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 3, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 4, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 5, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 6, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 7, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 8, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 9, "5000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(alice, 10, "5000000000000000000"));
}

function mintUsersWithHDX(api, txs) {
    const keyring = new Keyring({ type: 'sr25519' });
    const user1 = keyring.addFromUri('//User1');
    const user2 = keyring.addFromUri('//User2');
    const user3 = keyring.addFromUri('//User3');
    const user4 = keyring.addFromUri('//User4');
    const user5 = keyring.addFromUri('//User5');
    const user6 = keyring.addFromUri('//User6');
    const user7 = keyring.addFromUri('//User7');
    const user8 = keyring.addFromUri('//User8');
    const user9 = keyring.addFromUri('//User9');
    const user10 = keyring.addFromUri('//User10');

    txs.push(api.tx.currencies.updateBalance(user1.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user2.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user3.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user4.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user5.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user6.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user7.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user8.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user9.publicKey, 0, "5000000000000000000000000000"));
    txs.push(api.tx.currencies.updateBalance(user10.publicKey, 0, "5000000000000000000000000000"));
}

function assetRegistry(api, txs) {
    const assets = require('../assets.json');
    let keys = Object.keys(assets);

    for (let i = 0, l = keys.length; i < l; i++) {
        let k = keys[i];
        let a = assets[k];
        let tx;

        if (k == "0") {
            tx = api.tx.assetRegistry.setMetadata(k, a.metadata.symbol, a.metadata.decimals);
            txs.push(tx);

            continue;
        }

        if (k == "1" || k == "2") {
            let aType = {};
            aType[a.asset.assetType] = 0;

            tx = api.tx.assetRegistry.update(k, a.asset.name, aType, 100, null);
            txs.push(tx);

            tx = api.tx.assetRegistry.setMetadata(k, a.metadata.symbol, a.metadata.decimals);
            txs.push(tx);
            continue;
        }

        let aType = {};
        aType[a.asset.assetType] = 0;

        a.metadata.decimals = Number(a.metadata.decimals);

        tx = api.tx.assetRegistry.register(a.asset.name, aType, 100, k, a.metadata, null, null);
        txs.push(tx);
    };

    return txs;
}

function initOmnipool(api, txs) {
    let omniAccount = "7L53bUTBbfuj14UpdCNPwmgzzHSsrsTWBHX5pys32mVWM3C1";
    //hdx
    txs.push(
        api.tx.currencies.updateBalance(omniAccount, 0, "936329588000000000")
    );

    //dai
    txs.push(
        api.tx.currencies.updateBalance(omniAccount, 2, "50000000000000000000000")
    );

    txs.push(
        api.tx.currencies.updateBalance(omniAccount, 5, "936329588000000000")
    );

    txs.push(
        api.tx.omnipool.addToken(0, "1201500000000000", 1000000, omniAccount)
    );
    txs.push(
        api.tx.omnipool.addToken(2, "1501500000000000", 1000000, omniAccount)
    );

    txs.push(
        api.tx.omnipool.addToken(5, "1701500000000000", 1000000, omniAccount)
    );
}