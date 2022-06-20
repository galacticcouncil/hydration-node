const { ApiPromise, WsProvider, Keyring } = require("@polkadot/api");
const { encodeAddress, cryptoWaitReady } = require("@polkadot/util-crypto");
const assert = require("assert");

const ACCOUNT_SECRET = process.env.ACCOUNT_SECRET || "//Alice";
const RPC = process.env.RPC_SERVER || "ws://127.0.0.1:9944";

const hdxAddress = (pubKey) => encodeAddress(pubKey, 63);
const filterNulls = array => array.filter(item => item);
const range = (start, end) => Array.from({ length: end - start }, (_, i) => start + i);
const chunkify = (a, size) => Array(Math.ceil(a.length / size)).fill().map((_, i) => a.slice(i * size, i * size + size));
const sendAndWait = (from, tx, nonce = -1) => new Promise(async (resolve, reject) => {
  try {
    await tx.signAndSend(from, {nonce}, receipt => {
      let {status} = receipt;
      if (status.isInBlock) {
        resolve(receipt);
      }
    });
  } catch (e) {
    reject(e);
  }
});

async function main() {
  await cryptoWaitReady();
  const provider = new WsProvider(RPC);
  const keyring = new Keyring({ type: "sr25519" });
  const api = await ApiPromise.create({provider});
  const [chain, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.version(),
  ]);
  console.log(`connected to ${RPC} (${chain} ${nodeVersion})`);
  const from = keyring.addFromUri(ACCOUNT_SECRET);
  console.log("active account:", hdxAddress(from.addressRaw));

  async function unclaimedStashes(era) {
    const points = await api.query.staking.erasRewardPoints(era);
    const stashes = Object.keys(points.toHuman().individual);
    const controllers = await Promise.all(
      stashes.map(stash => api.query.staking.bonded(stash)
        .then(controller => controller.toHuman()))
    ).then(filterNulls);
    return await Promise.all(
      controllers.map(controller => api.query.staking.ledger(controller)
        .then(ledger => ledger.toHuman())
        .then(ledger => !ledger?.claimedRewards.includes(String(era)) && ledger.stash))
    ).then(filterNulls);
  }

  const activeEra = await api.query.staking.activeEra().then(era => Number(era.toHuman().index));
  const history = await api.query.staking.historyDepth();
  const firstEra = activeEra - history;
  console.log('first era:', firstEra);
  console.log('active era:', activeEra);
  const availablePayouts = await Promise.all(
    range(firstEra, activeEra)
      .map(era => unclaimedStashes(era)
        .then(stashes => stashes.map(stash => [stash, era])))
  ).then(eras => eras.flat());

  console.log('available payouts:', availablePayouts.length);

  const payoutTxs = availablePayouts.map(([stash, era]) => api.tx.staking.payoutStakers(stash, era));
  const batch = api.tx.utility.batch(payoutTxs);
  const weightLimit = Number(api.consts.system.blockWeights.perClass.normal.maxExtrinsic);
  const cost = await batch.paymentInfo(from);
  const totalWeight = Number(cost.weight);
  console.log('cost of payout of everyone', cost.partialFee.toHuman());
  const batchSize = Math.ceil(payoutTxs.length / Math.ceil(totalWeight / weightLimit));
  const batches = chunkify(payoutTxs, batchSize).map(txs => api.tx.utility.batch(txs));
  await Promise.all(batches.map(batch => batch.paymentInfo(from).then(({weight}) => assert(Number(weight) < weightLimit, 'batch overweight'))));
  console.log('executed in', batches.length ,'batches of', batchSize);

  console.log('encoded first batch:');
  console.log(batches[0].toHex());

  if (process.argv[2] === 'submit') {
    let currentNonce = await api.rpc.system.accountNextIndex(from.address).then(n => n.toNumber())
    await Promise.all(batches.map((batch, n) =>
      sendAndWait(from, batch, currentNonce++)
        .then(() => console.log(`batch ${n} in block`))
        .catch(e => console.log(`batch ${n} failed: ${e}`))));
  }
}


main().then(() => {
  process.exit(0);
}).catch((e) => {
  console.error(e);
  process.exit(1);
});
