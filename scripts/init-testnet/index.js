// Required imports
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');

async function main () {
  let rpcAddr = process.argv[2] || 'ws://localhost:9946';
  
  console.log(`\nConnecting to RPC node: ${rpcAddr}\n`);

  // Initialise the provider to connect to the local node
  const provider = new WsProvider(rpcAddr);

  // Create the API and wait until ready
  const api = await ApiPromise.create({ provider });

  // Retrieve the chain & node information information via rpc calls
  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version()
  ]);

  console.log(`\nYou are connected to chain ${chain} using ${nodeName} v${nodeVersion}\n`);
  
  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');

  let transactions = [];
  assetRegistry(api, transactions);
  mintOmnipool(api, transactions);
  mintStaking(api, transactions);
  initStaking(api, transactions);
  initOmnipool(api, transactions)

  let batch = api.tx.utility.batchAll(transactions);

  await api.tx.preimage.notePreimage(batch.method.toHex()).signAndSend(alice);

  console.log(`\nPreimage created\n`);
}

main().catch(console.error).finally(() => process.exit());

function assetRegistry(api, txs) {
  const assets = require('./assets.json');
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

      let aLocation = (a['location']) ? a['location'] : null;
      if (aLocation) {
        tx = api.tx.assetRegistry.setLocation(k, aLocation);
        txs.push(tx);
      }

      continue;
    }

    let aType = {};
    aType[a.asset.assetType] = 0;

    a.metadata.decimals = Number(a.metadata.decimals);

    let aLocation = (a['location']) ? a['location'] : null;
    tx = api.tx.assetRegistry.register(a.asset.name, aType, 100, k, a.metadata, aLocation, null);
    txs.push(tx);
  };
  
  return txs;
}

function mintOmnipool(api, txs) {
  let omniAccount = "7L53bUTBbfuj14UpdCNPwmgzzHSsrsTWBHX5pys32mVWM3C1";
  //hdx
  txs.push(
    api.tx.currencies.updateBalance(omniAccount, 0, "936329588000000000")
  ); 
  
  //dai
  txs.push(
    api.tx.currencies.updateBalance(omniAccount, 2, "50000000000000000000000")
  ); 
  
  //lrna
  txs.push(
    api.tx.currencies.updateBalance(omniAccount, 1, 3374999999982000)
  ); 
}

function mintStaking(api, txs) {
  let stakingPot = "7L53bUTCQURi4iNpkVMox9K5XUra9Nom1nvJDMwxNRdJR7zu";
  //hdx
  txs.push(
    api.tx.currencies.updateBalance(stakingPot, 0, "1000000000000000")
  ); 
}

function initStaking(api, txs) {
  txs.push(
    api.tx.staking.initializeStaking()
  );
}

function initOmnipool(api, txs) {
  txs.push(
    api.tx.omnipool.setTvlCap("522222000000000000000000")
  );

  txs.push(
    api.tx.omnipool.initializePool("45000000000", "1201500000000000", 1000000, 100000)
  );
}
