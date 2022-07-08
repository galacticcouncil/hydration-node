use super::ChainSpec;

pub fn parachain_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../res/hydradx.json")[..])
}
