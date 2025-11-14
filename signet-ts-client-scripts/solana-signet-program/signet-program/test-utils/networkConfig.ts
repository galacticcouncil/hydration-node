import * as anchor from "@coral-xyz/anchor";

export enum Network {
  LOCALNET = "localnet",
  DEVNET = "devnet",
  TESTNET = "testnet",
  MAINNET = "mainnet",
}

export function detectNetwork(provider: anchor.AnchorProvider): Network {
  const endpoint = provider.connection.rpcEndpoint.toLowerCase();

  if (endpoint.includes("localhost") || endpoint.includes("127.0.0.1")) {
    return Network.LOCALNET;
  } else if (endpoint.includes("devnet")) {
    return Network.DEVNET;
  } else if (endpoint.includes("testnet")) {
    return Network.TESTNET;
  } else if (endpoint.includes("mainnet")) {
    return Network.MAINNET;
  }

  // Default to localnet if we can't determine
  return Network.LOCALNET;
}

export function shouldUseMockSigner(network: Network): boolean {
  // Only use mock signer on localnet
  return network === Network.LOCALNET;
}
