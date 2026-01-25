import * as ecc from 'tiny-secp256k1';

const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';
const ROOT_PUBLIC_KEY = "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";

// Derive public key from private key
const privateKeyBytes = Buffer.from(MPC_ROOT_KEY.slice(2), 'hex');
const derivedUncompressed = ecc.pointFromScalar(privateKeyBytes, false)!;
const derivedCompressed = ecc.pointFromScalar(privateKeyBytes, true)!;

console.log('MPC_ROOT_KEY (private):        ', MPC_ROOT_KEY);
console.log('');
console.log('ROOT_PUBLIC_KEY (from test):   ', ROOT_PUBLIC_KEY);
console.log('Derived pubkey (uncompressed): ', '0x' + Buffer.from(derivedUncompressed).toString('hex'));
console.log('');
console.log('Match: ', ROOT_PUBLIC_KEY.toLowerCase() === ('0x' + Buffer.from(derivedUncompressed).toString('hex')));
console.log('');
console.log('Derived pubkey (compressed):   ', '0x' + Buffer.from(derivedCompressed).toString('hex'));
