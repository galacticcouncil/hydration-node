use lazy_static;
use sp_std::vec;
lazy_static::lazy_static! {
pub static ref CLAIMS_DATA: vec::Vec<(&'static str, u128)> = vec![
	("0x8202c0af5962b750123ce1a9b12e1c30a4973557", 555),
	("0xb3e7104ea029874c36da42ca115c8c90b5938ef5", 666),
	("0x30503adcd76c9bf9d068a15be4a8cf6e874fef6c", 777),
	("0x19ad3978b233a91a30f9ddda6c6f6c92ba97b8f2", 888),
];
}
