use crate::*;

#[allow(dead_code)]
pub struct MockClient {}

impl traits::Client for MockClient {}

// StorageProvider impl lands with the real MockClient (W1/W2):
// use sc_client_api::StorageProvider;
// impl StorageProvider for MockClient {}
