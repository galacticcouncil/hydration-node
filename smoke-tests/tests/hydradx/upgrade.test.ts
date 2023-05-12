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

  const code = fs.readFileSync('./148.wasm').toString('hex')

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
    console.log(`Spec version before upgrade: ${chain.api.runtimeVersion.specVersion.toNumber()}`)
    const proposal = chain.api.tx.parachainSystem.authorizeUpgrade(blake2AsHex(`0x${code}`))
    const encodedProposal = proposal.method.toHex()
    console.log(encodedProposal)
    const encodedHash = blake2AsHex(encodedProposal);

    const tx11 = chain.api.tx.democracy
      .notePreimage(encodedProposal)

    const tx0 = await sendTransaction(tx11.signAsync(alice))
    await chain.chain.newBlock()
    await checkEvents(tx0, "preimage").redact({ number: true }).toMatchSnapshot()

    let external = chain.api.tx.democracy.externalProposeMajority(encodedHash);
    let fastTrack = chain.api.tx.democracy.fastTrack(encodedHash, 2, 1);
    let referendumNextIndex = (await chain.api.query.democracy.referendumCount()).toNumber();
    console.log(referendumNextIndex)

    const voteAmount = 1n * 10n ** BigInt(chain.api.registry.chainDecimals[0]);

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
    //NOTE: this is neccesary, woting is finished after 2 blocks.
    await chain.chain.newBlock()
    await chain.chain.newBlock()
    //await checkEvents(voteTx2).redact({ number: true }).toMatchSnapshot("vote")

    let entries = await chain.api.query.democracy.referendumInfoOf.entries()

    for (let entry of entries) {
      let idx = chain.api.registry.createType("u32", entry[0].toU8a().slice(-4)).toNumber()
      if (idx == referendumNextIndex) {
        console.log(idx)
        let f = entry[1].unwrap().isFinished;
        console.log(f)

      }
    }

    //NOTE: this is neccesary to wait for scheduler to dispatch
    await chain.chain.newBlock()
    await chain.chain.newBlock()

    console.log("enact")
    const enact = chain.api.tx.parachainSystem.enactAuthorizedUpgrade(`0x${code}`)
    const enactTx = await sendTransaction(enact.signAsync(alice))
    //NOTE: it's necessary to wait multiple blocks.
    await chain.chain.newBlock()
    await chain.chain.newBlock()
    await chain.chain.newBlock()

    await checkEvents(enactTx).toMatchSnapshot("enact")

    console.log("Spec version after upgrade: ", chain.api.runtimeVersion.specVersion.toNumber())
    assert.equal(chain.api.runtimeVersion.specVersion.toNumber(), 148)
  })
})
