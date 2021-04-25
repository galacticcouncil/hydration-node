use crate::mock::*;
use super::*;

use hex_literal::hex;

#[test]
fn no_previous_chain() {
    ExtBuilder::default().build().execute_with(|| {
        assert_eq!(GenesisHistory::previous_chain(), Chain::default());
    })
}

#[test]
fn some_previous_chain() {
    let chain = Chain { genesis_hash: vec![1,2,3].into(), last_block_hash: vec![6,6,6].into() };
    ExtBuilder { chain: chain.clone() }.build().execute_with(|| {
        assert_eq!(GenesisHistory::previous_chain(), chain.clone());
    })
}

#[test]
fn construct_using_hex() {
    let chain = Chain { genesis_hash: hex!["aa"].to_vec().into(), last_block_hash: hex!["bb"].to_vec().into() };
    ExtBuilder { chain: chain.clone() }.build().execute_with(|| {
        assert_eq!(GenesisHistory::previous_chain(), chain.clone());
    })
}

#[test]
fn sample_data() {
    let chain = Chain {
        genesis_hash: hex!["0ed32bfcab4a83517fac88f2aa7cbc2f88d3ab93be9a12b6188a036bf8a943c2"].to_vec().into(),
        last_block_hash: hex!["5800478f2cac4166d40c1ebe80dddbec47275d4b102f228b8a3af54d86d64837"].to_vec().into()
    };
    ExtBuilder { chain: chain.clone() }.build().execute_with(|| {
        assert_eq!(GenesisHistory::previous_chain(), chain.clone());
    })
}
