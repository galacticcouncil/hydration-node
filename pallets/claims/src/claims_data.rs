use lazy_static;
use sp_std::vec;
lazy_static::lazy_static! {
pub static ref CLAIMS_DATA: vec::Vec<(&'static str, u128)> = vec::Vec::new()
}
