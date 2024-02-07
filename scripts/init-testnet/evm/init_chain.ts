// Required imports
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { hexToU8a } = require('@polkadot/util');
const { encodeAddress, decodeAddress } = require('@polkadot/util-crypto');
/*const {
  isAddress,
  getAddress,
}  =  require('@ethersproject/address')

*/
const { Keyring } = require('@polkadot/keyring');

/*
export class H160 {
  static prefixBytes = Buffer.from("ETH\0")
  address: string

  constructor(address: string) {
    this.address = safeConvertAddressH160(address) ?? ""
  }

  toAccount = () => {
    const addressBytes = Buffer.from(this.address.slice(2), "hex")
    return encodeAddress(
      new Uint8Array(
        Buffer.concat([H160.prefixBytes, addressBytes, Buffer.alloc(8)]),
      ),
      63,
    )
  }

  static fromAccount = (address: string) => {
    const decodedBytes = decodeAddress(address)
    const addressBytes = decodedBytes.slice(H160.prefixBytes.length, -8)
    return (
      safeConvertAddressH160(Buffer.from(addressBytes).toString("hex")) ?? ""
    )
  }
}

export function safeConvertAddressH160(value: string): string | null {
  try {
    return getAddress(value?.toLowerCase())
  } catch {
    return null
  }
}*/



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

  /*await api.tx.assetRegistry.register(
    "test token", 
    {"Token": 0}, 
    1000, 
    5, 
    {symbol: "ttt", decimals: 10}, 
    null,
    null
  ).signAndSend(alice, );*/

  let transactions = [];

  //let evmUser = "7KATdGbFsc58BDyfV9ZtxHEYPt5icvS5itHcJh3yWYmpwG8k";
  //const evmUserAcc = keyring.addFromUri(evmUser);

  /*let balance = await api.query.system.account(alice.publicKey);
  console.log(`Alice's HDX balance before tx ${balance.data.free}`);

  let weth = 20;
  let wethBalance = await api.query.tokens.accounts(alice.publicKey, weth);
  console.log(`Alice's HDX balance before tx ${wethBalance.free}`);

  const nonce = await api.rpc.system.accountNextIndex(alice.publicKey);
  await api.tx.evm.withdraw(safeConvertAddressH160(alice.publicKey), "2000000000000").signAndSend(alice, { nonce });
  console.log("Evm tx done");


  balance = await api.query.system.account(alice.publicKey);
  console.log(`Alice's HDX balance is after evm ${balance.data.free}`);*/

  assetRegistry(api, transactions);
  mintForAlice(api, transactions);
  initOmnipool(api, transactions)
  setWethLocation(api,transactions);
  mintMetamaskWeth(api,transactions);
  let tx = api.tx.multiTransactionPayment.addCurrency(20, '16420844565569051996');
  transactions.push(tx);

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
    api.tx.currencies.updateBalance(alice, 0, "5000000000000000000")
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
  txs.push(api.tx.currencies.updateBalance(alice, 20, "5000000000000000000")); 
                                                       
}

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

function setWethLocation(api, txs) {
  const multiLocation = {
    parents: 1,
    interior: {
      X3: [
        {Parachain: 2004},
        {PalletInstance: 110},
        {AccountKey20: {key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed"}}
      ]
    }
  };

  let tx = api.tx.assetRegistry.setLocation(20, multiLocation);
  txs.push(tx);
  
  return txs;
}


function mintMetamaskWeth(api, txs) {
  let my_original_metamask = "7KATdGbFsc58BDyfV9ZtxHEYPt5icvS5itHcJh3yWYmpwG8k";
  let new_metamask = "7KATdGauM2GcuFoZ91PPA8gp1BxWVLBEor7h8TJT3xDm2f5Y";
  let test_acc = "7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb"
  //hdx
  /*txs.push(
    api.tx.currencies.updateBalance(new_metamask, 0, "226329588000000000")
  );*/

  //weth
  txs.push(
    api.tx.currencies.updateBalance(my_original_metamask, 20, "1936329588000000000")
  ); 
  txs.push(
    api.tx.currencies.updateBalance(new_metamask, 20, "1936329588000000000")
  ); 

  txs.push(
    api.tx.currencies.updateBalance(test_acc, 20, "1936329588000000000")
  ); 

  txs.push(
    api.tx.currencies.updateBalance(my_original_metamask, 5, "1936329588000000000")
  ); 
  txs.push(
    api.tx.currencies.updateBalance(new_metamask, 5, "1936329588000000000")
  ); 
  
  txs.push(
    api.tx.currencies.updateBalance(test_acc, 5, "1936329588000000000")
  ); 
}

/*
function mintStaking(api, txs) {
  let stakingPot = "7L53bUTCbRAv4KC8NGQC17Cv8VDWYgbeCnLkT3tShzisck4C";
  //hdx
  txs.push(
    api.tx.currencies.updateBalance(stakingPot, 0, "1000000000000000")
  ); 
}

function initStaking(api, txs) {
  txs.push(
    api.tx.staking.initializeStaking()
  );
}*/


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
    api.tx.currencies.updateBalance(omniAccount, 20, "1434000000000000000000")
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

  txs.push(
    api.tx.omnipool.addToken(20, "16420844565569051996", 1000000, omniAccount)
  );
}