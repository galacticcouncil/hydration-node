use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::hydradx_run_to_next_block;
use crate::polkadot_test_net::TestNet;
use crate::polkadot_test_net::HDX;
use crate::polkadot_test_net::LRNA;
use crate::polkadot_test_net::UNITS;
use amm_simulator::aave::ReserveData;
use amm_simulator::aave::Simulator;
use frame_support::assert_err;
use hex_literal::hex;
use hydra_dx_math::types::Ratio;
use hydradx_runtime::ice_simulator_provider::Aave;
use hydradx_runtime::Runtime;
use hydradx_traits::amm::AmmSimulator;
use hydradx_traits::amm::SimulatorError;
use hydradx_traits::amm::TradeResult;
use sp_core::U256;
use xcm_emulator::Network;

const DOT: u32 = 5;
const A_DOT: u32 = 1001;

pub const PATH_TO_SNAPSHOT: &str =
	"snapshots/aave-simulator/7e10e2d20d0eb4293b3b5da688c63cffbb24b2cda27fd3abc85bf13b3656c98c";

#[test]
fn create_snapshot_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		let expected_dot = ReserveData {
			configuration: U256::from_dec_str("753997831161164877079002568592629221489798055993152").unwrap(),
			liquidity_index: U256::from_dec_str("1035336136294736724440835214").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1065196554024159900141310364").unwrap(),
			variable_borrow_index: U256::from_dec_str("51028877334674195433308708").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("79060184166853553946851366").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("149060184166853553946851366").unwrap(),
			last_update_timestamp: U256::from_dec_str("1769589174").unwrap(),
			id: 3,
			atoken_address: sp_core::H160(hex!("02639ec01313c8775fae74f2dad1118c8a8a86da")),
			stable_debt_token_address: sp_core::H160(hex!("dc92f2fd6137b0bd5766ddf59c39c828b24f5248")),
			variable_debt_token_address: sp_core::H160(hex!("34321cb7334807eb718b3e1ddfaeb0c6c0403f1a")),
			interest_rate_strategy_address: sp_core::H160(hex!("b2dc5c391c6ed54880da06fe786f6f28d9fd99a6")),
			accrued_to_treasury: U256::from_dec_str("32814671262692").unwrap(),
			scaled_total_supply: U256::from_dec_str("99494530926548567").unwrap(),
		};

		let expected_hollar = ReserveData {
			configuration: U256::from_dec_str("365354519770431488").unwrap(),
			liquidity_index: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1017192592529644194792669728").unwrap(),
			variable_borrow_index: U256::from_dec_str("0").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("48790164996148630000000000").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("0").unwrap(),
			last_update_timestamp: U256::from_dec_str("1769569944").unwrap(),
			id: 10,
			atoken_address: sp_core::H160(hex!("8c0f3b9602374198974d2b2679d14a386f5b108e")),
			stable_debt_token_address: sp_core::H160(hex!("d95d27688f028addbe93fa0e19fb095ee1111dd1")),
			variable_debt_token_address: sp_core::H160(hex!("342923782ccaebf9c38dd9cb40436e82c42c73b5")),
			interest_rate_strategy_address: sp_core::H160(hex!("6277f67402f9a7032e4c90c796b74343418e3628")),
			accrued_to_treasury: U256::from_dec_str("0").unwrap(),
			scaled_total_supply: U256::from_dec_str("0").unwrap(),
		};

		let expected_gdot = ReserveData {
			configuration: U256::from_dec_str("753997831576548625741237039960066689952748640410356").unwrap(),
			liquidity_index: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			variable_borrow_index: U256::from_dec_str("0").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("0").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("90000000000000000000000000").unwrap(),
			last_update_timestamp: U256::from_dec_str("1769585214").unwrap(),
			id: 6,
			atoken_address: sp_core::H160(hex!("34d5ffb83d14d82f87aaf2f13be895a3c814c2ad")),
			stable_debt_token_address: sp_core::H160(hex!("6fc3b2f6584b3bd4502ebbc3738903a0968a8767")),
			variable_debt_token_address: sp_core::H160(hex!("6bc2a0ac2495c0cdf5116d0df5d8052fccbc4d4e")),
			interest_rate_strategy_address: sp_core::H160(hex!("5383a606ece147e94c1fa0b7375bc778f132b832")),
			accrued_to_treasury: U256::from_dec_str("0").unwrap(),
			scaled_total_supply: U256::from_dec_str("10487846414586294956464513").unwrap(),
		};

		let expected_geth = ReserveData {
			configuration: U256::from_dec_str("1128142248241621894702555553377248808488946780872512").unwrap(),
			liquidity_index: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			variable_borrow_index: U256::from_dec_str("0").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("0").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("90000000000000000000000000").unwrap(),
			last_update_timestamp: U256::from_dec_str("1769589342").unwrap(),
			id: 7,
			atoken_address: sp_core::H160(hex!("8a598fe3e3a471ce865332e330d303502a0e2f52")),
			stable_debt_token_address: sp_core::H160(hex!("62a0e4f1c38b4f41aeeac727f29854097b478811")),
			variable_debt_token_address: sp_core::H160(hex!("fb2e66d76d2841443ab41102369ff33df9bc9a93")),
			interest_rate_strategy_address: sp_core::H160(hex!("5383a606ece147e94c1fa0b7375bc778f132b832")),
			accrued_to_treasury: U256::from_dec_str("0").unwrap(),
			scaled_total_supply: U256::from_dec_str("2355034935436638803964").unwrap(),
		};

		let expected_usdt = ReserveData {
			configuration: U256::from_dec_str("379853410758302483957202436554183033238679701692224").unwrap(),
			liquidity_index: U256::from_dec_str("1045395624087717879065064539").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1079125208703227655761523015").unwrap(),
			variable_borrow_index: U256::from_dec_str("19728462736792637876639013").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("44583604606801630982965448").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("53072950575850203872870681").unwrap(),
			last_update_timestamp: U256::from_dec_str("1769589570").unwrap(),
			id: 1,
			atoken_address: sp_core::H160(hex!("c64980e4eaf9a1151bd21712b9946b81e41e2b92")),
			stable_debt_token_address: sp_core::H160(hex!("6863e05d3f794903e76056cc751c1b2006728380")),
			variable_debt_token_address: sp_core::H160(hex!("32a8090e20748e530670ff520c4abc903db7e127")),
			interest_rate_strategy_address: sp_core::H160(hex!("aa659cf1ce049ec00161d305b17e70a5c1a7382f")),
			accrued_to_treasury: U256::from_dec_str("1009336828").unwrap(),
			scaled_total_supply: U256::from_dec_str("9468205889716").unwrap(),
		};

		let snapshot = Simulator::<Aave<Runtime>>::snapshot();

		assert_eq!(snapshot.reserves.get(&5), Some(&expected_dot));
		assert_eq!(snapshot.reserves.get(&222), Some(&expected_hollar));
		assert_eq!(snapshot.reserves.get(&690), Some(&expected_gdot));
		assert_eq!(snapshot.reserves.get(&4200), Some(&expected_geth));
		assert_eq!(snapshot.reserves.get(&10), Some(&expected_usdt));

		assert_eq!(snapshot.reserves.len(), 16);
	});
}

#[test]
fn simulate_sell_should_fail_when_no_asset_is_reserve_asset() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		assert_err!(
			Sim::simulate_sell(HDX, LRNA, 1_000 * UNITS, 1, &snapshot),
			SimulatorError::AssetNotFound
		);
	});
}

#[test]
fn simulate_buy_should_fail_when_no_asset_is_reserve_asset() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		assert_err!(
			Sim::simulate_buy(HDX, LRNA, 1_000 * UNITS, 1, &snapshot),
			SimulatorError::AssetNotFound
		);
	});
}

#[test]
fn simulate_sell_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		let (s, r) = Sim::simulate_sell(DOT, A_DOT, 1_000 * UNITS, 1, &snapshot).unwrap();

		assert_eq!(s, snapshot);
		assert_eq!(
			r,
			TradeResult {
				amount_in: 1_000 * UNITS,
				amount_out: 1_000 * UNITS,
			}
		)
	});
}

#[test]
fn simulate_buy_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		let (s, r) = Sim::simulate_buy(DOT, A_DOT, 1_000 * UNITS, 1, &snapshot).unwrap();

		assert_eq!(s, snapshot);
		assert_eq!(
			r,
			TradeResult {
				amount_in: 1_000 * UNITS,
				amount_out: 1_000 * UNITS,
			}
		)
	});
}

#[test]
fn get_spot_price_should_fail_when_no_asset_is_reserve_asset() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		assert_err!(Sim::get_spot_price(HDX, LRNA, &snapshot), SimulatorError::AssetNotFound);
	});
}

#[test]
fn get_spot_price_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		type Sim = Simulator<Aave<Runtime>>;
		let snapshot = Sim::snapshot();

		let sp = Sim::get_spot_price(DOT, A_DOT, &snapshot).unwrap();

		assert_eq!(sp, Ratio { n: 1, d: 1 });
	});
}
