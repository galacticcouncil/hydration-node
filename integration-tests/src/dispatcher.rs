use hydradx_runtime::Dispatcher;

#[test]
fn testnet_aave_manager_can_be_set_in_dispatcher() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Dispatcher::aave_manager_accout(),
			hex!["aa7e0000000000000000000000000000000aa7e0000000000000000000000000"].into()
		);
	});
}
