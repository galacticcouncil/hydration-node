import { afterAll, assert, beforeEach, describe, it } from 'vitest'
import { sendTransaction, testingPairs } from '@acala-network/chopsticks-testing'

import { checkEvents } from '../../support/helpers'
import { createNetworks } from '../../support/networks'

import * as fs from 'fs'

import { blake2AsHex } from "@polkadot/util-crypto";
describe.each([
  {
    name: 'hydraDX',
  },
] as const)('$name upgrade', async ({ name}) => {
  const { [name]: chain } = await createNetworks({ [name]: undefined })
  const { alice } = testingPairs()

  const head = chain.chain.head

  const code = fs.readFileSync('./38.wasm').toString()

  afterAll(async () => {
    await chain.teardown()
  })

  beforeEach(async () => {
    await chain.chain.setHead(head)
  })

  it.each([
    {
      name: 'gov upgrade',
    }
  ])('$name works', async () => {
    const proposal = chain.api.tx.parachainSystem.authorizeUpgrade(blake2AsHex(code))
    const encodedProposal = proposal.method.toHex()
    console.log(encodedProposal)
    let encodedHash = blake2AsHex(encodedProposal);

    const tx11 = chain.api.tx.preimage
      .notePreimage(encodedProposal)

    const tx0 = await sendTransaction(tx11.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(tx0, "preimage").redact({ number: true }).toMatchSnapshot()

    let external = chain.api.tx.democracy.externalProposeMajority(encodedHash);
    let fastTrack = chain.api.tx.democracy.fastTrack(encodedHash, 2, 1);
    let referendumNextIndex = (await chain.api.query.democracy.referendumCount()).toNumber();
    console.log(referendumNextIndex)

    const voteAmount = 1n * 10n ** BigInt(chain.api.registry.chainDecimals[0]);

    console.log("council")
    const c1 = chain.api.tx.council
      .propose(1, external, external.length)
    const c1tx = await sendTransaction(c1.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(c1tx).redact({ number: true }).toMatchSnapshot()

    console.log("tch committee")
    const tc = chain.api.tx.technicalCommittee
      .propose(1, fastTrack, fastTrack.length)
    const tctx = await sendTransaction(tc.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(tctx).redact({ number: true }).toMatchSnapshot("tech committee")


    console.log("vote")
    const voteTx = chain.api.tx.democracy
      .vote(referendumNextIndex, {
        Standard: {
          balance: voteAmount,
          vote: { aye: true, conviction: 1 },
        },
      });

    const voteTx2 = await sendTransaction(voteTx.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(voteTx2).redact({ number: true }).toMatchSnapshot("vote")

    let entries = await chain.api.query.democracy.referendumInfoOf.entries()

    for (let entry of entries) {
      let idx = chain.api.registry.createType("u32", entry[0].toU8a().slice(-4)).toNumber()
      if (idx == referendumNextIndex) {
        console.log(idx)
        let f = entry[1].unwrap().isFinished;
        console.log(f)

      }
    }

    console.log("enact")
    const enact = chain.api.tx.parachainSystem.enactAuthorizedUpgrade(code)
    const enactTx = await sendTransaction(enact.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(enactTx).toMatchSnapshot("enact")

    console.log(chain.api.runtimeVersion.specVersion.toNumber())

  })
})

