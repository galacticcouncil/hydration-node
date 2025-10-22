const priv =
  '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a'
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
