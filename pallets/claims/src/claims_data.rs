use primitives::Balance;
use sp_std::vec;
lazy_static::lazy_static! {
pub static ref CLAIMS_DATA: vec::Vec<(&'static str, Balance)> = vec![
	("0x8202C0aF5962B750123CE1A9B12e1C30A4973557", 555),
	("0x8aF7764663644989671A71Abe9738a3cF295f384", 666),
	("0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c", 777),
];
}
