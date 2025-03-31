// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]

use ethabi::ethereum_types::{H160, H256, U256};
use frame_support::assert_ok;
pub use crate::tests::mock::*;
use frame_support::traits::Hooks;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use pallet_ethereum::Transaction;
use polkadot_primitives::EncodeAs;

#[test]
fn offchain_worker_should_fetch_oracle_data() {
	env_logger::init();
	let (mut ext, _pool_state, state) = ExtBuilder::default().build();
	ext.execute_with(|| {
		let block_num = 10;
		frame_system::Pallet::<Test>::set_block_number(block_num);

		price_oracle_response(&mut state.write());

		<Liquidation as Hooks<BlockNumberFor<Test>>>::offchain_worker(block_num);
	});
}

#[test]
fn parse_oracle_data_should_work() {
	env_logger::init();
	let (mut ext, _pool_state, state) = ExtBuilder::default().build();
	ext.execute_with(|| {
		let tx = pallet_ethereum::Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: U256::from(9264),
			gas_price: U256::from(5143629),
			gas_limit: U256::from(80674),
			action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
				hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
			)),
			value: U256::from(0), // 0x40	= 64	/ 120 = 288 / 80 = 128
			input: hex!(
				"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
				.encode_as(),
			signature: ethereum::TransactionSignature::new(
				444480,
				H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
				H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
			)
				.unwrap(),
		});

		println!("RRR: {:x?}\n", hex::encode(tx.encode_as()));
		// "8d24152600000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000876444f542f5553440000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008414156452f555344000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000029b5c33700000000000000000000000067acbce5000000000000000000000005939a32ea00000000000000000000000067acbce5"
		println!("RRRRR: {:?}    {:?}",
		H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
		H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
		);

		assert!(Liquidation::parse_oracle_transaction(tx).is_some());

	});
}
fn price_oracle_response(state: &mut sp_core::offchain::testing::OffchainState) {
	state.expect_request(sp_core::offchain::testing::PendingRequest {
        method: "GET".into(),
        uri: "https://omniwatch.play.hydration.cloud/api/borrowers/by-health".into(),
        response: Some(br#"{"lastGlobalUpdate":6902556,"lastUpdate":6902556,"borrowers":[["0x56b2ea0f360d095f0fe04d23f19784e1b9f7f92d",{"totalCollateralBase":0,"totalDebtBase":0.00032616,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1738720626433,"account":"7KZEYyPwwbXk6f69hJcWrCLx3WoDyLNXN9TGn5TfNXiToVky","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1acc506f91c6b8dfd37f1a9361d205363d9cc9cf",{"totalCollateralBase":0,"totalDebtBase":0.88864427,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1738720627236,"account":"7JChDxuBxpopLkj2rQTJ7DZi9kFKg3YBEW12HYa29FJAHoAR","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4cc7b36a1f27af1ca1b4ff66e259da96d300ca3d",{"totalCollateralBase":0,"totalDebtBase":0.00238417,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1738720627559,"account":"7KLEFLtuS4iQV57YjZJSWUKi3iPrSjzwMaU9oAtacCfJ8TA6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9845856b900dfd4e97369f6659a14f0a479bb784",{"totalCollateralBase":0,"totalDebtBase":0.95691704,"availableBorrowsBase":0,"currentLiquidationThreshold":0,"ltv":0,"healthFactor":0,"updated":1738720627912,"account":"7M3DDUaAFHi4i3AQosHE6kPmRRERfJdoeLVWk9Brht8BTPMk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x75362b10afd0e72dc9b6a103ef2fe3bb6739d186",{"totalCollateralBase":2.00544799,"totalDebtBase":1.79092857,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0078030024391202,"updated":1738720628392,"account":"7KATdGb3dCxMY7yfiSKprsrvQddYnN3bzum8M54JC6ATRcHC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa0160030ba577ad80e739b18a83382ad253f3d88",{"totalCollateralBase":0.00736032,"totalDebtBase":0.00569116,"availableBorrowsBase":0,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.034632658368417,"updated":1738720627782,"account":"7MDTUwHXX7jbwd8ia9oeP9aGKL2ENEbmdQJ4FXfPcsUjCJyS","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5c4464cb3a63c97b065d8576533a3b6c14f9fc52",{"totalCollateralBase":26.52489759,"totalDebtBase":19.77287638,"availableBorrowsBase":0.12079681,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.0731831657767135,"updated":1738720627062,"account":"7KgY1Qr7YambXcxAfDGYmab32gzhMbKsKTzEmteMXgDGP6NV","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xeb23f8b56c4a1c9fd4ad3f1f48e08aa35a716669",{"totalCollateralBase":51994.81763889,"totalDebtBase":43981.25399911,"availableBorrowsBase":1919.7710125,"currentLiquidationThreshold":91.68,"ltv":88.28,"healthFactor":1.0838446946577427,"updated":1738720627247,"account":"7KATdGbTFjWqTnwkC6nXadj2yRQrwR8wadS8usjh2EDudtiW","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x58f1dbf9ce44114d9b99dc5ba0903528aae6478f",{"totalCollateralBase":111530.26708414,"totalDebtBase":92091.27177826,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0899756126446076,"updated":1738720628025,"account":"7KcBM3YRJAvhNQn5ir96h2JJf8sXQWViMGUyguZRU7qGM9Vi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9334f5b92da1bed85fd33a60146b453d58bbcbaf",{"totalCollateralBase":8.38645455,"totalDebtBase":6.90862999,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.0925189380420126,"updated":1738720627118,"account":"7KATdGb9doRG1PLU3Y7TgGvTyAjGEnUqy2HzqrxbCRZLp73p","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x375eb67c78c22746db431a8827e33bfd1e9ec062",{"totalCollateralBase":177.97257524,"totalDebtBase":144.44415483,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1089082691405159,"updated":1738720628020,"account":"7JrA47rx6r4PqNoTNSYXtCdu3F4UQW5vhDT8C3rtKdvVAfbE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x26219557f2597e5d6194b4c8f9b23570220d6cac",{"totalCollateralBase":319615.21281221,"totalDebtBase":200174.73657486,"availableBorrowsBase":0,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.11767674980744,"updated":1738720626673,"account":"7JTZ5fjcmPLJpbe9UEHMNQbW2vy741RnxgYEQf7jU5b4ajXq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0cc8969938f6ce86ae3fe79c808dbd198397d501",{"totalCollateralBase":137800.47321951,"totalDebtBase":110850.28611795,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1188101559394834,"updated":1738720628410,"account":"7HtKStLktsuUsWuNYJeWxz66Jou87bcdhRo7ML9xhTrT7CHN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10817a5e4400515e85d2e0e8c2b190116b653c4d",{"totalCollateralBase":28769.71578451,"totalDebtBase":23018.49608415,"availableBorrowsBase":0,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1248668944922575,"updated":1738720627347,"account":"7KATdGahSqi1bU1oNjgwsiNJGAXgkbTGMgM5mxVrFXRqB9Dt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf09dfdc7dcd63b7d20cbcbe25d3de8eb5431e82f",{"totalCollateralBase":3499.31557745,"totalDebtBase":2438.16157504,"availableBorrowsBase":122.9874961,"currentLiquidationThreshold":78.79,"ltv":73.19,"healthFactor":1.1308154355704534,"updated":1738720627013,"account":"7P33iBgp3pzy6Xnj5PnwyHfpFLMTCKJiG3vzp5MmWRgBEYBf","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x3684116871a3890d447d79d9470e2a97eb6ea6f1",{"totalCollateralBase":5713.18305997,"totalDebtBase":4512.69571195,"availableBorrowsBase":57.85073603,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.139421995671879,"updated":1738720627407,"account":"7KATdGaq4acSFUaNgq259utpMkh4PB22jmhnfkbpigRDKGi1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1232508adcaf57c6e78a850f9d715e3694b52000",{"totalCollateralBase":24657.68250925,"totalDebtBase":17151.72587849,"availableBorrowsBase":1339.0702352,"currentLiquidationThreshold":79.99,"ltv":74.99,"healthFactor":1.1499530938682674,"updated":1738720626537,"account":"7J1R6PsCy9CgQs8ZXFLSUmAt5U44sMTMgxr8knUuSfJvjoRN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x33a7ec1055edd4cc35ef289156c7a7a078043474",{"totalCollateralBase":1308.44771152,"totalDebtBase":820.43526078,"availableBorrowsBase":20.50408341,"currentLiquidationThreshold":72.85,"ltv":64.27,"healthFactor":1.1618273901755205,"updated":1738720627577,"account":"7KATdGapVLrNftwfar346f5Sc97jRrMiraEzkAymFKvfatvs","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf42fc37ca29f3ba9ceb2e0f28ea6b450a2f3cabd",{"totalCollateralBase":2133.72073879,"totalDebtBase":1535.68551402,"availableBorrowsBase":74.84689962,"currentLiquidationThreshold":83.81,"ltv":75.48,"healthFactor":1.1644775801126106,"updated":1738720627804,"account":"7P7jA7dPQ8rMdhnU4oUkL3PFTkXrhvsFCT4oE8vzeZCK8KoH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc04455453c91e51576ad6ebf2802c3ce71df0748",{"totalCollateralBase":7968.57327277,"totalDebtBase":6118.14625319,"availableBorrowsBase":256.71236503,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.1722040710861839,"updated":1738720627591,"account":"7Mwen6fYUmsmWDRQMKLKrb5jtNCYAbaWJmGmXAowz1haUccv","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x521c02a4787b05d9e563e64ea7d5266688ba6498",{"totalCollateralBase":5164.31125445,"totalDebtBase":3075.72534077,"availableBorrowsBase":22.8614119,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1753383275812883,"updated":1738720627677,"account":"7KTDXrCFaJ8YbvNg7LXZ2121wWatxksDwtdBucqd4EuQJuiK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa06a0dfa4149e2cd131e81cd47adcf358f9e4ff5",{"totalCollateralBase":3915.79455088,"totalDebtBase":2631.43507325,"availableBorrowsBase":279.17501642,"currentLiquidationThreshold":79.55,"ltv":74.33,"healthFactor":1.1837702540700525,"updated":1738720626669,"account":"7MDtT9iC3zM1LpGU2QwoUTKpeVoDw89sF4vxofT6fd8np8C6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xdc8b1e22756cf161f243387abb4547bc8cb07862",{"totalCollateralBase":2765.12850521,"totalDebtBase":1621.77519066,"availableBorrowsBase":37.30191247,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.1935007791445431,"updated":1738720626471,"account":"7Naj9THL7j3v91HRDxscmvJ3LY2w6SYvPLZBM43Fgeu8U8Db","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x34cd97eea8d7958109c6db5e35eb407390555d9d",{"totalCollateralBase":6514.9883052,"totalDebtBase":4360.65437086,"availableBorrowsBase":525.58685804,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1952313118391222,"updated":1738720626653,"account":"7JnnrDVoGrXA68TuQMVasG8TD8D2iagjmBA3bSEYyBHphbvy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb833625dcf03dc9282433e23e1129fa20791bf6d",{"totalCollateralBase":31521.65843579,"totalDebtBase":21031.08007452,"availableBorrowsBase":2610.16375232,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.1990504842964202,"updated":1738720627463,"account":"7Mm5MrjELZ6NpZXJC3E63pnBAiLPxr5SqdhvFvRWeZMbomaP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa8db958cf2f81ad2bcad99a30c315066f500b95d",{"totalCollateralBase":382.12933786,"totalDebtBase":253.75482295,"availableBorrowsBase":32.84218045,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2047198423110799,"updated":1738720627702,"account":"7MQxZSPthgKaeZ48f23YGgQ9GGQ1Y9ZS9Xpr2Vqkhqrxgx5m","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x58358640a707b5a6998f846a227d5630a45a82bc",{"totalCollateralBase":1579.56470728,"totalDebtBase":1045.99313693,"availableBorrowsBase":138.68039353,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2080880086162231,"updated":1738720626890,"account":"7KbDQ7NLW97WYxrEyXdTJdrfanRNWnynqVWMfMGh61xdxNS9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x30a38296580a47261adad3fd2df82c70600c574f",{"totalCollateralBase":30.29616358,"totalDebtBase":20.0541574,"availableBorrowsBase":2.66796529,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2085738820420349,"updated":1738720628368,"account":"7JhLA2Y3PKJ7gGqkRF3JXAewCttb9NVqjtvg1vx6NzKvKPAL","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbcbd7706146d4290c5235c7080add8ace95113ca",{"totalCollateralBase":2259.09737334,"totalDebtBase":1312.00346569,"availableBorrowsBase":57.00954255,"currentLiquidationThreshold":70.4,"ltv":60.6,"healthFactor":1.2121953885187224,"updated":1738720626446,"account":"7Ms2a3Qsb3SsnSjxsCy22Rinothf8rPatoVXYoB14xkQhGi1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc02dc42dd88fce69f499dab19e663954b23eae8a",{"totalCollateralBase":1711.12897596,"totalDebtBase":1128.4083359,"availableBorrowsBase":154.93839607,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.21312749757222,"updated":1738720627812,"account":"7MwY5HKazABMVvB11zR6GAgVEPUseAyTd5CuPmsk21r88447","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfc89997e9e97b2c5466a6095a9b44b18ce255e4c",{"totalCollateralBase":441.93032296,"totalDebtBase":291.39986047,"availableBorrowsBase":40.04788175,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2132615911338016,"updated":1738720626540,"account":"7PJgEAtRFHarM74kH19mwfhrGrBrMQV6dCg3NxcAMtc419DZ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc0f05d300585d81d5b589ecf07a2197fc48df3f8",{"totalCollateralBase":34688.78514904,"totalDebtBase":22823.60312179,"availableBorrowsBase":3192.98573999,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2158916351264328,"updated":1738720627152,"account":"7MxXt83nnmMJ8gCqhmwmvM5nQvjHijfw6asMb7qqmZ4NUtbd","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10a41d7838b131eca8e6e26ca54ebb8ccbe383d0",{"totalCollateralBase":2346.98795012,"totalDebtBase":1505.40473355,"availableBorrowsBase":194.28393993,"currentLiquidationThreshold":78.28,"ltv":72.42,"healthFactor":1.2204174242348222,"updated":1738720626915,"account":"7HyNoaSRGa8z56b2ExsGKhJxCSaLcWYNavUJoRGNJWTd7JWQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x04ceb6a33f688b53a0c6684efb161ce1e4b3925d",{"totalCollateralBase":15.39794824,"totalDebtBase":8.81768318,"availableBorrowsBase":0.42108576,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.2223804768181747,"updated":1738720627472,"account":"7HhrtCdrGEL4yVGkJkVud4ANNhB1xKS8VvZwgqGa6bN8R2zE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x30f2e3363d11a8aaa2f88c4bd461eb650387441a",{"totalCollateralBase":1653.29562967,"totalDebtBase":1081.90378273,"availableBorrowsBase":158.06793952,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2225084382296474,"updated":1738720628327,"account":"7JhjjfjsXuzhKdGKu9iEzXhTHLmAyGydzRtK7CocjFh6oC8x","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x65bb5eb1e8f42fec9e867ed7a2e87f2a5e31e3f5",{"totalCollateralBase":53002.26647185,"totalDebtBase":38950.904065,"availableBorrowsBase":3450.90911248,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2246709279216315,"updated":1738720626681,"account":"7KtwnKVJe3WAXUGCvcNMTd9x6krq2Xk7DNAFTKe91buErt5s","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32e3589c27c9f931b938766a820f48827bc254a4",{"totalCollateralBase":308.14307637,"totalDebtBase":201.14085604,"availableBorrowsBase":29.96645124,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2255812466611793,"updated":1738720627806,"account":"7JkHDT7YEx7gpxELgmei6K55MYNGR9xA5rBep5pJppVUhh4G","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7802f0469d452910f6543615068013217b763ba8",{"totalCollateralBase":29120.41953446,"totalDebtBase":21376.07757162,"availableBorrowsBase":1920.25805595,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2260611187061565,"updated":1738720627225,"account":"7LJuuQn6hrJTyV5CZsQmCBB8yXJWyW4Pyikm88mdCkBF5XPF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x742611e564c8cdc958f99228ed257557a8d7ce0a",{"totalCollateralBase":184.89660225,"totalDebtBase":120.12477654,"availableBorrowsBase":18.54767515,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2313636375485406,"updated":1738720627810,"account":"7KATdGb3QrWi9JPXLaZSJdztMwr7ktAMDeHHw7UAMArG5TgD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8aee4e164d5d70ac67308f303c7e063e9156903e",{"totalCollateralBase":9210.00268065,"totalDebtBase":6714.8744224,"availableBorrowsBase":653.12772212,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2344240399997506,"updated":1738720628037,"account":"7Ljigfve9PdRqvSjiRGUjVf37rbX3n89ZmaitD2hQQhtLBMN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80ae7d7ac8d94958ee0f13724c0aa67c0a354654",{"totalCollateralBase":1946.82861168,"totalDebtBase":1102.2041684,"availableBorrowsBase":66.47704719,"currentLiquidationThreshold":70.03,"ltv":60.03,"healthFactor":1.2369433139951822,"updated":1738720626484,"account":"7LWHFPp9NkErAj9T7pqGC4U4pvfH2i2uqVzGAyZFDi8vWbiH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x40449d64ace905e8f97ffda6c75d794f96ac17c2",{"totalCollateralBase":40259.41944091,"totalDebtBase":29214.03208118,"availableBorrowsBase":2993.50347155,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2402765012420864,"updated":1738720627357,"account":"7K3pjTS11AXxZKbkR9BJ9diBfY9TyfzMzHFTuRDRScruT6Dj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x77410188f811d7caa863dcc2e9e263721bc49fd7",{"totalCollateralBase":15460.63246398,"totalDebtBase":9965.87264863,"availableBorrowsBase":1629.60169936,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.241086095243279,"updated":1738720627685,"account":"7LHvJ2hp6dSRqQfcqYLG85KNMBqPeLhA6g63JEypiX4Y5YAi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x06bd31f6c1ddf6fe8f91e210e1381c5af2447a9e",{"totalCollateralBase":310.56705065,"totalDebtBase":199.8393993,"availableBorrowsBase":33.08588869,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2432665499910758,"updated":1738720627929,"account":"7HkPmvWweUi1i2uKADrGodyd9iJABrx6Dpu5rZBkK9X8yFPy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eb853e3e58105d04a16fef7a29364155b6f81aa",{"totalCollateralBase":1567.22671686,"totalDebtBase":1007.09515277,"availableBorrowsBase":168.32488488,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2449482752860972,"updated":1738720626492,"account":"7Lpgqmp1uZ3V1kCv2nL8J4qVmFPnnV7sdxxFCwzwDQ4QMtkN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x247c8ef2fa79b07a5e9c2e83dad14e7e6e8d2be3",{"totalCollateralBase":44976.24665537,"totalDebtBase":28667.94895734,"availableBorrowsBase":5064.23603419,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2550949277132575,"updated":1738720627816,"account":"7JRQ1aRqhkV4JEd68rHCkp2AgzfyFiotSBzZZbfainc1PehJ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eeda641ca05e0ea26570cb33827c27ba8284ee6",{"totalCollateralBase":613.12487297,"totalDebtBase":389.18343441,"availableBorrowsBase":70.66022032,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2603308748831903,"updated":1738720626553,"account":"7LpxgV88VdbhNLTNxNLLt5mCDcMAzRdNgELzA26fecAjNqSt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc07da84351adbf69063b5ffdfece90862d82d764",{"totalCollateralBase":300.76732527,"totalDebtBase":190.88967873,"availableBorrowsBase":34.68581522,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2604864852873023,"updated":1738720627223,"account":"7Mwwomgp4tMUXRcXiHB2aCXyzWBtxG5xPtTnW3N8mC6R3yBB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x562d5086ab334d537769c9ff0c730b18a95d38bc",{"totalCollateralBase":3841.76867708,"totalDebtBase":2437.83844536,"availableBorrowsBase":443.48806245,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2607131319590554,"updated":1738720627107,"account":"7KYYs6wX1Ny5tn3NsdKcQvwWjhxJZ91epbjbiro7wKv8ieLi","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x725f795d66f177a474d0f91d4e9b124c4410fcd9",{"totalCollateralBase":170.69147504,"totalDebtBase":101.79778272,"availableBorrowsBase":14.57966496,"currentLiquidationThreshold":75.45,"ltv":68.18,"healthFactor":1.2651230162275189,"updated":1738720626529,"account":"7LBX76YvSTre8hhfVoctN2uHPT46L4EANVxpkGo7FtsyDdCE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6a30ef1cf090b1ce0eef2469bae6bbc92b05536f",{"totalCollateralBase":420.72641108,"totalDebtBase":265.67576565,"availableBorrowsBase":49.86904266,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.266886831158738,"updated":1738720627491,"account":"7Kznu1dZDhfBYFcHhA5FaXardkhdZkV4piA53XpjaG6SimCn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd247c203b7c5598692ee05611aa4fcea9b67c777",{"totalCollateralBase":339.9180726,"totalDebtBase":214.34301716,"availableBorrowsBase":40.59553729,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.268688206796165,"updated":1738720627345,"account":"7NMGf6czqfcxtSstPpjQiBmeGg3H39AvzjAvAXxYTLjNp8N7","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8820ad235ac91d3036a790624b2be637c130b6f9",{"totalCollateralBase":18068.60490583,"totalDebtBase":11386.34348497,"availableBorrowsBase":2165.1101944,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.269493050489868,"updated":1738720627598,"account":"7Lg3WDYohqb9NA3RKWqyHTWcywTq46hdkALL2ZDkXHzYPKHU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76e031a692cb70e5a263b8584ce49bc2fe2e354a",{"totalCollateralBase":24633.61895166,"totalDebtBase":17444.45188707,"availableBorrowsBase":2262.44327426,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.2709059132389715,"updated":1738720626599,"account":"7LHRXzocZ4hQFLw9mpMtbsVkq4a4jZTeLn1wRGdyqpU27TgE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80562ba15cbedd1d663909b5967325c21629c31c",{"totalCollateralBase":18352.2888624,"totalDebtBase":11528.7229605,"availableBorrowsBase":2235.4936863,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.273500208151697,"updated":1738720627530,"account":"7LVq1gSDYnj9muw7fq4WivYLZAK67cfBkVA3bXFtjRGt5nx4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x66508d2d43a34ffc714785c9bb4a4be22fcb5c38",{"totalCollateralBase":6294.08627134,"totalDebtBase":3938.24331983,"availableBorrowsBase":782.32138368,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2785571149746418,"updated":1738720626641,"account":"7Kui6f8NrK5zbGBUfZhriBd727zymtgWefaVNPQX6YG2Spz9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x74072c1b55ff8be8d496606a0ec366ccce9b5aaa",{"totalCollateralBase":40.32185952,"totalDebtBase":25.22463782,"availableBorrowsBase":5.01675682,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2788087523866776,"updated":1738720628407,"account":"7LDgyFGFDi69GNed1tEMFgkpGwpT8FvLgkNLsYzhBYWhQfsG","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb0ad14257fd51120132ea5c93a711c34fef8b3b1",{"totalCollateralBase":7449.64921017,"totalDebtBase":4113.6925301,"availableBorrowsBase":408.98950539,"currentLiquidationThreshold":70.67,"ltv":60.71,"healthFactor":1.2797911021079693,"updated":1738720627920,"account":"7MbD8PMoWeni2KPo7YoFibMcSR9YznTEv37aFD9qWjh6CKRd","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe8881d8b8548331bc5df86b2e729774f876f836e",{"totalCollateralBase":28.58819286,"totalDebtBase":17.66784649,"availableBorrowsBase":3.77329816,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.2944732286951175,"updated":1738720628050,"account":"7NrSq43HzxthQTRWs4JGFYBtGE4QGrPy6GvdLjj4raUVjBJr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56a9d2ae4e9d5ce4b31ef54ff39bb41fe41576d5",{"totalCollateralBase":19372.21530709,"totalDebtBase":10419.20688282,"availableBorrowsBase":1204.12230143,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3014954849701366,"updated":1738720627944,"account":"7KZBrLHXs8LuhwBKuuAcsHuas7n4Ju8SNGTATNJJS7acNiTx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x42d77c715c1d27342fb13a615444f5ec0d4ca1c2",{"totalCollateralBase":53.66036108,"totalDebtBase":32.90208878,"availableBorrowsBase":7.34318203,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.304728375971515,"updated":1738720627094,"account":"7K7CTYY1RULUV2qAo8L5s92oDqGVnzVnB4diTtBjv9rLqiSr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7a5cd9f77bcc8fe16a6de9a23a3532ff987241cc",{"totalCollateralBase":11769.99415313,"totalDebtBase":7216.02371606,"availableBorrowsBase":1611.47189879,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3048731119804577,"updated":1738720628215,"account":"7LMzi8NGEtDYN6pwQhjcB7MrqS69vRmXDXNrYfT71NSdj7qE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x329b4791f71da7fd9454ed3c8b63ec472c3bbb66",{"totalCollateralBase":2560.99512283,"totalDebtBase":1563.09524647,"availableBorrowsBase":357.65109565,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.310730170082007,"updated":1738720628022,"account":"7JjuomnuogtYAqYk47zRTqTkyYDYEgvxzBmvqeHgepo1F7pT","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x60e265013352b87debe7efa75798e7994185eb96",{"totalCollateralBase":279.77935927,"totalDebtBase":170.28490447,"availableBorrowsBase":39.54961498,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3144059252735005,"updated":1738720627028,"account":"7Knb8p3vnLFKnEoPr9BabjUisMUc9KuPsfLXVr6d6yWAQNWm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4425333101d35f55eccfe16b28f0e65007a0076c",{"totalCollateralBase":1029.02267282,"totalDebtBase":624.93744828,"availableBorrowsBase":146.82955634,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3172808583094566,"updated":1738720626787,"account":"7K8ubJP81cZNSNsgBix1B4eF14YsQyKXUZAcqtGw9ZfsnFDz","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x14210bc9e9eaac83a9a0fc8dd09643252e543bc9",{"totalCollateralBase":165.99265121,"totalDebtBase":100.77284676,"availableBorrowsBase":23.72164165,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3177569676706828,"updated":1738720627277,"account":"7J3x4RGJKwp2GR8hTGUo95dj7advnyt5xuLmhqB9MfqpNVWR","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6244f66e034b30ede22115174afde52882cf0ed7",{"totalCollateralBase":4306.57474722,"totalDebtBase":2612.73001702,"availableBorrowsBase":617.2010434,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3186436315029435,"updated":1738720628268,"account":"7KpQTsrUpKgD8TVWtCgVVxfZFbmFz2Fh2DaCNsLqg2WVah7i","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6c1297ad63c76e3dd7190cba99c673e5d9ba7a36",{"totalCollateralBase":372.32614197,"totalDebtBase":225.64941297,"availableBorrowsBase":53.59519351,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.32001634597472,"updated":1738720628056,"account":"7L3Fynz5DGbjj9nUFXyupNtaMedUPNzyDRPJpL4NNFW3EzvP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x56417b9cadf44d16d1bc6adabc16693f642e9139",{"totalCollateralBase":10524.49860759,"totalDebtBase":5546.46535668,"availableBorrowsBase":768.23380787,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3282601713967657,"updated":1738720626886,"account":"7KATdGawRT3erLU1kxR57V5uF7gWoMUF6RH1MDLCyB96E8WN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf03dcbda983b1f95e91802b525eeb51eae975e12",{"totalCollateralBase":31614.38433512,"totalDebtBase":16654.12999133,"availableBorrowsBase":2314.50060974,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3288036688857796,"updated":1738720628451,"account":"7P2Z8nPQwfDSWJGsTE1RhdRNgNm3vGGt1Pi3YGub8LNVSBJr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x502c5f63385e3334f63f48c8d79c71e4a4f33c20",{"totalCollateralBase":7404.69022918,"totalDebtBase":4514.09172062,"availableBorrowsBase":1079.4112785,"currentLiquidationThreshold":81.09,"ltv":75.54,"healthFactor":1.3301597925917423,"updated":1738720626841,"account":"7KQgJD8dSzdbGYQUDfJaE4rF1oC8yoLVLZnR4wueYW356dXT","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1237936b529a53060201af397ca61e820f3398fa",{"totalCollateralBase":1339.88734588,"totalDebtBase":805.49400861,"availableBorrowsBase":199.4215008,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.330748416800443,"updated":1738720628121,"account":"7J1Sf3WUwEN7R3QCdKYwmnABJj6Ypg128khg9pFymYQrKLwk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x581ff8509aedb85b8d92743011462a6e19c0b3b4",{"totalCollateralBase":4619.85828638,"totalDebtBase":2429.21892278,"availableBorrowsBase":342.69604905,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3312512800489473,"updated":1738720627695,"account":"7Kb6zjemapseQ2vA6yKP2MwpXjbZfdaUFhHTeb6u6szTHj41","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7298006c7b6cef27eb0b69cf8763020604e86343",{"totalCollateralBase":1880.44606606,"totalDebtBase":1127.65340659,"availableBorrowsBase":282.68114296,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3340596002801457,"updated":1738720628048,"account":"7LBou3W43cj1cq3z2SM2A8jNGCAc8JAjfushUFLQnrNe7E1E","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7cb0517b58ec990eddcc664c2907f57311e3b41e",{"totalCollateralBase":191.13466043,"totalDebtBase":100.27650923,"availableBorrowsBase":14.40428703,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3342532895029457,"updated":1738720628143,"account":"7LR3bnMvXAoJjnF5iUCivgRB8wm8K5cq84cxyHR59mTqzqMK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x30217481fbc101b43b882fb7237f0932b6ad0bf7",{"totalCollateralBase":5397.5008744,"totalDebtBase":2825.5082877,"availableBorrowsBase":412.99223694,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.3371932506896111,"updated":1738720626864,"account":"7JgfXEQ9QYbzwzdSiw3Z1R9joVxi58wXYQoAuXpnTq47QPS3","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1ad9d16ce64de2df5b556e1c0cf58b8428e6ce66",{"totalCollateralBase":36021.87230708,"totalDebtBase":24226.26996499,"availableBorrowsBase":4591.22788067,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3382037401226237,"updated":1738720628391,"account":"7JCmEdYEruZ6eQAoLRpU6btAyEcdjFpgDiusjqhyd4cKtB7D","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xeed4ffd9c654ee1b26957cf2a3e70a05e6078948",{"totalCollateralBase":254.00238442,"totalDebtBase":151.6551615,"availableBorrowsBase":38.84662682,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3398944390033174,"updated":1738720627290,"account":"7NzhxQ26KH1GQHrusE1b9azLR2465zkKhBR2NogSJJnsfeQn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76126bccbd03939a60016a0719775d47876f7eb2",{"totalCollateralBase":4717.98561975,"totalDebtBase":2816.5273372,"availableBorrowsBase":721.96187761,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3400858731065044,"updated":1738720628264,"account":"7LGNQcYfVDdT2w8AuSaQufFPJc5DEEu4EGtV3yj8RremH51a","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xae262a4f42c640e0875ace3c112955363657e76e",{"totalCollateralBase":1010.25022426,"totalDebtBase":601.4671475,"availableBorrowsBase":156.2205207,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3437145865227826,"updated":1738720628039,"account":"7MXtxKwumQoBXaT4x9v8nQxWCMQn6URyARqa5pJAwNNCckLr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xc890d07efe185262cf6ac3cf407a5c9e0e11f91f",{"totalCollateralBase":1906.55522547,"totalDebtBase":1129.7551155,"availableBorrowsBase":300.1613036,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3500661864274603,"updated":1738720626792,"account":"7N8Xt4Ryt8hvf8tS72GPj17arHiZMUyFJWxcRZHq5oKNTvqP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf4e29ca3fd2936858e1cd3a5c7a049b07f002853",{"totalCollateralBase":25.4583305,"totalDebtBase":15.06580125,"availableBorrowsBase":4.02794663,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3518474100406708,"updated":1738720628160,"account":"7P8eHbgdhdzDhytPQ16APwkLLXUdwn5haRFXgpPnMMcZyQSj","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcc8800567968f958e319dcef16a123aae0039da3",{"totalCollateralBase":4.16259663,"totalDebtBase":2.76022134,"availableBorrowsBase":0.56985596,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3572596210708232,"updated":1738720628498,"account":"7NDjTKfuJzvPdi25rJvNGvcG8nuJVavYznRfpAW4aUQzUnM3","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0a385a4bb1bf8ff092734e41c9386cf3288f401f",{"totalCollateralBase":149026.54523806,"totalDebtBase":82659.82947452,"availableBorrowsBase":19378.64604998,"currentLiquidationThreshold":75.65,"ltv":68.47,"healthFactor":1.3638859672138788,"updated":1738720627575,"account":"7HpxWDAbeb9boQRhkqEasYXfYfYG1sXYYU6EhDbu51uu5g4g","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf2de6aee2c7a3a4371b5f32bdd86192f7f16bd28",{"totalCollateralBase":1877.50098228,"totalDebtBase":1100.80489086,"availableBorrowsBase":307.32084585,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3644568608761969,"updated":1738720627474,"account":"7P5zwnCcbzozXZxSaTEvbsTpbUzQqjm7WFfykPvnh4p6Eixt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe03b3664deda3dbe47d584ba8eb1426ed9e61cf2",{"totalCollateralBase":256.83595707,"totalDebtBase":150.3078535,"availableBorrowsBase":42.3191143,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3669862277688638,"updated":1738720628154,"account":"7NfZbpyipeQDT2Q3iwfbpUpu8nqJQSKRiH1XggQSu6yVzAdJ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x78bc8df38324d1e072bff0987d181d19e3bdf31a",{"totalCollateralBase":48.32329608,"totalDebtBase":28.26438527,"availableBorrowsBase":7.97808679,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.367750845833273,"updated":1738720627916,"account":"7LKs3VsautxTfZ2AE6oYVyQENJfGHyzwsKfgTDiHogqWM4yB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcafcf9b276c0927c9f0098e90be3f7f3c9768949",{"totalCollateralBase":1842.00515175,"totalDebtBase":1207.29401325,"availableBorrowsBase":266.31010815,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3731573406193232,"updated":1738720627283,"account":"7NBi7Be1NpFU3Li3A5t2Hbsxdz2AEAB4scrQkgsHY5f8yXEk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8eb7ae465ffc861172e0a865b22b48779f43378c",{"totalCollateralBase":377.71795083,"totalDebtBase":219.60522786,"availableBorrowsBase":63.68323526,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3759889215963403,"updated":1738720627567,"account":"7LpgedKS6NaGXvmHLipBs3Yhbbh72RjeuiQMM7LukVpvs9T1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xdcb8a44a43fd35358fc40e9671dce2432e572a69",{"totalCollateralBase":30.7375055,"totalDebtBase":17.84903739,"availableBorrowsBase":5.20409174,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3776655772919528,"updated":1738720626683,"account":"7NaxfpDqzbtrNCVMZs2fMisdPbkPsSYhvcvsEqiGMvccNE8S","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x24a1b9d4bd6be959c5d55e66f3be0f6591c38b98",{"totalCollateralBase":621.9292269,"totalDebtBase":360.80583592,"availableBorrowsBase":105.64108426,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.378978198208297,"updated":1738720626778,"account":"7JRb3xYLmXcpwVBy1UfuhvQUAw8XxWErRpJ9gvnKbo8Vx23f","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x404b2963a43b0f6aca3bfe2268f7b5ae52d5ec91",{"totalCollateralBase":620.27807067,"totalDebtBase":359.39792362,"availableBorrowsBase":105.81062938,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3807048508846362,"updated":1738720626449,"account":"7K3rgFpPTE6ewGzunBNH9HH3L5vyog1qpk2caTcZo7a4txRB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd8c2ebc049bc98fee89a13df6aad06c97b3febb7",{"totalCollateralBase":1220.32211632,"totalDebtBase":795.3942392,"availableBorrowsBase":180.86345386,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3808119930496978,"updated":1738720628280,"account":"7NVmXp1Mc4KPtGwratR2uLcU89ywudWeem31pbLqFcu695o1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0a6679243e822e0538039d187529d67c1bb74d8d",{"totalCollateralBase":15066.57446576,"totalDebtBase":9817.00594185,"availableBorrowsBase":2236.25363076,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.3812680871847016,"updated":1738720627149,"account":"7HqCCr9PuYf81ghRo995fRvYpxiK71Uh2Cbh3WqgKMg4GWDR","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8ae6e65078a5263f4f335e160b8da0ed59518f69",{"totalCollateralBase":3490.51945725,"totalDebtBase":2016.76559455,"availableBorrowsBase":601.12399839,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.3846009537975434,"updated":1738720627552,"account":"7LjgV5d6FPWKGzjBC4Ewn8YMPtTaubFRdgTnAnuuiNnVh5tx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe14e33cc20a77cafbfe2ab8591b8dde9ffb1087f",{"totalCollateralBase":644.77527175,"totalDebtBase":366.96921725,"availableBorrowsBase":116.61223656,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4056225785515801,"updated":1738720627011,"account":"7NgyHnwk16FFVxCGBG2eGQUo3YHuN6MPFd6Hy6MBJuS2yWet","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba99240fa3a03fdf8ee4bc08a43d33e009e37a02",{"totalCollateralBase":6734.08625805,"totalDebtBase":3829.4762574,"availableBorrowsBase":1221.08843614,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4067900267118132,"updated":1738720627687,"account":"7KATdGbHXbLXjMThV8W5nxWB9Ep8YHudAsyDLQHtZ3ZbMfj4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb3ff54b7f24950945ba8f1778f52835a7ad30a7f",{"totalCollateralBase":2012.25747769,"totalDebtBase":1000.14238322,"availableBorrowsBase":207.21210339,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.408379704742656,"updated":1738720627593,"account":"7KATdGbGCt2GqD6VovBYYYzruU9skQx9adn8uRHEGqqtLPJU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6edaa7dc187523280a0c835d584db3d5e894a895",{"totalCollateralBase":3094.69220251,"totalDebtBase":1592.60612253,"availableBorrowsBase":391.4010485,"currentLiquidationThreshold":72.74,"ltv":64.11,"healthFactor":1.413456269108118,"updated":1738720626918,"account":"7L6uWMfnkzcMVSKY5rEdzbYYR2Z1p76GEXyKLraYu64vykB6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x6e17e05d80b1d365f3789e5e1bdee6979df35f89",{"totalCollateralBase":234940.42808705,"totalDebtBase":116121.89748308,"availableBorrowsBase":24842.35936915,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.4162557039244303,"updated":1738720626872,"account":"7L5uePUoBCZW9Fb63cne34vDVwgcBSuNeuyhwFDgPKs1usRr","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfc8660f1761ccc8b5aa586839e4a6dabea2a1388",{"totalCollateralBase":8518.12579138,"totalDebtBase":4811.01559573,"availableBorrowsBase":1578.43056038,"currentLiquidationThreshold":80.01,"ltv":75.01,"healthFactor":1.4166140828412492,"updated":1738720626546,"account":"7PJfGgBdnBzoUeYoy5hj1VsaXKdjspLKL8V7vt7soAEu2zgF","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba8e05dbe5f2831bda3bc369f19c4b90bd58a19e",{"totalCollateralBase":90.11890103,"totalDebtBase":50.48714572,"availableBorrowsBase":17.10203005,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.427989635616105,"updated":1738720627616,"account":"7MpAP5cKM8m61QKdPWwD46PN6ePhXA8NP8UazedB9vZYhRPs","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x8aa40cdeb25d51c3f0958576ec3d92820c801553",{"totalCollateralBase":8974.99306028,"totalDebtBase":5014.21804985,"availableBorrowsBase":1717.02674536,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4319270476150892,"updated":1738720627331,"account":"7LjLdHuY5qiMMCSkNH6Nxt6NZfPJ4FBBMpFUGTtb7fKzRRxq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xac91a74171112e9ea9529fd99c7e088b63239dd6",{"totalCollateralBase":2350.81221994,"totalDebtBase":1472.55188128,"availableBorrowsBase":408.09789467,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.4367785779547022,"updated":1738720627585,"account":"7MVpnkQYguGsNswpjtq7rPY7Gey81odPVzcgxwBZV155LgoS","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x1eda1d770859f4199819db8862f0968aba11190b",{"totalCollateralBase":630.12838987,"totalDebtBase":350.62284431,"availableBorrowsBase":121.97344809,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4377349339346017,"updated":1738720627006,"account":"7JJ1Wrhi3NX8hGQJBg5fjFDkipKP5n9UZdHawzEi28DSNcto","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xece792e16f847add756d2c421801ff82999d2333",{"totalCollateralBase":56317.9563808,"totalDebtBase":27364.05255925,"availableBorrowsBase":6426.72126923,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.4406699951039892,"updated":1738720627129,"account":"7KATdGbTcEbXAegK69QTWLNj1Y8EKkr6aTgHBcyHb7eum574","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb2c882ed0aaf258ab3a0b2bc31bca319856e4d58",{"totalCollateralBase":3252.15819077,"totalDebtBase":1802.23625137,"availableBorrowsBase":636.88239171,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4436101541305997,"updated":1738720626675,"account":"7MdyNbWtnau72BWDiShxadUcztcEBSzWs9STUoyHQCgVBLWD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd67d4219e96c3cf32a654c1fc9170e39c10f199b",{"totalCollateralBase":340.97621936,"totalDebtBase":164.52817083,"availableBorrowsBase":40.05756079,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.4507141989478594,"updated":1738720626784,"account":"7NSnjzjyci8ecCRYtLdYTKZZSxRNxfYxJSvvgaDZWTCCmJ1f","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5009e192ec169788c9c1f0202fe7c2bc79405ff8",{"totalCollateralBase":65544.83933708,"totalDebtBase":32056.56461891,"availableBorrowsBase":8738.54338449,"currentLiquidationThreshold":71.49,"ltv":62.24,"healthFactor":1.4617288595683988,"updated":1738720626637,"account":"7KQW3wEr1vwkWeaYNhMEwQqsyQnoH7JZqT9UBKLsAS6bY5cy","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x38116e97fa274dc8743ab46e51868d599f026c76",{"totalCollateralBase":685.20349936,"totalDebtBase":372.14759459,"availableBorrowsBase":141.75502993,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4729714969511447,"updated":1738720626489,"account":"7Js59NvGJuuwzyYtsArbFrMZNSVjree4bwdoikFfMx5r57bC","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xab0211d3c1ee22c9e0c727964318b366517b4658",{"totalCollateralBase":1055.42790495,"totalDebtBase":501.35772691,"availableBorrowsBase":131.89901606,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.473597580760182,"updated":1738720628018,"account":"7KATdGbEQQjtFPHk87hUaVH7N4tb8RcVxt3uWUVhFLpySmk3","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x39b812e2751f3a5a32e08b51f73c95c09d22dd26",{"totalCollateralBase":8324.61543496,"totalDebtBase":4505.71920618,"availableBorrowsBase":1737.74237004,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4780531238688002,"updated":1738720626597,"account":"7JuEhKuQJNikoS8UdushXb4tE4f8fBV8SQvyYfmzMjC7LBCD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xf8bd2aa8ee55c107cc630aa30121921892cf1eff",{"totalCollateralBase":2794.68520567,"totalDebtBase":1507.72992479,"availableBorrowsBase":588.28397946,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4828571933076145,"updated":1738720626423,"account":"7PDhMYNLR8tEmq5EwG3TNEn2zd9DmRxCf6rgsGNU3fnuxa8z","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5a6bb2cfc2079dae439cd2df5fb81e997df6e2bd",{"totalCollateralBase":11998.02595574,"totalDebtBase":6289.26591239,"availableBorrowsBase":2331.31573681,"currentLiquidationThreshold":77.9,"ltv":71.85,"healthFactor":1.4860974793746997,"updated":1738720626603,"account":"7Ke7b4acZ1jrTge166Uvr2iDJWCdmp5bkZhWfAisfuCzGPEp","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x06763e31c9266acc2ef2d09d5cf2591c3294a3dd",{"totalCollateralBase":590.66877279,"totalDebtBase":277.88686718,"availableBorrowsBase":76.51439649,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.487900976198986,"updated":1738720628158,"account":"7Hk2hSmj4Efg7QtYHA3yVRic9ZbQ5J8NCUbpN9CpBvExYL6H","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x05973b27fe36d10a72be2a946023420c72e470d1",{"totalCollateralBase":8062.19683522,"totalDebtBase":4311.97830984,"availableBorrowsBase":1734.66931658,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.4957768812198233,"updated":1738720627273,"account":"7HitT3HC79XQ7FDspXh9GshxJYfFfKrXsmTrh9eSx9UHjydY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7a1065d9facf9616d1cbf70e3b5f6bf92ea42ee4",{"totalCollateralBase":7040.24539815,"totalDebtBase":3751.88639131,"availableBorrowsBase":1528.2976573,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.501163876274376,"updated":1738720627026,"account":"7LMbzs2i7Qb9Q26Tzcy8aMxnxz2Wq7yt3psEwzNrTYq8oWYQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x805513ebb83c9eb6ff9e6af4ca3448edb0a097ee",{"totalCollateralBase":19231.89831844,"totalDebtBase":10244.78101992,"availableBorrowsBase":4179.14271891,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5017908752597373,"updated":1738720626587,"account":"7LVpgrZpjemnfHXfAaiigw3G8gdGVABRWmFvBpGAgUuebszL","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x00037ff736d90742c08089f482b996f5bb928447",{"totalCollateralBase":4320.34185955,"totalDebtBase":2281.1209064,"availableBorrowsBase":938.39784734,"currentLiquidationThreshold":79.68,"ltv":74.52,"healthFactor":1.5091038725881365,"updated":1738720626776,"account":"7HbaKoavnGFvMMna72zsZSj7eSDd8rGuq3Hv89wi2d1LH9Dt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xba796be5c16cde3238ec2c7dcae2ed2d82143d1e",{"totalCollateralBase":962.81912136,"totalDebtBase":509.79407813,"availableBorrowsBase":212.32026289,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5109145636124497,"updated":1738720628383,"account":"7Mp4G8GqW2YKXYRELYs8wW9rZSE5siiwDhjpdX4e4NpEMwMs","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x864159e69c32e12b73db380366c2ccc8c1611fe8",{"totalCollateralBase":931.05833565,"totalDebtBase":491.72766362,"availableBorrowsBase":206.56608812,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.514754453789703,"updated":1738720627386,"account":"7Ldb7duiNdnPrAsqqPkuCHfaUPid3nxoS1Sguso7B9Nk3sq9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x54c9b04ccd87caea676fe0959fea909e295e0935",{"totalCollateralBase":2286.20411347,"totalDebtBase":1072.30779081,"availableBorrowsBase":335.53670226,"currentLiquidationThreshold":71.05,"ltv":61.58,"healthFactor":1.5148150899780368,"updated":1738720628315,"account":"7KWjDp3Y72usf7btscKvVidH2yjxF4LcNE9zrC4TzVsSPgM2","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e125d12f06a9264d4481505bba2bf0381d08ddd",{"totalCollateralBase":16591.80022547,"totalDebtBase":8760.60019917,"availableBorrowsBase":3683.24996993,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.515129086890366,"updated":1738720627770,"account":"7Jdxwf1EtHvFNafmQpLbd4Fgqi4y8LycFw3qU7oNV5vAx6zU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x46c4c89314db831e91fcfed9a98fcfea6a17c8e9",{"totalCollateralBase":28490.04215503,"totalDebtBase":15042.85404438,"availableBorrowsBase":6324.67757189,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5151402557505427,"updated":1738720626981,"account":"7KCM6R3JAUf8XWV3LyDxZRJUtNY9ZNxChnCqFSrptPLLKd7N","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe2ac69b596da01ef1e56504a00bc218a73f9b06b",{"totalCollateralBase":19240.35222274,"totalDebtBase":11384.93438188,"availableBorrowsBase":4007.34739631,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.5209852265841999,"updated":1738720628274,"account":"7NimKnFYXP4YgWf32K6opo5UrH2ubs7W7XdcnzR7bPbntxVM","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe663963a1a18804315d0a1e135f00f0ca783e28b",{"totalCollateralBase":1747.15171987,"totalDebtBase":801.68155588,"availableBorrowsBase":246.60947604,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5255511305452387,"updated":1738720626880,"account":"7KATdGbSJWjyZnmnPoghJ5u6hKovB8Nq4qLbU8Lc6VKHsySD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x12df24d5be824f97f489a85d0a637444304dd5ed",{"totalCollateralBase":40008.77930424,"totalDebtBase":18281.46049764,"availableBorrowsBase":5723.8070849,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.5319424570364815,"updated":1738720626567,"account":"7J2JSB3cWXFUhRCZykyzKDvzdRenY61eLAUZsGaAMoTVDEt1","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe806cb404a70f85dd540d2c7df5f7da8318e757c",{"totalCollateralBase":29775.92420031,"totalDebtBase":15232.53933483,"availableBorrowsBase":6453.26626026,"currentLiquidationThreshold":78.55,"ltv":72.83,"healthFactor":1.5354622066105452,"updated":1738720628497,"account":"7NqnQtwX1NPjdDxrnCdJyyEjQWe73gA1Q826d9mzzhEVGCYn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x40516ae940349430d92f5b15e392cdc22c53758a",{"totalCollateralBase":35520.87119444,"totalDebtBase":18231.75566298,"availableBorrowsBase":7872.53257781,"currentLiquidationThreshold":78.99,"ltv":73.49,"healthFactor":1.5389596413614892,"updated":1738720627513,"account":"7K3tY3VH1GqRMmVM3GHos91fPVBfFur2eWUMf6F2kNGUmsmQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xddbcf39e88154fdce1fef51b596ce2abc8f38cd9",{"totalCollateralBase":10155.23886912,"totalDebtBase":5254.39592551,"availableBorrowsBase":2362.03322633,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5461703325128575,"updated":1738720626770,"account":"7NcHzrEwaqP1zpyVaoME78Zede3aK3MA2crxpvEx77NhVmUK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x689af88ce2eaa5c73c29d248d3bcb0d9163679a9",{"totalCollateralBase":1673.0604624,"totalDebtBase":797.36259937,"availableBorrowsBase":309.03228442,"currentLiquidationThreshold":74.08,"ltv":66.13,"healthFactor":1.5543783863567948,"updated":1738720628289,"account":"7KxiJRsksomESPYdYoMQWQ9r2cuVcDoLsp8SV4YAy61zFfWP","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x32406ea80e9b35a6da619b3c93ded9d6065735f5",{"totalCollateralBase":910.07242493,"totalDebtBase":464.51288276,"availableBorrowsBase":218.04143594,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5673579075225905,"updated":1738720627466,"account":"7JjSpWYd4GYpbusx1JapctncaRNxzuN1Utesj5oWJdT5PiYE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5226e4dc206a573da19cba5f94592151e00d74c2",{"totalCollateralBase":109388.80636848,"totalDebtBase":61753.00727141,"availableBorrowsBase":25758.03782337,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.5942531397529154,"updated":1738720627352,"account":"7KTGmNRKYRE831TrCLxtchusmaU5bVmLraWzyYZZVpdmGcP7","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xaf72a664e1021cac17ee165195f273138ee3046e",{"totalCollateralBase":144010.96387021,"totalDebtBase":72225.45405869,"availableBorrowsBase":35782.76884397,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.5951269894759263,"updated":1738720628385,"account":"7MZbiuUddZVqBwkCzmfcAYVGkmDBZaAnDfa3s7ib3QXuNmSZ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x38d4950cdea6241265a1c863a96bb7d9effbcbc7",{"totalCollateralBase":1611.80487277,"totalDebtBase":800.11522661,"availableBorrowsBase":408.73842797,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6115727526936734,"updated":1738720626708,"account":"7Jt57joQj8QMu5rqLwo58YmZcREpEKgr1BgktnzhJezdvcAt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xe64afe6914886cdcfea8da5f13e1e21aa11876cf",{"totalCollateralBase":3807.34913642,"totalDebtBase":1638.12911466,"availableBorrowsBase":646.28036719,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.6269440373405248,"updated":1738720626754,"account":"7NoWaQ1pua9UXtRLyus4F3MpnH4UsCBN9sP9FeD4Ntv9FfKw","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xced1c69114de2d69a8bd64344071732ae828b6de",{"totalCollateralBase":11330.08136979,"totalDebtBase":5500.8917278,"availableBorrowsBase":2996.66929954,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6477446829252607,"updated":1738720626772,"account":"7NGjTymGyEssHZhmaG3T7efsi8uYqXBNjC3PU3RaNhXdZoQg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x0453c43ab2dfb1ec81c0e40dbaa3a875f19e2236",{"totalCollateralBase":3158.87121791,"totalDebtBase":1484.6316085,"availableBorrowsBase":779.01550625,"currentLiquidationThreshold":77.77,"ltv":71.66,"healthFactor":1.6547230519038219,"updated":1738720628082,"account":"7HhEMsjWNUfLwTcTER9cBMy3BsFyBwiRvAT5oQmawPJtB7Ps","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd43c7e59fc0417deebdc50ec4afb4a80b37b5dab",{"totalCollateralBase":1659.63754753,"totalDebtBase":801.64618015,"availableBorrowsBase":443.0819805,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6562294824027797,"updated":1738720628450,"account":"7NPqQaBFeES16emvahBN7VGwmdmwRgs93uaqobzvX3dYKD2u","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2c4270dad627d74ff6e0fe99092d3b9302eb44b3",{"totalCollateralBase":24082.25359053,"totalDebtBase":11624.1504622,"availableBorrowsBase":6437.5397307,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6573944853062175,"updated":1738720627112,"account":"7Jbb8TFohE9xbBmbk2KuQijj1zJaHr1DBdMM2YBytACQfaGX","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd736b3431f5e5195fd039f1a0a6ad900045b7718",{"totalCollateralBase":4620.65052449,"totalDebtBase":2212.59520417,"availableBorrowsBase":1232.56182689,"currentLiquidationThreshold":79.7,"ltv":74.56,"healthFactor":1.6644067839790233,"updated":1738720627337,"account":"7KATdGbPG9jNnyMYzVbekSsJR2LkuyqTb1gNgtEN1vBq8AAY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xeca6171fdbe1822b3ac31971ed5082a7f2428bc4",{"totalCollateralBase":4869.06870142,"totalDebtBase":2045.94702383,"availableBorrowsBase":875.49419702,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.665902416480752,"updated":1738720626665,"account":"7NwqvcpGiriH8hYHUFQpX3DcB1Aoz2ZrAJcdwXd7mYwRFxdt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x76a497415fc75a15a2014b49e2d53bf748c30a8f",{"totalCollateralBase":3872.97557018,"totalDebtBase":1856.51712424,"availableBorrowsBase":1048.2145534,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6689210218884354,"updated":1738720627700,"account":"7KATdGb3uqkncqvKgMPuKDdizM1HpFCa9gDppUUWekawS6CN","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9c51f33019ae97643c1fac54d8bdc0a3eff9f0ec",{"totalCollateralBase":2239.68437533,"totalDebtBase":1071.65754018,"availableBorrowsBase":608.10574132,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.671940366284411,"updated":1738720627131,"account":"7KATdGbBTiF8WmRRZd5JLo6LBNFVyShMmx6YKiT1Q7XaUc1t","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x826860a86e166968aa399880b67997fc89ee9514",{"totalCollateralBase":15999.49417488,"totalDebtBase":7651.43292438,"availableBorrowsBase":4348.18770678,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.672836378022246,"updated":1738720627449,"account":"7LYYWwJWRdLc8Syr4jaVxGyjVFduaG2V95yS5ggtGw93QjE9","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7fe0e0bc5bd476181d53ce0bffbc8dc3a35d2ed9",{"totalCollateralBase":6714.8218514,"totalDebtBase":3200.68556364,"availableBorrowsBase":1835.43082491,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.6783458963119204,"updated":1738720627705,"account":"7LVEAn29nTJMKChtr9HV2cdB6QqjZ1k4shv95YjDKh4cmMwU","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x7bda8684105f3f0f79d44823ee52c77913b7c032",{"totalCollateralBase":7536.33162811,"totalDebtBase":3558.77623017,"availableBorrowsBase":2093.47249091,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.694140039314019,"updated":1738720627795,"account":"7LPx6Dbvggddcn8yMHxy3UaqV5DQ9u4CDGgquDLXyvKcFK8M","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x3261ec20e922ad196f633c259d26ec394c8a9d29",{"totalCollateralBase":102597.55929718,"totalDebtBase":45526.87296639,"availableBorrowsBase":25234.66368088,"currentLiquidationThreshold":75.98,"ltv":68.97,"healthFactor":1.7122552126861184,"updated":1738720627251,"account":"7JjcmXnhVWjAFyD2MVz7CCyGinoVThhLCKsgZYtXmoy9ATs6","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x10aeda815442c83aad7ba990aeace28ad5cfe8cc",{"totalCollateralBase":49535.26241301,"totalDebtBase":20057.10529777,"availableBorrowsBase":9664.05215004,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.7287980082033683,"updated":1738720627378,"account":"7KATdGahUuBZVHu6NdCDJ32Cs4fx7Pgb337e1DEJy7PaZyzg","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xae70debed84304554c2909745a07d44b8ba4b6de",{"totalCollateralBase":424524.58667249,"totalDebtBase":188921.69180259,"availableBorrowsBase":111429.4532682,"currentLiquidationThreshold":77.17,"ltv":70.75,"healthFactor":1.7340815679201362,"updated":1738720626781,"account":"7MYH9TjQccMqw5VJnhpUrjMjd8TMBCKet1jftikHMMJsj5M4","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x5c82f3a21b49dec00ccb3505adba2af107ca9414",{"totalCollateralBase":19399.27517031,"totalDebtBase":8930.16539591,"availableBorrowsBase":5619.29098182,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.737864804089504,"updated":1738720628381,"account":"7KgrbG52RBUgk6PPyfkiECSSJ3rJVCoDGSSrjNdcjBespseq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xde0419f94107c4dcf4b8fc79b0a97bb39c755022",{"totalCollateralBase":9486.19990532,"totalDebtBase":4340.99511205,"availableBorrowsBase":2773.65481694,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.748207433635228,"updated":1738720627793,"account":"7Ncf8jSz3XarDx8t2SXmnaURW81jHrEoXgH971sCanAb2V9n","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x908fdface285549efcc6bc7cd221f8a27ec1dc9b",{"totalCollateralBase":103.19510845,"totalDebtBase":40.91942592,"availableBorrowsBase":20.99763915,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.7653369834959796,"updated":1738720627818,"account":"7Ls6vKf1M8ZXUMNnsKuGVqFgo111qhxdsgahiFgxjAXGY8aa","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb2927ffd2bbb0a73a317ab830e2dccd5e30cb023",{"totalCollateralBase":458753.89038481,"totalDebtBase":201714.04079753,"availableBorrowsBase":128221.75716723,"currentLiquidationThreshold":77.95,"ltv":71.92,"healthFactor":1.7728000298893363,"updated":1738720628137,"account":"7MdhKzwvPmvkNv1PQThTPMQLWrxBgKMqcwAJe6EEV2P9uNDv","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x14e5b21d2eee0865adfb8783ac900540d67f0a89",{"totalCollateralBase":11250.52026916,"totalDebtBase":5052.13135699,"availableBorrowsBase":3385.75884488,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7815087493473925,"updated":1738720626848,"account":"7J4xUccGHKhmnWcRfW2gzCEkgzrAw7dd2qRuyBzXDEH4D54o","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb3de624935eb0778250f733030ca11eec6c6ffcd",{"totalCollateralBase":6418.22857831,"totalDebtBase":2878.2379117,"availableBorrowsBase":1935.43352203,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.7839327464133479,"updated":1738720626744,"account":"7MfPvE9zHjpSmEUdSxoftun9RdGHpPKKjzehXgjrreu5NSop","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x00574e7c5f8e90222e33415ea3f51bc4b64d8c79",{"totalCollateralBase":0.2043916,"totalDebtBase":0.0983015,"availableBorrowsBase":0.06192108,"currentLiquidationThreshold":86.78,"ltv":78.39,"healthFactor":1.8043573088915226,"updated":1738720626901,"account":"7KATdGaeCynstQNMWCugqbRtP8SZTtgYZyXkfGTB8uR2tV1t","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfcacbd0797bc102db06b206788f7c96909dbb214",{"totalCollateralBase":775.9003313,"totalDebtBase":341.44477545,"availableBorrowsBase":240.48047303,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8179228668001575,"updated":1738720626475,"account":"7PJrfbgoMXPveNVxB6SJxhErKCusQizEeQitpgC57Ugm9nPH","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x14d359c50599b72fe0080103261bd85a71d74cdf",{"totalCollateralBase":2140.74720357,"totalDebtBase":821.01457249,"availableBorrowsBase":463.43374965,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.825208824193254,"updated":1738720626922,"account":"7J4s2Y3kB2Niji8yb7hreJcqvZEPUpHgGhwBExhoUYo6uLVM","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x72aae1fa2cc19a1f367740bb49b93c96135d4e01",{"totalCollateralBase":9.83015081,"totalDebtBase":3.7407291,"availableBorrowsBase":2.15736139,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.8395091935419756,"updated":1738720628373,"account":"7LBuWMfmdg4d824RRxvJe3U9GsYyKfATWivKC9bjAaC2MoLn","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x582c50704848d821b1d54d9551429cc0033e24ac",{"totalCollateralBase":1854.03239386,"totalDebtBase":805.5265301,"availableBorrowsBase":584.9977653,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8413123089885925,"updated":1738720626430,"account":"7KbAfRT7KrU3dnoyrnSsNymHhFaXARXyTU5NxPmS9SG2ka52","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd089bd98d28ff9df3ac58d7a05f0d50e0eecf43b",{"totalCollateralBase":7398.69922435,"totalDebtBase":3188.03495832,"availableBorrowsBase":2360.98945994,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8566168366607612,"updated":1738720628260,"account":"7KATdGbMvZzfQUmyRNtc33SzoqwXS5W5K5L6GEDjNv3z7CiD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xa416e3003795ae8cf97b8b40e5900c33eb4a82ab",{"totalCollateralBase":7106.2795608,"totalDebtBase":3055.80969674,"availableBorrowsBase":2273.89997386,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.860398458289107,"updated":1738720628188,"account":"7MJhwJzhA2nrkZWAQkyz2aVF9dGtis5rT4s8kqbNPYoCcBMk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x9e5a1e58511c1c937cb741638e211f8b63864dba",{"totalCollateralBase":281.03110904,"totalDebtBase":135.61431126,"availableBorrowsBase":89.21057597,"currentLiquidationThreshold":90,"ltv":80,"healthFactor":1.8650538854640937,"updated":1738720628133,"account":"7MBBd2NDGKLV7jzgRrQis6o8ShwAQH4gWYzjNcizgaLdQcef","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfa6bec1c59118bb967eacc2d3f52b46779d20a63",{"totalCollateralBase":118.57750218,"totalDebtBase":50.4899978,"availableBorrowsBase":38.44312884,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.878827606920593,"updated":1738720626762,"account":"7PFuKHiTzHFsuwq6GPPTc5eUMG9BZgzL5LPRAhi1iCYwxgSt","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x72c7df8b90146d5ecbb62ac78c68a35fa47c85e2",{"totalCollateralBase":45664.55550404,"totalDebtBase":20918.21061256,"availableBorrowsBase":14736.67432499,"currentLiquidationThreshold":86.16,"ltv":78.08,"healthFactor":1.88087699043704,"updated":1738720628469,"account":"7LC47rMsqUGmbdT5xBNMAwEu77kGmKBdezwEmAiGmuqkcuyK","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x4e276ab655f6e4444c44e2953ba31ebb011e53e5",{"totalCollateralBase":4.74140619,"totalDebtBase":2.00815274,"availableBorrowsBase":1.5479019,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.888862771464286,"updated":1738720627723,"account":"7KN2jGb1Q65iKWPboApK3GPcUV1ycaHseiYtKv1QEsfmNhuh","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xfaeb3f91b59c6fa4460a5bd32219f20f40411184",{"totalCollateralBase":4017.41910399,"totalDebtBase":1701.03507605,"availableBorrowsBase":1312.02925194,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8893997710224348,"updated":1738720628192,"account":"7PGZ94jAoHbmvp4pwELWimu7gsWLTZSZut345WRKJesSKrsq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xb27b9088b217981687f9be39a8406ff5df1cdc03",{"totalCollateralBase":2.37151377,"totalDebtBase":1.00309634,"availableBorrowsBase":0.77553899,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.8913547426561241,"updated":1738720628249,"account":"7MdaWqFKKL7hYuADd7hpJyUJjYh3FUrPo6r5i25mDVwfLPpQ","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x51bc3dd10a9ee4946f961038cf5d50057e49eb77",{"totalCollateralBase":29839.42475783,"totalDebtBase":10925.39526641,"availableBorrowsBase":6978.25958829,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.911839052148408,"updated":1738720627015,"account":"7KATdGavWuztuT7tK9JsYU7iCboszXysXHcTH2ZLTfmFtnEY","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x269017cf8fcb7a80bce005c6f4681e74ebe09e32",{"totalCollateralBase":2480.62009227,"totalDebtBase":1034.52071472,"availableBorrowsBase":825.94435448,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9182758214340032,"updated":1738720627219,"account":"7JU7uhsB7p5BTRJYCuuHqCrXGu7bkX1yJLWwW3KsKpy8hV3p","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x2e927c0d25a6145dd923802b590c594bfae63c0c",{"totalCollateralBase":20594.29375888,"totalDebtBase":7513.36733876,"availableBorrowsBase":4843.20891657,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.9187143368926782,"updated":1738720627318,"account":"7Jed18UCMRySCjrgR2gKaSdJErrUXYXnnEmSS1T5eDpQK4Pk","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xbec33a3ae810bb2466593ce1b15edf83f45a17d1",{"totalCollateralBase":608.91089784,"totalDebtBase":252.72385232,"availableBorrowsBase":203.95932106,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9275138211061913,"updated":1738720628046,"account":"7MugNtAi3EkfkuBbcDrHGjzma64fm5FfeNreCafBJPjVCTTx","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x70e255c2d405b147d179abc79e0c2633dbd52417",{"totalCollateralBase":4823.42213405,"totalDebtBase":1987.89622497,"availableBorrowsBase":1629.67037557,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9411162709452972,"updated":1738720626884,"account":"7L9ZtDtjkmnwRF6ivpNRiVTutzUkRxoymP1YAJ3d1e6yDsaD","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x48d53e997686419653106b28dc5b1ec7bee128c1",{"totalCollateralBase":117.06705489,"totalDebtBase":50.3484228,"availableBorrowsBase":39.95710334,"currentLiquidationThreshold":84.29,"ltv":77.14,"healthFactor":1.959859218668514,"updated":1738720627922,"account":"7KF45azjsJLGegB5PKhb6rKy7frtHKveR5pszHPyUJo8nKkB","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xd2a4e645c9f881cad3146befd701329abd8dcc10",{"totalCollateralBase":858.69895086,"totalDebtBase":350.44031709,"availableBorrowsBase":293.58389606,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9602743382793348,"updated":1738720627154,"account":"7NMkKtcaRX6xAKxvZ9fT8zyB1TFYcT9cqHxxnBxY3Ww1Z2XE","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x589a2b25775ea0a4db41fb9975b288efbe4da7e8",{"totalCollateralBase":2461.63167437,"totalDebtBase":1003.77808109,"availableBorrowsBase":842.44567469,"currentLiquidationThreshold":80,"ltv":75,"healthFactor":1.9618931480965758,"updated":1738720627937,"account":"7KbjJAidH76pvZ5ZKRRdKCM7hjkaMPEPg57hxJZYPjfYNHCm","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0xcad3acc5fdc054877afde6ccebb1b176cd4acc92",{"totalCollateralBase":36710.7242902,"totalDebtBase":13025.48983354,"availableBorrowsBase":9000.94474058,"currentLiquidationThreshold":70,"ltv":60,"healthFactor":1.9728630041205955,"updated":1738720627354,"account":"7NBVqbNgut84x2KDwi7A1nqJJLkxMY9B6FLhfDnkoxSQALcq","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x80d8ec3368f9c2ad259b0051967ef2d85c245007",{"totalCollateralBase":7284.3317801,"totalDebtBase":2911.18750656,"availableBorrowsBase":2529.4799,"currentLiquidationThreshold":79.79,"ltv":74.69,"healthFactor":1.996493978571631,"updated":1738720627935,"account":"7LWVrVKqnock474KHKxEKg43FR7MTBJUrtTXadUq1eQUo851","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}]]}"#.to_vec()),
        sent: true,
        ..Default::default()
    });
}

// -----------------------------------------------------------------------
use ethereum::{TransactionAction, TransactionSignature};
use rlp::RlpStream;
use sp_core::crypto::AccountId32;
use sp_core::hashing::keccak_256;

pub const CHAIN_ID: u64 = 222_222;
fn create_unsigned_legacy_transaction() -> LegacyUnsignedTransaction {
	LegacyUnsignedTransaction {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
				"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
			.encode_as(),
	}
}

fn create_transaction(account: &AccountInfo) -> Transaction {
	LegacyUnsignedTransaction {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
				"18160ddd")
			.encode_as(),
	}.sign(&account.private_key)
}

pub fn create_legacy_transaction(account: &AccountInfo) -> Transaction {
	create_unsigned_legacy_transaction().sign(&account.private_key)
}

#[test]
fn print_tx() {
	let alice = alice_keys();
	let tx = create_legacy_transaction(&alice);
	println!("- - - - {:?}", tx);
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));


	let tx = create_transaction(&alice);
	println!("\n- - - - {:?}", tx);
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));
	let opt = recover_signer(&tx);
	println!("\n- - - - {:?}", opt);
}

fn recover_signer(transaction: &Transaction) -> Option<H160> {
	let mut sig = [0u8; 65];
	let mut msg = [0u8; 32];
	match transaction {
		Transaction::Legacy(t) => {
			sig[0..32].copy_from_slice(&t.signature.r()[..]);
			sig[32..64].copy_from_slice(&t.signature.s()[..]);
			sig[64] = t.signature.standard_v();
			msg.copy_from_slice(
				&ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..],
			);
		}
		Transaction::EIP2930(t) => {
			sig[0..32].copy_from_slice(&t.r[..]);
			sig[32..64].copy_from_slice(&t.s[..]);
			sig[64] = t.odd_y_parity as u8;
			msg.copy_from_slice(
				&ethereum::EIP2930TransactionMessage::from(t.clone()).hash()[..],
			);
		}
		Transaction::EIP1559(t) => {
			sig[0..32].copy_from_slice(&t.r[..]);
			sig[32..64].copy_from_slice(&t.s[..]);
			sig[64] = t.odd_y_parity as u8;
			msg.copy_from_slice(
				&ethereum::EIP1559TransactionMessage::from(t.clone()).hash()[..],
			);
		}
	}
	let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg).ok()?;
	Some(H160::from(H256::from(sp_io::hashing::keccak_256(&pubkey))))
}

pub struct AccountInfo {
	// pub address: H160,
	// pub account_id: AccountId32,
	pub private_key: H256,
}

fn alice_keys() -> AccountInfo {
	let private_key = H256::from_slice(hex!("e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a").as_slice());
	let secret_key = libsecp256k1::SecretKey::parse_slice(&private_key[..]).unwrap();
	let public_key = &libsecp256k1::PublicKey::from_secret_key(&secret_key).serialize()[1..65];
	println!("- - - - public key: {:?}", hex::encode(public_key));
	let address = H160::from(H256::from(keccak_256(public_key)));
	println!("- - - - address: {:?}", address);

	AccountInfo {
		private_key,
		// account_id: <Test as pallet_evm::Config>::AddressMapping::into_account_id(address),
		// address,
	}
}

pub struct LegacyUnsignedTransaction {
	pub nonce: U256,
	pub gas_price: U256,
	pub gas_limit: U256,
	pub action: TransactionAction,
	pub value: U256,
	pub input: Vec<u8>,
}

impl LegacyUnsignedTransaction {
	fn signing_rlp_append(&self, s: &mut RlpStream) {
		s.begin_list(9);
		s.append(&self.nonce);
		s.append(&self.gas_price);
		s.append(&self.gas_limit);
		s.append(&self.action);
		s.append(&self.value);
		s.append(&self.input);
		s.append(&CHAIN_ID);
		s.append(&0u8);
		s.append(&0u8);
	}

	fn signing_hash(&self) -> H256 {
		let mut stream = RlpStream::new();
		self.signing_rlp_append(&mut stream);
		H256::from(keccak_256(&stream.out()))
	}

	pub fn sign(&self, key: &H256) -> Transaction {
		self.sign_with_chain_id(key, CHAIN_ID)
	}

	pub fn sign_with_chain_id(&self, key: &H256, chain_id: u64) -> Transaction {
		let hash = self.signing_hash();
		let msg = libsecp256k1::Message::parse(hash.as_fixed_bytes());
		let s = libsecp256k1::sign(
			&msg,
			&libsecp256k1::SecretKey::parse_slice(&key[..]).unwrap(),
		);
		let sig = s.0.serialize();

		let sig = TransactionSignature::new(
			s.1.serialize() as u64 % 2 + chain_id * 2 + 35,
			H256::from_slice(&sig[0..32]),
			H256::from_slice(&sig[32..64]),
		)
			.unwrap();

		Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: self.nonce,
			gas_price: self.gas_price,
			gas_limit: self.gas_limit,
			action: self.action,
			value: self.value,
			input: self.input.clone(),
			signature: sig,
		})
	}
}

// -----------------------------------------------------------
use ethereum::TransactionV2 as EthereumTransaction;
use ethereum::EnvelopedEncodable;
use jsonrpsee::types::ErrorObjectOwned;
use fc_rpc_core::types::TransactionMessage;
use fc_rpc::{internal_err, EthSigner};

pub struct EthDevSigner {
	keys: Vec<libsecp256k1::SecretKey>,
}

impl EthDevSigner {
	pub fn new() -> Self {
		Self {
			keys: vec![libsecp256k1::SecretKey::parse(&[
				0xe5, 0xbe, 0x9a, 0x50, 0x92, 0xb8, 0x1b, 0xca, 0x64, 0xbe, 0x81, 0xd2, 0x12, 0xe7,
				0xf2, 0xf9, 0xeb, 0xa1, 0x83, 0xbb, 0x7a, 0x90, 0x95, 0x4f, 0x7b, 0x76, 0x36, 0x1f,
				0x6e, 0xdb, 0x5c, 0xa,
			])
				.expect("Test key is valid; qed"),
					   libsecp256k1::SecretKey::parse(&[
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11,
			])
				.expect("Test key is valid; qed")],
		}
	}
}

pub fn secret_key_address(secret: &libsecp256k1::SecretKey) -> H160 {
	let public = libsecp256k1::PublicKey::from_secret_key(secret);
	public_key_address(&public)
}

pub fn public_key_address(public: &libsecp256k1::PublicKey) -> H160 {
	let mut res = [0u8; 64];
	res.copy_from_slice(&public.serialize()[1..65]);
	H160::from(H256::from(keccak_256(&res)))
}

impl EthSigner for EthDevSigner {
	fn accounts(&self) -> Vec<H160> {
		self.keys.iter().map(secret_key_address).collect()
	}

	fn sign(
		&self,
		message: TransactionMessage,
		address: &H160,
	) -> Result<EthereumTransaction, ErrorObjectOwned> {
		let mut transaction = None;

		for secret in &self.keys {
			let key_address = secret_key_address(secret);

			if &key_address == address {
				match message {
					TransactionMessage::Legacy(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let v = match m.chain_id {
							None => 27 + recid.serialize() as u64,
							Some(chain_id) => 2 * chain_id + 35 + recid.serialize() as u64,
						};
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction =
							Some(EthereumTransaction::Legacy(ethereum::LegacyTransaction {
								nonce: m.nonce,
								gas_price: m.gas_price,
								gas_limit: m.gas_limit,
								action: m.action,
								value: m.value,
								input: m.input,
								signature: ethereum::TransactionSignature::new(v, r, s)
									.ok_or_else(|| {
										internal_err("signer generated invalid signature")
									})?,
							}));
					}
					TransactionMessage::EIP2930(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction =
							Some(EthereumTransaction::EIP2930(ethereum::EIP2930Transaction {
								chain_id: m.chain_id,
								nonce: m.nonce,
								gas_price: m.gas_price,
								gas_limit: m.gas_limit,
								action: m.action,
								value: m.value,
								input: m.input.clone(),
								access_list: m.access_list,
								odd_y_parity: recid.serialize() != 0,
								r,
								s,
							}));
					}
					TransactionMessage::EIP1559(m) => {
						let signing_message = libsecp256k1::Message::parse_slice(&m.hash()[..])
							.map_err(|_| internal_err("invalid signing message"))?;
						let (signature, recid) = libsecp256k1::sign(&signing_message, secret);
						let rs = signature.serialize();
						let r = H256::from_slice(&rs[0..32]);
						let s = H256::from_slice(&rs[32..64]);
						transaction =
							Some(EthereumTransaction::EIP1559(ethereum::EIP1559Transaction {
								chain_id: m.chain_id,
								nonce: m.nonce,
								max_priority_fee_per_gas: m.max_priority_fee_per_gas,
								max_fee_per_gas: m.max_fee_per_gas,
								gas_limit: m.gas_limit,
								action: m.action,
								value: m.value,
								input: m.input.clone(),
								access_list: m.access_list,
								odd_y_parity: recid.serialize() != 0,
								r,
								s,
							}));
					}
				}
				break;
			}
		}

		transaction.ok_or_else(|| internal_err("signer not available"))
	}
}

pub const NONCE: u32 = 0;
#[test]
fn eth_tx() {
	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE),
		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: ethereum::TransactionAction::Call(H160::from_slice(
			hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
				"8d241526\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000120\
			0000000000000000000000000000000000000000000000000000000000000002\
			0000000000000000000000000000000000000000000000000000000000000040\
			0000000000000000000000000000000000000000000000000000000000000080\
			0000000000000000000000000000000000000000000000000000000000000008\
			76444f542f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000008\
			414156452f555344000000000000000000000000000000000000000000000000\
			0000000000000000000000000000000000000000000000000000000000000002\
			00000000000000000000000029b5c33700000000000000000000000067acbce5\
			000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
			.encode_as(),
		chain_id: Some(CHAIN_ID)
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	println!("- - - - EVM address: {:?}", addr);
	let tx = signer.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr).unwrap();
	println!("\n- - - - {:?}", tx);
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));

}

#[test]
fn eth_tx_second() {
	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE),


		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
				"18160ddd")
			.encode_as(),
		chain_id: Some(CHAIN_ID)
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	let tx = signer.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr).unwrap();
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));

	let msg = ethereum::LegacyTransactionMessage {
		nonce: U256::from(NONCE + 1),


		gas_price: U256::from(51436290),
		gas_limit: U256::from(806740),
		action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
			hex!("0000000000000000000000000000000100000000").as_slice(),
		)),
		value: U256::zero(),
		input: hex!(
				"18160ddd")
			.encode_as(),
		chain_id: Some(CHAIN_ID)
	};

	let signer = EthDevSigner::new();
	let addr = secret_key_address(&signer.keys[0].clone());
	let tx = signer.sign(fc_rpc_core::types::TransactionMessage::Legacy(msg), &addr).unwrap();
	println!("\n- - - - {:?}", hex::encode(&tx.encode_payload()[..]));

}

// EVM account 		7KATdGb5uUXrET6mzKwHK9U3BhTZ9tQQMthCCqr4enLwWsVE

// preimage 		0x350284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01e0ad1fc73521aa1b76e0a7ada0ee4c0590ac45970b1e7687018474a2c0b4b12dca039be6d59a0f906b6a691efc11682bf83c0b0f67a4f91c17b41a16f7d6218455000000000f008c67025da919ee5f9f0c3ef934f421ad4a05258dd53e1fdd1d6f6d5630c663188835a000
// propose external	0x490284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01b636d232c6a01d4f704235ef4e4db2224c5710fb2cc9060a13fbdc7eb974046c667790053bcc23d934e40d7924cf3ac17a226ff716212b767ec407aacbb74682b501040000170204130502979c60952137c5cfb58ec643b70928c8f0cc34a268bee06384155b724fd9b85523000000a4
// fast track		0x550284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01baa6a39e4d2cde4280b62abecb673f9dfbfb3ffe28d9ecc02011fe5108ef473fad1a3c64396e298e9a643fec891abaf1f3106f06c31596bd0c38969b9750bb8415020800001902041307979c60952137c5cfb58ec643b70928c8f0cc34a268bee06384155b724fd9b8550500000002000000ac
// vote				0xf50184d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d015a363a63a65943e280f79db3ec69775334bf455518449fc3b42d85a66410dc4f11625c5b965f1f3051e605265fb6c18ab2fbac5dd3155d9223b827f4c7e8058855020c00001302310300800070f4986991e00d0000000000000000
// enact authorized

// buy Alice WETH 	0x450284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01b2dc493c49f474b7d9e21e02e3480ae16ff75152a07829a282add65ae5d41e1c855581223ca0b43248c4742eaae26123fd78fcd251fc688f92d55f7e632f358a25000000003b0614000000000000000000f4448291634500000000000000000000f444829163450000000000000000
// send WETH 		0x590284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d010a1f51f3240e1c412004ddf54394ffd288ec5dc95a10dfd01275f3214c01ae1165c17c5dce07d198d68e93eb4a1bacce84fcdf9b9e873bb83fbb9add5e287a88a5000400004d00455448008097c3c354652cb1eeed3e5b65fba2576470678a000000000000000014000000130000c84e676dc11b
// transact 		0xf86b80840310db02830c4f54940000000000000000000000000000000100000000808418160ddd8306c83fa09d9346fa1a83c414dbf77c7dda1e159ac7e6d6931483f2efdcdef9df75fe4015a035b02e2627a861cac8df102e97008b1611dd2d6f36296eb6bdec12f603af3bf6

// curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "eth_sendRawTransaction", "params": ["0xf86b80840310db02830c4f54940000000000000000000000000000000100000000808418160ddd8306c83fa09d9346fa1a83c414dbf77c7dda1e159ac7e6d6931483f2efdcdef9df75fe4015a035b02e2627a861cac8df102e97008b1611dd2d6f36296eb6bdec12f603af3bf6"]}' http://localhost:9988 && curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_submitExtrinsic", "params": ["0x590284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01ec4320708109c661d46654241a6e72fe8a06a32f8becfd6a25059d90d828781e2ab05f4c7f2acb3afcf7fa2f74c035a3325b414a74fde9f396f350a2bca7ff8535011c00004f008eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a480000000013000064a7b3b6e00d"]}' http://localhost:9988