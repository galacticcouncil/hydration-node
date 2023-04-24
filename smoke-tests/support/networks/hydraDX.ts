import { Config } from './types'

export default {
  polkadot: {
    name: 'hydraDX' as const,
    endpoint: 'wss://rpc.hydradx.cloud',
  },
  kusama: {
    name: 'basilisk' as const,
    endpoint: 'wss://rpc.basilisk.cloud',
  },
  config: ({ alice }) => ({
    storages: {
      System: {
        Account: [[[alice.address], {data: {free: "1000000000000000000"}}]],
      },
      Tokens: {
        accounts: [
          [[alice.address, 2], {free: '0x0000000000000000000000000000000000000000000000056bc75e2d6310000'}],
          [[alice.address, 5], {free: 100e10}],
        ],
      },
      Council: {
        members: [
          alice.address,
        ]
      },
      TechnicalCommittee: {
        members: [
          alice.address,
        ]
      },


    },
  }),
} satisfies Config

export const hydraDX = {
  paraId: 2034,
  dai: 2,
  hdx: 0,
  dot: 5,
}

export const basilisk = {
  paraId: 2090,
  dai: 13,
}
