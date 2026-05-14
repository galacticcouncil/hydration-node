/**
 * Deploy GasFaucet contracts to Sepolia
 *
 * Usage:
 *   1. Set DEPLOYER_PRIVATE_KEY below
 *   2. Run: npx ts-node deploy-faucet.ts
 *   3. Update runtime hardcode with the deployed faucet address
 *   4. Rebuild runtime and restart chopsticks
 */

import { ethers } from 'ethers'
import * as fs from 'fs'
import * as path from 'path'

// ============================================
// CONFIGURATION - Update these values
// ============================================

// Private key for deployment (with 0x prefix)
const DEPLOYER_PRIVATE_KEY = '0x...' // TODO: Set your deployer private key

// MPC address that will call fund() - this is the derived caller address
// (pallet account as sender, user account id as path, chainId = polkadot:2034)
const MPC_ADDRESS = '0x508311e567d51cb659df2225c6f32c9f369f9470'

// Sepolia RPC
const SEPOLIA_RPC = 'https://ethereum-sepolia-rpc.publicnode.com'

// ============================================

const GasFaucetArtifact = JSON.parse(
  fs.readFileSync(path.join(__dirname, 'artifacts/GasFaucet.json'), 'utf8'),
)
const GasVoucherArtifact = JSON.parse(
  fs.readFileSync(path.join(__dirname, 'artifacts/GasVoucher.json'), 'utf8'),
)

async function main() {
  console.log('\n=== Faucet Contract Deployment ===\n')

  const provider = new ethers.JsonRpcProvider(SEPOLIA_RPC)
  const wallet = new ethers.Wallet(DEPLOYER_PRIVATE_KEY, provider)
  const deployerAddress = wallet.address

  console.log(`Deployer: ${deployerAddress}`)
  console.log(`MPC Address: ${MPC_ADDRESS}`)

  const balance = await provider.getBalance(deployerAddress)
  console.log(`Balance: ${ethers.formatEther(balance)} ETH\n`)

  if (balance < ethers.parseEther('0.02')) {
    throw new Error(
      'Insufficient balance. Need at least 0.02 ETH for deployment.',
    )
  }

  // Deploy GasVoucher
  console.log('📦 Deploying GasVoucher...')
  const voucherFactory = new ethers.ContractFactory(
    GasVoucherArtifact.abi,
    GasVoucherArtifact.bytecode.object,
    wallet,
  )
  const voucher = await voucherFactory.deploy(deployerAddress)
  await voucher.waitForDeployment()
  const voucherAddress = await voucher.getAddress()
  console.log(`   ✅ GasVoucher deployed at: ${voucherAddress}\n`)

  // Deploy GasFaucet
  console.log('📦 Deploying GasFaucet...')
  const faucetFactory = new ethers.ContractFactory(
    GasFaucetArtifact.abi,
    GasFaucetArtifact.bytecode.object,
    wallet,
  )
  const faucet = await faucetFactory.deploy(
    MPC_ADDRESS, // mpc
    voucherAddress, // voucher
    ethers.parseEther('0.05'), // threshold
    deployerAddress, // owner
  )
  await faucet.waitForDeployment()
  const faucetAddress = await faucet.getAddress()
  console.log(`   ✅ GasFaucet deployed at: ${faucetAddress}\n`)

  // Set faucet in voucher
  console.log('📝 Setting faucet in GasVoucher...')
  const voucherContract = new ethers.Contract(
    voucherAddress,
    ['function setFaucet(address _faucet) external'],
    wallet,
  )
  const tx = await voucherContract.setFaucet(faucetAddress)
  await tx.wait()
  console.log('   ✅ Faucet set in GasVoucher\n')

  console.log('='.repeat(50))
  console.log('DEPLOYMENT COMPLETE')
  console.log('='.repeat(50))
  console.log(`\nGasVoucher: ${voucherAddress}`)
  console.log(`GasFaucet:  ${faucetAddress}`)
  console.log(`MPC:        ${MPC_ADDRESS}`)
  console.log(`\n📋 Next steps:`)
  console.log(`   1. Update runtime/hydradx/src/assets.rs with faucet address:`)
  console.log(`      ${faucetAddress}`)
  console.log(`   2. Rebuild runtime: cargo build --release`)
  console.log(`   3. Restart chopsticks with new runtime`)
  console.log(`   4. Run the dispenser test\n`)
}

main().catch(console.error)
