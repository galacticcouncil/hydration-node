// Script to print all Tokens::Reserves storage entries
import { ApiPromise, WsProvider } from '@polkadot/api';

async function main() {
  let rpcAddr = process.argv[2] || 'wss://hydration.ibp.network';

  console.log(`\nConnecting to RPC node: ${rpcAddr}\n`);

  // Initialise the provider to connect to the local node
  const provider = new WsProvider(rpcAddr);

  // Create the API and wait until ready
  const api = await ApiPromise.create({ provider });

  // Retrieve the chain & node information
  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version()
  ]);

  console.log(`You are connected to chain ${chain} using ${nodeName} v${nodeVersion}\n`);

  console.log('Fetching all Tokens::Reserves entries...\n');

  // Query all entries from Tokens::Reserves storage
  const entries = await api.query.tokens.reserves.entries();

  // Filter for entries with id "depositc"
  const filteredEntries = entries.filter(([key, value]) => {
    const reserves = value.toHuman();
    return reserves.some(reserve => reserve.id === 'depositc');
  });

  console.log(`Found ${filteredEntries.length} Tokens::Reserves entries with id "depositc" (out of ${entries.length} total):\n`);

  // Loop through filtered entries and print them
  for (const [key, value] of filteredEntries) {
    // Decode the storage key to get the account and currency ID
    const [accountId, currencyId] = key.args;

    console.log('-----------------------------------');
    console.log(`Account: ${accountId.toString()}`);
    console.log(`Currency ID: ${currencyId.toString()}`);
    console.log(`Reserves: ${JSON.stringify(value.toHuman(), null, 2)}`);
    console.log('');
  }

  console.log(`\nTotal count: ${filteredEntries.length} entries with "depositc" reserves\n`);
  console.log('Done!\n');

  await api.disconnect();
}

main().catch(console.error).finally(() => process.exit());
