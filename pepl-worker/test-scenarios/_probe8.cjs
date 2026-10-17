const { ApiPromise, WsProvider } = require('@polkadot/api');
(async () => {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud') });
  const head = (await api.rpc.chain.getHeader()).number.toNumber();
  let found = 0, ok = 0, fail = 0;
  for (let b = head - 400; b < head && found < 12; b++) {
    const h = await api.rpc.chain.getBlockHash(b);
    const blk = await api.rpc.chain.getBlock(h);
    const evs = await (await api.at(h)).query.system.events();
    blk.block.extrinsics.forEach((x, i) => {
      if (x.method.section === 'ethereum' && x.method.method === 'transact') {
        found++;
        const e = evs.filter(r => r.phase.isApplyExtrinsic && r.phase.asApplyExtrinsic.eq(i));
        const good = e.some(r => r.event.method === 'ExtrinsicSuccess');
        good ? ok++ : fail++;
      }
    });
  }
  console.log(`EVM txs in last 400 blocks (post-break): found=${found} success=${ok} failed=${fail}`);
  // Fresh, uncached metadata read of the constant at head
  const hh = await api.rpc.chain.getBlockHash(head);
  console.log('WethAssetId const at head =', (await api.at(hh)).consts.dynamicEvmFee.wethAssetId.toString());
  await api.disconnect();
})().catch(e => { console.error('ERR', e.message); process.exit(1); });
