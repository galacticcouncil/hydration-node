const priv =
  '0x0aafff3d8934d620e90cd9eeeea1d63f76c5d35a912471974439560321e9323a'
import { SigningKey, getAddress, keccak256 } from 'ethers'

const sk = new SigningKey(priv)

// uncompressed pubkey (0x04 + X + Y)
const uncompressedPub = sk.publicKey

// compressed pubkey (0x02/0x03 + X)
const compressedPub = sk.compressedPublicKey

// Ethereum address = keccak256(uncompressedPub without 0x04) last 20 bytes
const address = getAddress(
  '0x' + keccak256('0x' + uncompressedPub.slice(4)).slice(-40)
)

console.log({ uncompressedPub, compressedPub, address })
