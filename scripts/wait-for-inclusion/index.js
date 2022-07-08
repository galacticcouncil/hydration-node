const { ApiPromise, WsProvider } = require('@polkadot/api');

let api;
const id = process.argv[2];
let header = null;
async function checkParaHead() {
  let head = (await api.query.paras.heads(id)).toHex();
  if (header === null) {
    header = head;
    console.log(`parachain ${id} registered`)
    return;
  }
  if (header !== head) {
    console.log(`parachain ${id} block included`);
    process.exit();
  }
}

async function main() {
  const provider = new WsProvider('ws://127.0.0.1:9944');

  api = await ApiPromise.create({ provider });

  const [chain,, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version()
  ]);

  console.log(`connected to relay chain ${chain} ${nodeVersion}`);
  console.log(`waiting for parachain ${id} block`);

  let count = 0;
  await api.rpc.chain.subscribeNewHeads(async header => {
    console.log(`relay chain #${header.number}`);
    await checkParaHead();
    if (++count === 50) {
      if (header === null) {
        console.log(`parachain ${id} not registered`);
      } else {
        console.log(`parachain ${id} block not included in 50 relay blocks`);
      }
      process.exit(1);
    }
  });
}

main().catch(console.error);

setTimeout(() => {
  if (header == null) {
    console.log(`parachain ${id} not registered in 60 sec`);
    process.exit(1);
  }
}, 60000)

