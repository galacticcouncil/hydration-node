import { afterAll, assert, beforeEach, describe, it } from 'vitest'
import { sendTransaction, testingPairs } from '@acala-network/chopsticks-testing'

import { checkEvents } from '../../support/helpers'
import { createNetworks } from '../../support/networks'

import { hydraDX } from '../../support/networks/hydraDX'
import { query } from '../../support/helpers/api'
import {queryTokenBalance} from "../../support/helpers/api/query";

const s = "0x0000000000000000000000000000000000000000000000056bc75e2d63100000";

describe.each([
  {
    name: 'hydraDX',
    asset: [hydraDX.dot,  2e10],
  },
  {
    name: 'hydraDX',
    asset: [hydraDX.dai,  "0x0000000000000000000000000000000000000000000000056bc75e2d63100000"],
  }
] as const)('$name omnipool liquidity', async ({ name, asset}) => {
  const { [name]: chain } = await createNetworks({ [name]: undefined })
  const { alice } = testingPairs()

  const head = chain.chain.head

  afterAll(async () => {
    await chain.teardown()
  })

  beforeEach(async () => {
    await chain.chain.setHead(head)
  })

  it.each([
    {
      name: 'addLiquidity',
      tx: chain.api.tx.omnipool.addLiquidity(asset[0], asset[1]),
    }
  ])('$name works', async ({ tx }) => {
    const tx0 = await sendTransaction(tx.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(tx0, 'omnipool', 'tokens').redact({ number: true }).toMatchSnapshot()
  })
})

describe.each([
  {
    name: 'hydraDX',
    trade: [hydraDX.dot, hydraDX.dai, 1e10],
  }
] as const)('$name omnipool trades', async ({ name, trade }) => {
  const { [name]: chain } = await createNetworks({ [name]: undefined })
  const { alice } = testingPairs()

  const head = chain.chain.head

  afterAll(async () => {
    await chain.teardown()
  })

  beforeEach(async () => {
    await chain.chain.setHead(head)
  })

  it.each([
    {
      name: 'sell',
      tx: chain.api.tx.omnipool.sell(trade[0], trade[1], trade[2], 0),
    }
  ])('$name works', async ({ tx }) => {
    const tx0 = await sendTransaction(tx.signAsync(alice))

    await chain.chain.newBlock()
    const lp: any = await query.tokens(trade[1])(chain, alice.address)
    assert(lp.free > 0)
    await checkEvents(tx0, 'omnipool', 'tokens').redact({ number: true }).toMatchSnapshot()
  })
})

