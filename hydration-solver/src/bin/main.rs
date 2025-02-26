use hydration_solver::types::{Asset, AssetId, Intent, OmnipoolAsset};
use hydration_solver::v3::SolverV3;
use rand::Rng;
use std::time::Instant;

const DATA: &str = r##"[{"Omnipool":{"asset_id":100,"reserve":1266820919355504832882362,"hub_reserve":51038318293060725,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":1000771,"reserve":32002376429556860,"hub_reserve":26014476849111470,"decimals":12,"fee":[1826,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":0,"reserve":86787892196719287820,"hub_reserve":39349774149749914,"decimals":12,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":28,"reserve":4556265753708959318053,"hub_reserve":9612298398114154,"decimals":15,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":20,"reserve":867832381419449632355,"hub_reserve":84761286694072999,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":101,"reserve":629572314354454914,"hub_reserve":2197799364666895,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":16,"reserve":13914336495313970320555284,"hub_reserve":66021994665334248,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":11,"reserve":1009131601,"hub_reserve":35503394980025219,"decimals":8,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":14,"reserve":6471691646341811818,"hub_reserve":44889397455328376,"decimals":12,"fee":[1535,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":19,"reserve":990858849,"hub_reserve":34410872917154085,"decimals":8,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":31,"reserve":31148932675898826330936703,"hub_reserve":1951040402032030,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":33,"reserve":6993437475117384536714797,"hub_reserve":13119005909651208,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":15,"reserve":9176720393339485,"hub_reserve":253368708945365471,"decimals":10,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":13,"reserve":5389536572490594437378352,"hub_reserve":34974939704700272,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":27,"reserve":432086546530017808,"hub_reserve":2882049565264472,"decimals":12,"fee":[1500,1000000],"hub_fee":[888,1000000]}},{"Omnipool":{"asset_id":102,"reserve":14934592932069234854488578,"hub_reserve":589363778311062935,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":5,"reserve":27636964158378707,"hub_reserve":521484187516233687,"decimals":10,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":1000624,"reserve":952264575327865867062,"hub_reserve":7739733753488502,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":8,"reserve":5086493193287732516,"hub_reserve":29474085676606977,"decimals":12,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":1000765,"reserve":10415120493009989839,"hub_reserve":36587988309690706,"decimals":18,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":12,"reserve":86803118192888873,"hub_reserve":3525228080391990,"decimals":10,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":17,"reserve":687562983768946254,"hub_reserve":15028516256510867,"decimals":10,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":9,"reserve":48181473981847403159167513,"hub_reserve":77577321627031475,"decimals":18,"fee":[2655,1000000],"hub_fee":[500,1000000]}},{"Omnipool":{"asset_id":1000752,"reserve":743299641347,"hub_reserve":4098130243199764,"decimals":9,"fee":[1500,1000000],"hub_fee":[500,1000000]}},{"StableSwap":{"pool_id":100,"asset_id":10,"reserve":385396342492,"decimals":6,"fee":[200,1000000],"amplification":320}},{"StableSwap":{"pool_id":100,"asset_id":18,"reserve":362426820253036888064363,"decimals":18,"fee":[200,1000000],"amplification":320}},{"StableSwap":{"pool_id":100,"asset_id":21,"reserve":261168403527,"decimals":6,"fee":[200,1000000],"amplification":320}},{"StableSwap":{"pool_id":100,"asset_id":23,"reserve":313648570687,"decimals":6,"fee":[200,1000000],"amplification":320}},{"StableSwap":{"pool_id":101,"asset_id":11,"reserve":33466127,"decimals":8,"fee":[200,1000000],"amplification":5}},{"StableSwap":{"pool_id":101,"asset_id":19,"reserve":34162536,"decimals":8,"fee":[200,1000000],"amplification":5}},{"StableSwap":{"pool_id":102,"asset_id":10,"reserve":8864469279144,"decimals":6,"fee":[200,1000000],"amplification":100}},{"StableSwap":{"pool_id":102,"asset_id":22,"reserve":8201889911186,"decimals":6,"fee":[200,1000000],"amplification":100}}]"##;

fn load_amm_state() -> Vec<Asset> {
	serde_json::from_str(DATA).unwrap()
}

fn price(da: &OmnipoolAsset, db: &OmnipoolAsset, asset_a: AssetId, asset_b: AssetId) -> f64 {
	if asset_a == asset_b {
		1.
	} else if asset_b == 1u32 {
		da.hub_reserve as f64 / da.reserve as f64
	} else if asset_a == 1u32 {
		let p = db.hub_reserve as f64 / db.reserve as f64;
		1. / p
	} else {
		let p1 = db.reserve as f64 / db.hub_reserve as f64;
		let p2 = da.hub_reserve as f64 / da.reserve as f64;
		let r = p1 * p2;
		r
	}
}

pub(crate) fn generate_random_intents(c: u32, data: &[Asset]) -> Vec<Intent> {
	let random_pair = || {
		let mut rng = rand::thread_rng();
		loop {
			let idx_in = rng.gen_range(0..data.len());
			let idx_out = rng.gen_range(0..data.len());
			if idx_in == idx_out {
				continue;
			}
			let data_idx_in = match &data[idx_in] {
				Asset::Omnipool(v) => v,
				Asset::StableSwap(_) => continue,
			};
			let data_idx_out = match &data[idx_out] {
				Asset::Omnipool(v) => v,
				Asset::StableSwap(_) => continue,
			};
			let reserve_in = data_idx_in.reserve;
			//let reserve_out = data_idx_out.reserve;
			let amount_in = rng.gen_range(1..reserve_in / 4);
			let price = price(&data_idx_in, &data_idx_out, data_idx_in.asset_id, data_idx_out.asset_id);
			let p = 0.9f64;
			let amount_out = (price * amount_in as f64) as u128;
			let amount_out = (p * amount_out as f64) as u128;
			return (data_idx_in.asset_id, data_idx_out.asset_id, amount_in, amount_out);
		}
	};

	let mut intents = Vec::new();
	for i in 0..c {
		let (asset_in, asset_out, amount_in, amount_out) = random_pair();
		let partial = if i < c / 2 { true } else { false };
		intents.push(Intent {
			intent_id: i as u128,
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial,
		});
	}
	intents
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 3 {
		eprintln!("Usage: {} <output_file_path> <number_of_intents>", args[0]);
		std::process::exit(1);
	}

	let output_path = &args[1];
	let num_intents = args[2].parse::<u32>().unwrap_or_else(|e| {
		eprintln!("Invalid number of intents: {}", e);
		std::process::exit(1);
	});

	let data = load_amm_state();
	let intents = generate_random_intents(num_intents, &data);
	println!("Generated intents {:?}", intents.len());
	let result = std::panic::catch_unwind(|| {
		let start = Instant::now();
		let solution = SolverV3::solve(intents.clone(), data.clone()).unwrap();
		let duration = start.elapsed();
		println!(
			"Time elapsed to solve(): {:?} - resolved intents {:?}",
			duration,
			solution.resolved_intents.len()
		);
		println!("Solution: {:?}", solution.resolved_intents);
	});
	let (data_output, intent_output) = match result {
		Ok(_) => (output_path.to_string() + ".data", output_path.to_string() + ".intents"),
		Err(e) => {
			eprintln!("Error: {:?}", e);
			std::process::exit(1);
		}
	};
	let intent_serialized = serde_json::to_string_pretty(&intents).unwrap();
	let data_serialized = serde_json::to_string(&data).unwrap();
	std::fs::write(intent_output, intent_serialized).unwrap();
	std::fs::write(data_output, data_serialized).unwrap();
}
