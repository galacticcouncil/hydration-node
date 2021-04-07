use super::*;
use crate::mock::*;
use frame_support::assert_eq;

pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

#[test]
fn no_previous_chain() {
    new_test_ext().execute_with(|| {
        assert_eq!(GenesisHistory::previous_chain(), Some(Chain { genesis_hash: BlockHash::from(vec![]), last_block_hash: BlockHash::from(vec![]) }));
    })
}

#[test]
fn some_previous_chain() {
    // TODO
}
