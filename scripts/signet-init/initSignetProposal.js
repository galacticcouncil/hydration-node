import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { blake2AsHex } from '@polkadot/util-crypto'
import { writeFileSync } from 'fs'

// ============================================================
// Configuration — fill in before running
// ============================================================

// Production RPC
const RPC = 'wss://node.lark.hydration.cloud'

const SIGNER_URI = '//Alice'

const ADMIN = '15oF4uVJwmo4TdGW7VfQxNLavjCXviqxT9S1MgbjMNHr6Sp5'

const SIGNATURE_DEPOSIT = '1000000000000'

const CHAIN_ID = 'polkadot:e6b50b06e72a81194e9c96c488175ecd'

// Enactment moment for the referendum
const ENACTMENT = { After: 1 }

// ============================================================
// Usage:
//   node initSignetProposal.js              — preimage only (outputs hex)
//   node initSignetProposal.js --full-flow  — full governance flow
// ============================================================

const FULL_FLOW = process.argv.includes('--full-flow')

async function signAndSend(api, tx, signer, label) {
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, { nonce: -1 }, (result) => {
      const { status, dispatchError, events } = result
      if (dispatchError) {
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule)
          reject(
            new Error(
              `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`,
            ),
          )
        } else {
          reject(new Error(dispatchError.toString()))
        }
      } else if (status.isInBlock) {
        console.log(`${label} included in block ${status.asInBlock.toHex()}`)
        resolve({ events })
      }
    }).catch(reject)
  })
}

async function main() {
  console.log(`\nConnecting to ${RPC}...`)
  console.log(`Mode: ${FULL_FLOW ? 'Full governance flow' : 'Preimage only'}\n`)

  const provider = new WsProvider(RPC)
  const api = await ApiPromise.create({ provider })

  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version(),
  ])
  console.log(`Connected to ${chain} using ${nodeName} v${nodeVersion}\n`)

  const keyring = new Keyring({ type: 'sr25519' })
  const signer = keyring.addFromUri(SIGNER_URI)
  console.log(`Signer: ${signer.address}\n`)

  // 1. Build signet.initialize call
  const chainIdBytes = Array.from(new TextEncoder().encode(CHAIN_ID))
  const initCall = api.tx.signet.initialize(
    ADMIN,
    SIGNATURE_DEPOSIT,
    chainIdBytes,
  )

  const encodedCall = initCall.method.toHex()
  const encodedHash = blake2AsHex(encodedCall)
  const len = encodedCall.length / 2 - 1

  console.log(`signet.initialize encoded call: ${encodedCall}`)
  console.log(`Encoded call length: ${len} bytes`)
  console.log(`Preimage hash: ${encodedHash}`)
  console.log(`Preimage length: ${len}\n`)

  // 2. Note preimage on-chain
  console.log('Noting preimage on-chain...')
  const notePreimageTx = api.tx.preimage.notePreimage(encodedCall)
  await signAndSend(api, notePreimageTx, signer, 'notePreimage')
  console.log()

  // 3. Build referendum submission tx
  const submitTx = api.tx.referenda.submit(
    { system: 'Root' },
    { Lookup: { hash: encodedHash, len } },
    ENACTMENT,
  )

  const submitHex = submitTx.method.toHex()

  // 4. Output preimage info and referendum hex
  console.log('=== Proposal Details ===')
  console.log(`Admin: ${ADMIN}`)
  console.log(`Signature Deposit: ${SIGNATURE_DEPOSIT}`)
  console.log(`Chain ID: ${CHAIN_ID}`)
  console.log(`Preimage Hash: ${encodedHash}`)
  console.log(`Preimage Length: ${len}`)
  console.log(`\nReferendum Submission Hex:\n${submitHex}\n`)

  const output = [
    '=== Signet Initialization Proposal ===',
    '',
    `Admin: ${ADMIN}`,
    `Signature Deposit: ${SIGNATURE_DEPOSIT}`,
    `Chain ID: ${CHAIN_ID}`,
    '',
    `Preimage Hash: ${encodedHash}`,
    `Preimage Length: ${len}`,
    '',
    '--- Referendum Submission Hex (submit via governance UI on root track) ---',
    submitHex,
    '',
  ].join('\n')

  writeFileSync('signet-init-proposal.txt', output, 'utf8')
  console.log('Output written to signet-init-proposal.txt\n')

  if (!FULL_FLOW) {
    console.log('Done. Submit the referendum hex via governance UI on the root track.\n')
    await api.disconnect()
    return
  }

  // ============================================================
  // Full flow: submit referendum, place deposit, vote, wait
  // ============================================================

  // 5. Submit referendum
  console.log('Submitting referendum with Root origin...')
  await signAndSend(api, submitTx, signer, 'submitReferendum')

  const referendumIndex =
    parseInt((await api.query.referenda.referendumCount()).toString()) - 1
  console.log(`Referendum index: ${referendumIndex}\n`)

  // 6. Place decision deposit
  console.log('Placing decision deposit...')
  const decisionDepositTx =
    api.tx.referenda.placeDecisionDeposit(referendumIndex)
  await signAndSend(api, decisionDepositTx, signer, 'placeDecisionDeposit')
  console.log()

  // 7. Vote AYE
  const { data } = await api.query.system.account(signer.address)
  const free = data.free.toBigInt()
  const voteAmount = (free * 5n) / 10n

  console.log(`Signer free balance: ${free.toString()}`)
  console.log(`Vote amount (50%): ${voteAmount.toString()}`)
  console.log('Voting AYE...')

  const voteTx = api.tx.convictionVoting.vote(referendumIndex, {
    Standard: {
      balance: voteAmount,
      vote: { aye: true, conviction: 'Locked1x' },
    },
  })
  await signAndSend(api, voteTx, signer, 'vote')
  console.log()

  // 8. Check referendum status
  const info = await api.query.referenda.referendumInfoFor(referendumIndex)
  console.log('Referendum info:', info.toHuman())
  console.log(
    `\nDone. Referendum ${referendumIndex} submitted and voted on. Waiting for enactment.\n`,
  )

  await api.disconnect()
}

main()
  .catch(console.error)
  .finally(() => process.exit())
