use crate::liquidity_mining::liquidity_mining::*;

use sp_arithmetic::FixedU128;

use std::vec;

use crate::assert_approx_eq;

#[test]
fn calculate_loyalty_multiplier_should_work() {
	let testing_values = vec![
		(
			0,
			FixedU128::from_inner(500_000_000_000_000_000),   //0.5
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(123_580_000_000_000_000),   //0.12_358
			FixedU128::from_inner(0),
		),
		(
			1,
			FixedU128::from_inner(504_950_495_049_504_950),   //0.504950495
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(160_097_500_000_000_000),   //0.1600975
			FixedU128::from_inner(62_500_000_000_000_000),    //0.0625
		),
		(
			4,
			FixedU128::from_inner(519_230_769_230_769_230),   //0.5192307692
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(253_420_000_000_000_000),   //0.25342
			FixedU128::from_inner(210_526_315_789_473_684),   //0.2105263158
		),
		(
			130,
			FixedU128::from_inner(782_608_695_652_173_913),   //0.7826086957
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(868_250_588_235_294_117),   //0.8682505882
			FixedU128::from_inner(896_551_724_137_931_034),   //0.8965517241
		),
		(
			150,
			FixedU128::from_inner(800_000_000_000_000_000),   //0.8
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(883481734104046242),        //0.8834817341
			FixedU128::from_inner(909090909090909090),        //0.9090909091
		),
		(
			180,
			FixedU128::from_inner(821_428_571_428_571_428),   //0.8214285714
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(900_701_182_266_009_852),   //0.9007011823
			FixedU128::from_inner(923_076_923_076_923_076),   //0.9230769231
		),
		(
			240,
			FixedU128::from_inner(852_941_176_470_588_235),   //0.8529411765
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(923_354_904_942_965_779),   //0.9233549049
			FixedU128::from_inner(941_176_470_588_235_294),   // 0.9411764706
		),
		(
			270,
			FixedU128::from_inner(864_864_864_864_864_864),   //0.8648648649
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(931_202_525_597_269_624),   //0.9312025256
			FixedU128::from_inner(947_368_421_052_631_578),   //0.9473684211
		),
		(
			280,
			FixedU128::from_inner(868_421_052_631_578_947),   //0.8684210526
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(933_473_069_306_930_693),   //0.9334730693
			FixedU128::from_inner(949_152_542_372_881_355),   //0.9491525424
		),
		(
			320,
			FixedU128::from_inner(880_952_380_952_380_952),   //0.880952381
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(941_231_311_953_352_769),   //0.941231312
			FixedU128::from_inner(955_223_880_597_014_925),   //0.9552238806
		),
		(
			380,
			FixedU128::from_inner(895_833_333_333_333_333),   //0.8958333333
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(949_980_992_555_831_265),   //0.9499809926
			FixedU128::from_inner(962_025_316_455_696_202),   //0.9620253165
		),
		(
			390,
			FixedU128::from_inner(897_959_183_673_469_387),   //0.8979591837
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(951_192_106_537_530_266),   //0.9511921065
			FixedU128::from_inner(962_962_962_962_962_962),   //0.962962963
		),
		(
			4000,
			FixedU128::from_inner(987_804_878_048_780_487),   //0.987804878
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(994_989_395_973_154_362),   //0.994989396
			FixedU128::from_inner(996_264_009_962_640_099),   //0.99626401
		),
		(
			4400,
			FixedU128::from_inner(988_888_888_888_888_888),   //0.9888888888
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(995_442_536_739_769_387),   //0.9954425367
			FixedU128::from_inner(996_602_491_506_228_765),   //0.9966024915
		),
		(
			4700,
			FixedU128::from_inner(989_583_333_333_333_333),   //0.9895833333)
			FixedU128::from_inner(1_000_000_000_000_000_000), //1
			FixedU128::from_inner(995_732_022_019_902_604),   // 0.995732022),
			FixedU128::from_inner(996_818_663_838_812_301),   //0.9968186638)
		),
	];

	for (periods, expected_multiplier_1, expected_multiplier_2, expected_multiplier_3, expected_multiplier_4) in
		testing_values.iter()
	{
		//1th curve test
		assert_eq!(
			calculate_loyalty_multiplier(*periods, FixedU128::from_inner(500_000_000_000_000_000), 100).unwrap(),
			*expected_multiplier_1
		);

		//2nd curve test
		assert_eq!(
			calculate_loyalty_multiplier(*periods, FixedU128::from(1), 50).unwrap(),
			*expected_multiplier_2,
		);

		//3rd curve test
		assert_eq!(
			calculate_loyalty_multiplier(*periods, FixedU128::from_inner(123_580_000_000_000_000), 23).unwrap(),
			*expected_multiplier_3,
		);

		//4th curve test
		assert_eq!(
			calculate_loyalty_multiplier(*periods, FixedU128::from_inner(0), 15).unwrap(),
			*expected_multiplier_4,
		);
	}
}

#[test]
fn calculate_accumulated_rps_should_work() {
	let testing_values = vec![
		(
			FixedU128::from(596850065_u128),
			107097_u128,
			58245794_u128,
			FixedU128::from_float(596850608.8601828_f64),
		),
		(
			FixedU128::from(610642940_u128),
			380089_u128,
			72666449_u128,
			FixedU128::from_float(610643131.1827203_f64),
		),
		(
			FixedU128::from(342873091_u128),
			328911_u128,
			32953786_u128,
			FixedU128::from_float(342873191.1905865_f64),
		),
		(
			FixedU128::from(678009825_u128),
			130956_u128,
			49126054_u128,
			FixedU128::from_float(678010200.134045_f64),
		),
		(
			FixedU128::from(579839575_u128),
			349893_u128,
			48822879_u128,
			FixedU128::from_float(579839714.5365983_f64),
		),
		(
			FixedU128::from(53648392_u128),
			191826_u128,
			5513773_u128,
			FixedU128::from_float(53648420.7436166_f64),
		),
		(
			FixedU128::from(474641194_u128),
			224569_u128,
			88288774_u128,
			FixedU128::from_float(474641587.1476472_f64),
		),
		(
			FixedU128::from(323929643_u128),
			117672_u128,
			43395220_u128,
			FixedU128::from_float(323930011.7811883_f64),
		),
		(
			FixedU128::from(18684290_u128),
			293754_u128,
			84347520_u128,
			FixedU128::from_float(18684577.1365836_f64),
		),
		(
			FixedU128::from(633517462_u128),
			417543_u128,
			43648027_u128,
			FixedU128::from_float(633517566.5354059_f64),
		),
		(
			FixedU128::from(899481210_u128),
			217000_u128,
			46063156_u128,
			FixedU128::from_float(899481422.2726082_f64),
		),
		(
			FixedU128::from(732260582_u128),
			120313_u128,
			91003576_u128,
			FixedU128::from_float(732261338.3902155_f64),
		),
		(
			FixedU128::from(625857089_u128),
			349989_u128,
			71595913_u128,
			FixedU128::from_float(625857293.5661806_f64),
		),
		(
			FixedU128::from(567721341_u128),
			220776_u128,
			75561456_u128,
			FixedU128::from_float(567721683.2539406_f64),
		),
		(
			FixedU128::from(962034430_u128),
			196031_u128,
			40199198_u128,
			FixedU128::from_float(962034635.065515_f64),
		),
		(
			FixedU128::from(548598381_u128),
			457172_u128,
			37345481_u128,
			FixedU128::from_float(548598462.688032_f64),
		),
		(
			FixedU128::from(869164975_u128),
			172541_u128,
			4635196_u128,
			FixedU128::from_float(869165001.8643163_f64),
		),
		(
			FixedU128::from(776275145_u128),
			419601_u128,
			32861993_u128,
			FixedU128::from_float(776275223.3172418_f64),
		),
		(
			FixedU128::from(684419217_u128),
			396975_u128,
			24222103_u128,
			FixedU128::from_float(684419278.0166962_f64),
		),
		(
			FixedU128::from(967509392_u128),
			352488_u128,
			77778911_u128,
			FixedU128::from_float(967509612.6569046_f64),
		),
	];

	for (accumulated_rps_now, total_shares, reward, expected_accumulated_rps) in testing_values.iter() {
		assert_approx_eq!(
			calculate_accumulated_rps(*accumulated_rps_now, *total_shares, *reward).unwrap(),
			*expected_accumulated_rps,
			FixedU128::from_float(0.000_000_11),
			"calculate_accumulated_rps"
		);
	}

	assert_eq!(
		calculate_accumulated_rps(FixedU128::from(1_u128), 0, 10_000_u128),
		Err(crate::MathError::DivisionByZero)
	);
}

#[test]
fn calculate_user_reward_should_work() {
	let testing_values = vec![
		(
			79_u128,
			1733800371_u128,
			259_u128,
			2333894_u128,
			FixedU128::from_inner(456_446_123_846_332_000_u128),
			142447228701_u128,
			169634504185_u128,
		),
		(
			61_u128,
			3117_u128,
			1148_u128,
			34388_u128,
			FixedU128::from_inner(621_924_695_680_678_000_u128),
			2072804_u128,
			1280987_u128,
		),
		(
			0_u128,
			3232645500_u128,
			523_u128,
			1124892_u128,
			FixedU128::from_inner(1_000_000_000_000_u128),
			565781_u128,
			1690671905827_u128,
		),
		(
			159_u128,
			3501142339_u128,
			317_u128,
			3309752_u128,
			FixedU128::from_inner(384_109_209_525_475_000_u128),
			212478410818_u128,
			340698768992_u128,
		),
		(
			352_u128,
			156_u128,
			596_u128,
			2156_u128,
			FixedU128::from_inner(100_703_041_057_143_000_u128),
			1677_u128,
			34231_u128,
		),
		(
			0_u128,
			192208478782_u128,
			4_u128,
			534348_u128,
			FixedU128::from_inner(104_779_339_071_984_000_u128),
			80557375135_u128,
			688276005645_u128,
		),
		(
			138_u128,
			36579085_u128,
			213_u128,
			1870151_u128,
			FixedU128::from_inner(129_927_485_118_411_000_u128),
			354576988_u128,
			2386984236_u128,
		),
		(
			897_u128,
			1_u128,
			970_u128,
			1_u128,
			FixedU128::from_inner(502_367_859_476_566_000_u128),
			35_u128,
			37_u128,
		),
		(
			4_u128,
			38495028244_u128,
			6_u128,
			2568893_u128,
			FixedU128::from_inner(265_364_053_378_152_000_u128),
			20427824566_u128,
			56559663029_u128,
		),
		(
			10_u128,
			13343864050_u128,
			713_u128,
			1959317_u128,
			FixedU128::from_inner(279_442_586_539_696_000_u128),
			2621375291532_u128,
			6759359176301_u128,
		),
		(
			29_u128,
			18429339175_u128,
			833_u128,
			3306140_u128,
			FixedU128::from_inner(554_635_100_856_657_000_u128),
			8218129641066_u128,
			6599055749494_u128,
		),
		(
			224_u128,
			39102822603_u128,
			586_u128,
			1839083_u128,
			FixedU128::from_inner(654_427_828_000_143_000_u128),
			9263569206758_u128,
			4891650736445_u128,
		),
		(
			36_u128,
			55755691086_u128,
			251_u128,
			3521256_u128,
			FixedU128::from_inner(802_407_775_824_621_000_u128),
			9618838494628_u128,
			2368631567606_u128,
		),
		(
			36_u128,
			258339226986_u128,
			77_u128,
			2106922_u128,
			FixedU128::from_inner(743_748_274_128_360_000_u128),
			7877711415708_u128,
			2714194783796_u128,
		),
		(
			383_u128,
			34812134025_u128,
			2491_u128,
			1442758_u128,
			FixedU128::from_inner(130_076_146_093_442_000_u128),
			9545503668738_u128,
			63838473413204_u128,
		),
		(
			117_u128,
			44358629274_u128,
			295_u128,
			2076570_u128,
			FixedU128::from_inner(495_172_207_692_510_000_u128),
			3909796472461_u128,
			3986037461741_u128,
		),
		(
			172_u128,
			64667747645_u128,
			450_u128,
			33468_u128,
			FixedU128::from_inner(326_047_919_016_893_000_u128),
			5861570070642_u128,
			12116063741200_u128,
		),
		(
			37_u128,
			68875501378_u128,
			82_u128,
			230557_u128,
			FixedU128::from_inner(176_816_131_903_196_000_u128),
			548023257587_u128,
			2551374073866_u128,
		),
		(
			41_u128,
			100689735793_u128,
			81_u128,
			2268544_u128,
			FixedU128::from_inner(376_605_306_400_251_000_u128),
			1516809283443_u128,
			2510777879733_u128,
		),
		(
			252_u128,
			16283442689_u128,
			266_u128,
			3797763_u128,
			FixedU128::from_inner(189_489_655_763_324_000_u128),
			43193817533_u128,
			184770582350_u128,
		),
		(
			20_u128,
			205413646819_u128,
			129_u128,
			3184799_u128,
			FixedU128::from_inner(543_081_681_209_601_000_u128),
			12159643178907_u128,
			10230441139565_u128,
		),
		(
			23_u128,
			100000_u128,
			155_u128,
			1210762_u128,
			FixedU128::from_inner(404_726_206_620_574_000_u128),
			4131623_u128,
			7857615_u128,
		),
		(
			11_u128,
			84495025009_u128,
			166_u128,
			468012_u128,
			FixedU128::from_inner(735_133_167_032_114_000_u128),
			9627839308653_u128,
			3468889099730_u128,
		),
		(
			198_u128,
			79130076897_u128,
			571_u128,
			830256_u128,
			FixedU128::from_inner(689_497_061_649_446_000_u128),
			20350862574442_u128,
			9164655277883_u128,
		),
		(
			30_u128,
			68948735954_u128,
			72_u128,
			3278682_u128,
			FixedU128::from_inner(238_786_980_081_793_000_u128),
			691487259752_u128,
			2204356371634_u128,
		),
		(
			54_u128,
			280608075911_u128,
			158_u128,
			0_u128,
			FixedU128::from_inner(504_409_653_378_878_000_u128),
			14720307919780_u128,
			14462931974964_u128,
		),
		(
			193_u128,
			22787841433_u128,
			1696_u128,
			2962625_u128,
			FixedU128::from_inner(623_942_971_029_398_000_u128),
			21370122208415_u128,
			12880000502759_u128,
		),
		(
			193_u128,
			22787841433_u128,
			193_u128,
			2962625_u128,
			FixedU128::from_inner(623_942_971_029_398_000_u128),
			0_u128,
			0_u128,
		),
	];

	for (
		accumulated_rpvs,
		valued_shares,
		accumulated_rpvs_now,
		accumulated_claimed_rewards,
		loyalty_multiplier,
		expected_user_rewards,
		expected_unclaimable_rewards,
	) in testing_values.iter()
	{
		assert_eq!(
			calculate_user_reward(
				FixedU128::from(*accumulated_rpvs),
				*valued_shares,
				*accumulated_claimed_rewards,
				FixedU128::from(*accumulated_rpvs_now),
				*loyalty_multiplier
			)
			.unwrap(),
			(*expected_user_rewards, *expected_unclaimable_rewards)
		);
	}
}

#[test]
fn calculate_valued_shares_should_work() {
	assert_eq!(
		calculate_valued_shares(10_000_000, 1_000_000_000_000_000_000_000).unwrap(),
		10_000_000_000_000_000_000_000_000_000
	);

	assert_eq!(calculate_valued_shares(16_874, 49_898_646).unwrap(), 841_989_752_604);

	assert_eq!(calculate_valued_shares(16_874_138_468_415_354, 0).unwrap(), 0);

	assert_eq!(calculate_valued_shares(0, 466_874_688_464).unwrap(), 0);
}

#[test]
fn calculate_global_farm_shares_should_work() {
	assert_eq!(calculate_global_farm_shares(0, FixedU128::from(0)).unwrap(), 0);

	assert_eq!(calculate_global_farm_shares(16_841_351, FixedU128::from(0)).unwrap(), 0);

	assert_eq!(
		calculate_global_farm_shares(16_841_351, FixedU128::from_inner(156_874_561_300_000_000)).unwrap(),
		2_641_979
	);

	assert_eq!(
		calculate_global_farm_shares(16_841_351, FixedU128::from_inner(18_641_535_156_874_561_300_000_000)).unwrap(),
		313_948_636_755_764
	);
}

#[test]
fn calculate_reward_should_work() {
	assert_eq!(
		calculate_reward(FixedU128::from(0), FixedU128::from(1), 168_416_531).unwrap(),
		168_416_531
	);

	assert_eq!(
		calculate_reward(FixedU128::from(684_131), FixedU128::from(19_874_646), 9_798_646).unwrap(),
		188_041_063_042_690
	);

	assert_eq!(
		calculate_reward(FixedU128::from(1_688_453), FixedU128::from(786_874_343), 58).unwrap(),
		45_540_781_620
	);

	//NOTE: start and now RPS are the same
	assert_eq!(
		calculate_reward(FixedU128::from(1_688_453), FixedU128::from(1_688_453), 268_413_545_346).unwrap(),
		0
	);
}

#[test]
fn calculate_yield_farm_rewards_should_work() {
	let testing_values = vec![
		(
			259_u128,
			299_u128,
			55563662_u128,
			2222546480_u128,
			40_000_000_000_000_000_000_u128,
		),
		(
			1148_u128,
			1151_u128,
			5671016_u128,
			17013048_u128,
			3_000_000_000_000_000_000_u128,
		),
		(
			523_u128,
			823_u128,
			61428_u128,
			18428400_u128,
			300_000_000_000_000_000_000_u128,
		),
		(
			317_u128,
			320_u128,
			527114_u128,
			1581342_u128,
			3_000_000_000_000_000_000_u128,
		),
		(
			596_u128,
			5684_u128,
			3011_u128,
			15319968_u128,
			5_088_000_000_000_000_000_000_u128,
		),
		(
			4_u128,
			37_u128,
			71071995_u128,
			2345375835_u128,
			33_000_000_000_000_000_000_u128,
		),
		(
			213_u128,
			678_u128,
			85452_u128,
			39735180_u128,
			465_000_000_000_000_000_000_u128,
		),
		(
			970_u128,
			978_u128,
			474403_u128,
			3795224_u128,
			8_000_000_000_000_000_000_u128,
		),
		(
			6_u128,
			28_u128,
			147690_u128,
			3249180_u128,
			22_000_000_000_000_000_000_u128,
		),
		(
			713_u128,
			876_u128,
			75987_u128,
			12385881_u128,
			163_000_000_000_000_000_000_u128,
		),
		(
			833_u128,
			8373_u128,
			7521_u128,
			56708340_u128,
			7_540_000_000_000_000_000_000_u128,
		),
		(
			586_u128,
			5886_u128,
			318_u128,
			1685400_u128,
			5_300_000_000_000_000_000_000_u128,
		),
		(
			251_u128,
			2591_u128,
			28732_u128,
			67232880_u128,
			2_340_000_000_000_000_000_000_u128,
		),
		(
			77_u128,
			80_u128,
			26611087_u128,
			79833261_u128,
			3_000_000_000_000_000_000_u128,
		),
		(
			2491_u128,
			2537_u128,
			85100506_u128,
			3914623276_u128,
			46_000_000_000_000_000_000_u128,
		),
		(
			295_u128,
			471_u128,
			358776_u128,
			63144576_u128,
			176_000_000_000_000_000_000_u128,
		),
		(
			450_u128,
			952_u128,
			356723_u128,
			179074946_u128,
			502_000_000_000_000_000_000_u128,
		),
		(
			82_u128,
			357_u128,
			932564_u128,
			256455100_u128,
			275_000_000_000_000_000_000_u128,
		),
		(
			81_u128,
			1557_u128,
			758404_u128,
			1119404304_u128,
			1_476_000_000_000_000_000_000_u128,
		),
		(
			266_u128,
			2564373_u128,
			5278_u128,
			13533356746_u128,
			2_564_107_000_000_000_000_000_000_u128,
		),
		(129_u128, 129_u128, 86085676_u128, 0_u128, 0),
	];

	for (
		yield_farm_accumulated_rpz,
		global_farm_accumuated_rpz,
		total_valued_shares,
		expected_rewards_from_global_farm,
		expected_delta_rpvs,
	) in testing_values.iter()
	{
		assert_eq!(
			calculate_yield_farm_rewards(
				FixedU128::from(*yield_farm_accumulated_rpz),
				FixedU128::from(*global_farm_accumuated_rpz),
				FixedU128::from(1),
				*total_valued_shares
			)
			.unwrap(),
			(
				FixedU128::from_inner(*expected_delta_rpvs),
				*expected_rewards_from_global_farm
			)
		);
	}
}

#[test]
fn calculate_global_farm_rewards_should_work() {
	let testing_values = vec![
		(
			FixedU128::from_inner(833_333_333_300_000), //0.000_833_333_333_3
			12578954_u128,
			156789_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			100,
			1_048_246_u128,
		),
		(
			FixedU128::from_inner(83_333_333_330_000_000), //0.083_333_333_33
			1246578_u128,
			4684789_u128,
			FixedU128::from_inner(500_000_000_000_000_000), //.5
			10,
			519_407_u128,
		),
		(
			FixedU128::from_inner(36_666_666_670_000_000), //0.036_666_666_67
			3980_u128,
			488_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			58,
			8_464_u128,
		),
		(
			FixedU128::from_inner(166_666_666_700_000_000), //0.166_666_6667
			9897454_u128,
			1684653_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			1649575_u128,
		),
		(
			FixedU128::from_inner(6_250_000_000_000_000), //0.006_25
			1687_u128,
			28_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			10_u128,
		),
		(
			FixedU128::from_inner(12_500_000_000_000_000), //0.0125
			3879_u128,
			7_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1_000,
			7_000_u128,
		),
		(
			FixedU128::from_inner(133_333_333_300_000_000), //0.133_333_333_3
			35189_u128,
			468787897_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			10,
			46_918_u128,
		),
		(
			FixedU128::from_inner(3_111_392_405_000_000), //0.003_111_392_405
			48954_u128,
			161_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			152_u128,
		),
		(
			FixedU128::from_inner(375_000_000_000_000), //0.000_375
			54789782_u128,
			3_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			3_u128,
		),
		(
			FixedU128::from_inner(138_571_428_600_000_000), //0.138_571_428_6
			17989865464312_u128,
			59898_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			59898_u128,
		),
		(
			FixedU128::from_inner(37_500_000_000_000_000), //0.0375
			2_u128,
			7987_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			0_u128,
		),
		(
			FixedU128::from_inner(78_750_000_000_000_000), //0.078_75
			5000000000000_u128,
			498741_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			498741_u128,
		),
		(
			FixedU128::from_inner(40_000_000_000_000_000), //0.04
			5468_u128,
			8798_u128,
			FixedU128::from_inner(1_000_000_000_000_000_000),
			1,
			218_u128,
		),
		(
			FixedU128::from_inner(0),
			68797_u128,
			789846_u128,
			FixedU128::from_inner(500_000_000_000_000_000),
			1_000,
			0_u128,
		),
		(
			FixedU128::from_inner(152_207_001_522),
			648_006_082_473_472_682_238_u128,
			380_517_503_805_175_u128,
			FixedU128::from(1),
			10,
			986_310_627_813_051_u128,
		),
		(
			FixedU128::from(1), //max value for yield_per_period
			100_000_000_000_000_000_000_000_000_000_u128,
			u128::max_value() / 1_000_000,
			FixedU128::from(3),
			1_000_000,
			300_000_000_000_000_000_000_000_000_000_000_000_u128,
		),
		(
			FixedU128::from_inner(10_000_000),
			100_000_000_000_u128,
			1_000_000_000_000_000_000_u128,
			FixedU128::from(1),
			532_895,
			532_895,
		),
	];

	for (
		yield_per_period,
		total_shares_z,
		max_reward_per_period,
		price_adjustment,
		periods_since_last_update,
		expected_rewards,
	) in testing_values.iter()
	{
		assert_eq!(
			calculate_global_farm_rewards(
				*total_shares_z,
				*price_adjustment,
				*yield_per_period,
				*max_reward_per_period,
				*periods_since_last_update
			)
			.unwrap(),
			*expected_rewards
		);
	}
}
