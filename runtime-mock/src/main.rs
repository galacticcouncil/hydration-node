use runtime_mock::hydradx_mocked_runtime;

fn main() {
	let path = std::path::PathBuf::from("./MOCK_SNAPSHOT");
	let ext = hydradx_mocked_runtime();
	scraper::save_externalities::<hydradx_runtime::Block>(ext, path).unwrap();
}
