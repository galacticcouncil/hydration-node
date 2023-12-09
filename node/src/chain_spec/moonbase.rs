use super::*;

// The URL for the telemetry server.
const _TELEMETRY_URLS: [&str; 1] = ["wss://telemetry.hydradx.io:9000/submit/"];

pub fn parachain_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../res/moonbase.json")[..])
}
