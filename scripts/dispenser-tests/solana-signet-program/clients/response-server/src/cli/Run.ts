import { ChainSignatureServer } from '../server/ChainSignatureServer.js';
import { envConfig } from '../config/EnvConfig.js';
import type { ServerConfig } from '../types/index.js';

async function main() {
  const config: ServerConfig = {
    solanaRpcUrl: envConfig.SOLANA_RPC_URL,
    solanaPrivateKey: envConfig.SOLANA_PRIVATE_KEY,
    mpcRootKey: envConfig.MPC_ROOT_KEY,
    infuraApiKey: envConfig.INFURA_API_KEY,
    programId: envConfig.PROGRAM_ID,
    isDevnet: envConfig.SOLANA_RPC_URL.includes('devnet'),
    verbose: envConfig.VERBOSE,
    bitcoinNetwork: envConfig.BITCOIN_NETWORK,
    substrateWsUrl: envConfig.SUBSTRATE_WS_URL,
    rpcUrl: envConfig.RPC_URL
  };

  const server = new ChainSignatureServer(config);
  await server.start();

  process.on('SIGINT', async () => {
    console.log('\n🛑 Shutting down...');
    await server.shutdown();
    process.exit(0);
  });

  process.on('SIGTERM', async () => {
    await server.shutdown();
    process.exit(0);
  });
}

main().catch((err) => {
  console.error('❌ Fatal error:', err);
  process.exit(1);
});
