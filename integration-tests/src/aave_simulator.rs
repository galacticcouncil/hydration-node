use crate::ice::PATH_TO_SNAPSHOT;
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

#[test]
fn create_snapshot_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		hydradx_run_to_next_block();

		let expected_dot = ReserveData {
			configuration: U256::from_dec_str("753997831161164877079002568592629221489798055993152").unwrap(),
			liquidity_index: U256::from_dec_str("1045464948422465089107155597").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1079785809929338250408957895").unwrap(),
			variable_borrow_index: U256::from_dec_str("25387293470761311698916052").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("39445756760347147130571212").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("113891513520694294261142424").unwrap(),
			last_update_timestamp: U256::from_dec_str("1775492238").unwrap(),
			id: 3,
			atoken_address: sp_core::H160(hex!("02639ec01313c8775fae74f2dad1118c8a8a86da")),
			stable_debt_token_address: sp_core::H160(hex!("dc92f2fd6137b0bd5766ddf59c39c828b24f5248")),
			variable_debt_token_address: sp_core::H160(hex!("34321cb7334807eb718b3e1ddfaeb0c6c0403f1a")),
			interest_rate_strategy_address: sp_core::H160(hex!("74aa8048311db37f8ef0db76a4b035c19a36586e")),
			accrued_to_treasury: U256::from_dec_str("7273030205000").unwrap(),
			scaled_total_supply: U256::from_dec_str("71468032489613751").unwrap(),
		};

		let expected_hollar = ReserveData {
			configuration: U256::from_dec_str("2671197528984125440").unwrap(),
			liquidity_index: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1026085346880660334471661913").unwrap(),
			variable_borrow_index: U256::from_dec_str("0").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("44016888917752794000000000").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("0").unwrap(),
			last_update_timestamp: U256::from_dec_str("1775485962").unwrap(),
			id: 10,
			atoken_address: sp_core::H160(hex!("8c0f3b9602374198974d2b2679d14a386f5b108e")),
			stable_debt_token_address: sp_core::H160(hex!("d95d27688f028addbe93fa0e19fb095ee1111dd1")),
			variable_debt_token_address: sp_core::H160(hex!("342923782ccaebf9c38dd9cb40436e82c42c73b5")),
			interest_rate_strategy_address: sp_core::H160(hex!("39dfb27d814db32f904a17560837c9be8bf1b761")),
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
			last_update_timestamp: U256::from_dec_str("1775492304").unwrap(),
			id: 6,
			atoken_address: sp_core::H160(hex!("34d5ffb83d14d82f87aaf2f13be895a3c814c2ad")),
			stable_debt_token_address: sp_core::H160(hex!("6fc3b2f6584b3bd4502ebbc3738903a0968a8767")),
			variable_debt_token_address: sp_core::H160(hex!("6bc2a0ac2495c0cdf5116d0df5d8052fccbc4d4e")),
			interest_rate_strategy_address: sp_core::H160(hex!("5383a606ece147e94c1fa0b7375bc778f132b832")),
			accrued_to_treasury: U256::from_dec_str("0").unwrap(),
			scaled_total_supply: U256::from_dec_str("6102227836230613007916143").unwrap(),
		};

		let expected_geth = ReserveData {
			configuration: U256::from_dec_str("1128142248241621894702555553377248808488946780872512").unwrap(),
			liquidity_index: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1000000000000000000000000000").unwrap(),
			variable_borrow_index: U256::from_dec_str("0").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("0").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("90000000000000000000000000").unwrap(),
			last_update_timestamp: U256::from_dec_str("1775489466").unwrap(),
			id: 7,
			atoken_address: sp_core::H160(hex!("8a598fe3e3a471ce865332e330d303502a0e2f52")),
			stable_debt_token_address: sp_core::H160(hex!("62a0e4f1c38b4f41aeeac727f29854097b478811")),
			variable_debt_token_address: sp_core::H160(hex!("fb2e66d76d2841443ab41102369ff33df9bc9a93")),
			interest_rate_strategy_address: sp_core::H160(hex!("5383a606ece147e94c1fa0b7375bc778f132b832")),
			accrued_to_treasury: U256::from_dec_str("0").unwrap(),
			scaled_total_supply: U256::from_dec_str("1895632023631277681532").unwrap(),
		};

		let expected_usdt = ReserveData {
			configuration: U256::from_dec_str("379853410924455983430316920667158915773722773692224").unwrap(),
			liquidity_index: U256::from_dec_str("1048831948416008838193948051").unwrap(),
			current_liquidity_rate: U256::from_dec_str("1087751222982553415429376062").unwrap(),
			variable_borrow_index: U256::from_dec_str("29319922340343707644029565").unwrap(),
			current_variable_borrow_rate: U256::from_dec_str("51579857797538693765744510").unwrap(),
			current_stable_borrow_rate: U256::from_dec_str("53947482224692336720718064").unwrap(),
			last_update_timestamp: U256::from_dec_str("1775492310").unwrap(),
			id: 1,
			atoken_address: sp_core::H160(hex!("c64980e4eaf9a1151bd21712b9946b81e41e2b92")),
			stable_debt_token_address: sp_core::H160(hex!("6863e05d3f794903e76056cc751c1b2006728380")),
			variable_debt_token_address: sp_core::H160(hex!("32a8090e20748e530670ff520c4abc903db7e127")),
			interest_rate_strategy_address: sp_core::H160(hex!("aa659cf1ce049ec00161d305b17e70a5c1a7382f")),
			accrued_to_treasury: U256::from_dec_str("525239578").unwrap(),
			scaled_total_supply: U256::from_dec_str("5129873488101").unwrap(),
		};

		let snapshot = Simulator::<Aave<Runtime>>::snapshot();
		assert_eq!(snapshot.reserves.get(&5), Some(&expected_dot));
		assert_eq!(snapshot.reserves.get(&222), Some(&expected_hollar));
		assert_eq!(snapshot.reserves.get(&690), Some(&expected_gdot));
		assert_eq!(snapshot.reserves.get(&4200), Some(&expected_geth));
		assert_eq!(snapshot.reserves.get(&10), Some(&expected_usdt));

		assert_eq!(snapshot.reserves.len(), 21);
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
