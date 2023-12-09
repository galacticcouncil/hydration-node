use crate::transcendental::pow;
use fixed::types::U32F96;
use std::str::FromStr;

fn ensure_accuracy(result: U32F96, expected: U32F96, tolerance: U32F96) -> bool {
	let diff = if result > expected {
		result - expected
	} else {
		expected - result
	};

	// TODO: verify with colin : otherwise diff / expected is > tolerance in some cases
	if diff < tolerance {
		return true;
	}

	let r = diff / expected;
	r <= tolerance
}

#[test]
fn pow_should_be_accurate() {
	type S = U32F96;
	type D = U32F96;
	let tolerance = S::from_num(10e-10);

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(0.01)).unwrap();
	let expected = S::from_str("0.9930924954370359015332102168880745712214323654004972804695688652").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(0.1)).unwrap();
	let expected = S::from_str("0.9330329915368074159813432661499421670272299643514940389004973854").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(0.5)).unwrap();
	let expected = S::from_str("0.7071067811865475244008443621048490392848359376884740365883398689").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(0.75)).unwrap();
	let expected = S::from_str("0.5946035575013605333587499852802379576464860462319087065095011123").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(1.0)).unwrap();
	let expected = S::from_str("0.5").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(1.5)).unwrap();
	let expected = S::from_str("0.3535533905932737622004221810524245196424179688442370182941699344").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(2.0)).unwrap();
	let expected = S::from_str("0.25").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(2.5)).unwrap();
	let expected = S::from_str("0.1767766952966368811002110905262122598212089844221185091470849672").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(3.0)).unwrap();
	let expected = S::from_str("0.125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(3.5)).unwrap();
	let expected = S::from_str("0.0883883476483184405501055452631061299106044922110592545735424836").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(4.0)).unwrap();
	let expected = S::from_str("0.0625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(4.5)).unwrap();
	let expected = S::from_str("0.0441941738241592202750527726315530649553022461055296272867712418").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(5.0)).unwrap();
	let expected = S::from_str("0.03125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(5.5)).unwrap();
	let expected = S::from_str("0.0220970869120796101375263863157765324776511230527648136433856209").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(6.0)).unwrap();
	let expected = S::from_str("0.015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(6.5)).unwrap();
	let expected = S::from_str("0.0110485434560398050687631931578882662388255615263824068216928104").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(7.0)).unwrap();
	let expected = S::from_str("0.0078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(7.5)).unwrap();
	let expected = S::from_str("0.0055242717280199025343815965789441331194127807631912034108464052").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(8.0)).unwrap();
	let expected = S::from_str("0.00390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(8.5)).unwrap();
	let expected = S::from_str("0.0027621358640099512671907982894720665597063903815956017054232026").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(9.0)).unwrap();
	let expected = S::from_str("0.001953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(9.5)).unwrap();
	let expected = S::from_str("0.0013810679320049756335953991447360332798531951907978008527116013").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(10.0)).unwrap();
	let expected = S::from_str("0.0009765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(10.5)).unwrap();
	let expected = S::from_str("0.0006905339660024878167976995723680166399265975953989004263558006").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(11.0)).unwrap();
	let expected = S::from_str("0.00048828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(11.5)).unwrap();
	let expected = S::from_str("0.0003452669830012439083988497861840083199632987976994502131779003").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(12.0)).unwrap();
	let expected = S::from_str("0.000244140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(12.5)).unwrap();
	let expected = S::from_str("0.0001726334915006219541994248930920041599816493988497251065889501").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(13.0)).unwrap();
	let expected = S::from_str("0.0001220703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(13.5)).unwrap();
	let expected = S::from_str("0.0000863167457503109770997124465460020799908246994248625532944750").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(14.0)).unwrap();
	let expected = S::from_str("0.00006103515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(14.5)).unwrap();
	let expected = S::from_str("0.0000431583728751554885498562232730010399954123497124312766472375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(15.0)).unwrap();
	let expected = S::from_str("0.000030517578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(15.5)).unwrap();
	let expected = S::from_str("0.0000215791864375777442749281116365005199977061748562156383236187").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(16.0)).unwrap();
	let expected = S::from_str("0.0000152587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(16.5)).unwrap();
	let expected = S::from_str("0.0000107895932187888721374640558182502599988530874281078191618093").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(17.0)).unwrap();
	let expected = S::from_str("0.00000762939453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(17.5)).unwrap();
	let expected = S::from_str("0.0000053947966093944360687320279091251299994265437140539095809046").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(18.0)).unwrap();
	let expected = S::from_str("0.000003814697265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(18.5)).unwrap();
	let expected = S::from_str("0.0000026973983046972180343660139545625649997132718570269547904523").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(19.0)).unwrap();
	let expected = S::from_str("0.0000019073486328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(19.5)).unwrap();
	let expected = S::from_str("0.0000013486991523486090171830069772812824998566359285134773952261").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(20.0)).unwrap();
	let expected = S::from_str("0.00000095367431640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(20.5)).unwrap();
	let expected = S::from_str("0.00000067434957617430450859150348864064124992831796425673869761308").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(21.0)).unwrap();
	let expected = S::from_str("0.000000476837158203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(21.5)).unwrap();
	let expected = S::from_str("0.00000033717478808715225429575174432032062496415898212836934880654").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(22.0)).unwrap();
	let expected = S::from_str("0.0000002384185791015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(22.5)).unwrap();
	let expected = S::from_str("0.00000016858739404357612714787587216016031248207949106418467440327").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(23.0)).unwrap();
	let expected = S::from_str("0.00000011920928955078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(23.5)).unwrap();
	let expected = S::from_str("0.000000084293697021788063573937936080080156241039745532092337201635").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(24.0)).unwrap();
	let expected = S::from_str("0.000000059604644775390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(24.5)).unwrap();
	let expected = S::from_str("0.000000042146848510894031786968968040040078120519872766046168600817").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(25.0)).unwrap();
	let expected = S::from_str("0.0000000298023223876953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(25.5)).unwrap();
	let expected = S::from_str("0.000000021073424255447015893484484020020039060259936383023084300408").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(26.0)).unwrap();
	let expected = S::from_str("0.00000001490116119384765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(26.5)).unwrap();
	let expected = S::from_str("0.000000010536712127723507946742242010010019530129968191511542150204").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(27.0)).unwrap();
	let expected = S::from_str("0.000000007450580596923828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(27.5)).unwrap();
	let expected = S::from_str("0.0000000052683560638617539733711210050050097650649840957557710751022").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(28.0)).unwrap();
	let expected = S::from_str("0.0000000037252902984619140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(28.5)).unwrap();
	let expected = S::from_str("0.0000000026341780319308769866855605025025048825324920478778855375511").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(29.0)).unwrap();
	let expected = S::from_str("0.00000000186264514923095703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(29.5)).unwrap();
	let expected = S::from_str("0.0000000013170890159654384933427802512512524412662460239389427687755").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(30.0)).unwrap();
	let expected = S::from_str("0.000000000931322574615478515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(30.5)).unwrap();
	let expected = S::from_str("0.00000000065854450798271924667139012562562622063312301196947138438777").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(31.0)).unwrap();
	let expected = S::from_str("0.0000000004656612873077392578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	/*
		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(31.5)).unwrap();
		let expected = S::from_str("0.00000000032927225399135962333569506281281311031656150598473569219388").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(32.0)).unwrap();
		let expected = S::from_str("0.00000000023283064365386962890625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(32.5)).unwrap();
		let expected = S::from_str("0.00000000016463612699567981166784753140640655515828075299236784609694").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(33.0)).unwrap();
		let expected = S::from_str("0.000000000116415321826934814453125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(33.5)).unwrap();
		let expected = S::from_str("0.000000000082318063497839905833923765703203277579140376496183923048472").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(34.0)).unwrap();
		let expected = S::from_str("0.0000000000582076609134674072265625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(34.5)).unwrap();
		let expected = S::from_str("0.000000000041159031748919952916961882851601638789570188248091961524236").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(35.0)).unwrap();
		let expected = S::from_str("0.00000000002910383045673370361328125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(35.5)).unwrap();
		let expected = S::from_str("0.000000000020579515874459976458480941425800819394785094124045980762118").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(36.0)).unwrap();
		let expected = S::from_str("0.000000000014551915228366851806640625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(36.5)).unwrap();
		let expected = S::from_str("0.000000000010289757937229988229240470712900409697392547062022990381059").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(37.0)).unwrap();
		let expected = S::from_str("0.0000000000072759576141834259033203125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(37.5)).unwrap();
		let expected = S::from_str("0.0000000000051448789686149941146202353564502048486962735310114951905295").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(38.0)).unwrap();
		let expected = S::from_str("0.00000000000363797880709171295166015625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(38.5)).unwrap();
		let expected = S::from_str("0.0000000000025724394843074970573101176782251024243481367655057475952647").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(39.0)).unwrap();
		let expected = S::from_str("0.000000000001818989403545856475830078125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(39.5)).unwrap();
		let expected = S::from_str("0.0000000000012862197421537485286550588391125512121740683827528737976323").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(40.0)).unwrap();
		let expected = S::from_str("0.0000000000009094947017729282379150390625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(40.5)).unwrap();
		let expected = S::from_str("0.00000000000064310987107687426432752941955627560608703419137643689881619").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(41.0)).unwrap();
		let expected = S::from_str("0.00000000000045474735088646411895751953125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(41.5)).unwrap();
		let expected = S::from_str("0.00000000000032155493553843713216376470977813780304351709568821844940809").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(42.0)).unwrap();
		let expected = S::from_str("0.000000000000227373675443232059478759765625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(42.5)).unwrap();
		let expected = S::from_str("0.00000000000016077746776921856608188235488906890152175854784410922470404").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(43.0)).unwrap();
		let expected = S::from_str("0.0000000000001136868377216160297393798828125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(43.5)).unwrap();
		let expected = S::from_str("0.000000000000080388733884609283040941177444534450760879273922054612352023").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(44.0)).unwrap();
		let expected = S::from_str("0.00000000000005684341886080801486968994140625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(44.5)).unwrap();
		let expected = S::from_str("0.000000000000040194366942304641520470588722267225380439636961027306176011").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(45.0)).unwrap();
		let expected = S::from_str("0.000000000000028421709430404007434844970703125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(45.5)).unwrap();
		let expected = S::from_str("0.000000000000020097183471152320760235294361133612690219818480513653088005").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(46.0)).unwrap();
		let expected = S::from_str("0.0000000000000142108547152020037174224853515625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(46.5)).unwrap();
		let expected = S::from_str("0.000000000000010048591735576160380117647180566806345109909240256826544002").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(47.0)).unwrap();
		let expected = S::from_str("0.00000000000000710542735760100185871124267578125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(47.5)).unwrap();
		let expected = S::from_str("0.0000000000000050242958677880801900588235902834031725549546201284132720014").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(48.0)).unwrap();
		let expected = S::from_str("0.000000000000003552713678800500929355621337890625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(48.5)).unwrap();
		let expected = S::from_str("0.0000000000000025121479338940400950294117951417015862774773100642066360007").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(49.0)).unwrap();
		let expected = S::from_str("0.0000000000000017763568394002504646778106689453125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(49.5)).unwrap();
		let expected = S::from_str("0.0000000000000012560739669470200475147058975708507931387386550321033180003").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(50.0)).unwrap();
		let expected = S::from_str("0.00000000000000088817841970012523233890533447265625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(50.5)).unwrap();
		let expected = S::from_str("0.00000000000000062803698347351002375735294878542539656936932751605165900018").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(51.0)).unwrap();
		let expected = S::from_str("0.000000000000000444089209850062616169452667236328125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(51.5)).unwrap();
		let expected = S::from_str("0.00000000000000031401849173675501187867647439271269828468466375802582950009").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(52.0)).unwrap();
		let expected = S::from_str("0.0000000000000002220446049250313080847263336181640625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(52.5)).unwrap();
		let expected = S::from_str("0.00000000000000015700924586837750593933823719635634914234233187901291475004").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(53.0)).unwrap();
		let expected = S::from_str("0.00000000000000011102230246251565404236316680908203125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(53.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000078504622934188752969669118598178174571171165939506457375023").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(54.0)).unwrap();
		let expected = S::from_str("0.000000000000000055511151231257827021181583404541015625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(54.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000039252311467094376484834559299089087285585582969753228687511").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(55.0)).unwrap();
		let expected = S::from_str("0.0000000000000000277555756156289135105907917022705078125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(55.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000019626155733547188242417279649544543642792791484876614343755").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(56.0)).unwrap();
		let expected = S::from_str("0.00000000000000001387778780781445675529539585113525390625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(56.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000098130778667735941212086398247722718213963957424383071718779").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(57.0)).unwrap();
		let expected = S::from_str("0.000000000000000006938893903907228377647697925567626953125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(57.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000049065389333867970606043199123861359106981978712191535859389").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(58.0)).unwrap();
		let expected = S::from_str("0.0000000000000000034694469519536141888238489627838134765625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(58.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000024532694666933985303021599561930679553490989356095767929694").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(59.0)).unwrap();
		let expected = S::from_str("0.00000000000000000173472347597680709441192448139190673828125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(59.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000012266347333466992651510799780965339776745494678047883964847").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(60.0)).unwrap();
		let expected = S::from_str("0.000000000000000000867361737988403547205962240695953369140625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(60.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000061331736667334963257553998904826698883727473390239419824236").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(61.0)).unwrap();
		let expected = S::from_str("0.0000000000000000004336808689942017736029811203479766845703125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(61.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000030665868333667481628776999452413349441863736695119709912118").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(62.0)).unwrap();
		let expected = S::from_str("0.00000000000000000021684043449710088680149056017398834228515625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(62.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000015332934166833740814388499726206674720931868347559854956059").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(63.0)).unwrap();
		let expected = S::from_str("0.000000000000000000108420217248550443400745280086994171142578125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(63.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000076664670834168704071942498631033373604659341737799274780296").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(64.0)).unwrap();
		let expected = S::from_str("0.0000000000000000000542101086242752217003726400434970855712890625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(64.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000038332335417084352035971249315516686802329670868899637390148").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(65.0)).unwrap();
		let expected = S::from_str("0.00000000000000000002710505431213761085018632002174854278564453125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(65.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000019166167708542176017985624657758343401164835434449818695074").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(66.0)).unwrap();
		let expected = S::from_str("0.000000000000000000013552527156068805425093160010874271392822265625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(66.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000095830838542710880089928123288791717005824177172249093475370").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(67.0)).unwrap();
		let expected = S::from_str("0.0000000000000000000067762635780344027125465800054371356964111328125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(67.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000047915419271355440044964061644395858502912088586124546737685").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(68.0)).unwrap();
		let expected = S::from_str("0.00000000000000000000338813178901720135627329000271856784820556640625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(68.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000023957709635677720022482030822197929251456044293062273368842").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(69.0)).unwrap();
		let expected = S::from_str("0.000000000000000000001694065894508600678136645001359283924102783203125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(69.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000011978854817838860011241015411098964625728022146531136684421").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(70.0)).unwrap();
		let expected = S::from_str("0.0000000000000000000008470329472543003390683225006796419620513916015625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(70.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000059894274089194300056205077055494823128640110732655683422106").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(71.0)).unwrap();
		let expected = S::from_str("0.00000000000000000000042351647362715016953416125033982098102569580078125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(71.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000029947137044597150028102538527747411564320055366327841711053").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(72.0)).unwrap();
		let expected = S::from_str("0.000000000000000000000211758236813575084767080625169910490512847900390625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(72.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000014973568522298575014051269263873705782160027683163920855526").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(73.0)).unwrap();
		let expected = S::from_str("0.0000000000000000000001058791184067875423835403125849552452564239501953125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(73.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000074867842611492875070256346319368528910800138415819604277633").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(74.0)).unwrap();
		let expected = S::from_str("0.00000000000000000000005293955920339377119177015629247762262821197509765625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(74.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000037433921305746437535128173159684264455400069207909802138816").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(75.0)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000026469779601696885595885078146238811314105987548828125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(75.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000018716960652873218767564086579842132227700034603954901069408").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(76.0)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000132348898008484427979425390731194056570529937744140625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(76.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000093584803264366093837820432899210661138500173019774505347041").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(77.0)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000661744490042422139897126953655970282852649688720703125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(77.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000046792401632183046918910216449605330569250086509887252673520").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(78.0)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000003308722450212110699485634768279851414263248443603515625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(78.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000023396200816091523459455108224802665284625043254943626336760").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(79.0)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000016543612251060553497428173841399257071316242218017578125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(79.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000011698100408045761729727554112401332642312521627471813168380").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(80.0)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000082718061255302767487140869206996285356581211090087890625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(80.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000058490502040228808648637770562006663211562608137359065841900").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(81.0)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000413590306276513837435704346034981426782906055450439453125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(81.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000029245251020114404324318885281003331605781304068679532920950").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(82.0)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000002067951531382569187178521730174907133914530277252197265625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(82.5)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000014622625510057202162159442640501665802890652034339766460475").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(83.0)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000010339757656912845935892608650874535669572651386260986328125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(83.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000073113127550286010810797213202508329014453260171698832302376").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(84.0)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000051698788284564229679463043254372678347863256931304931640625").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(84.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000036556563775143005405398606601254164507226630085849416151188").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(85.0)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000000258493941422821148397315216271863391739316284656524658203125").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(85.5)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000018278281887571502702699303300627082253613315042924708075594").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(86.0)).unwrap();
		let expected =
			S::from_str("0.00000000000000000000000001292469707114105741986576081359316958696581423282623291015625")
				.unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(86.5)).unwrap();
		let expected =
			S::from_str("0.0000000000000000000000000091391409437857513513496516503135411268066575214623540377970").unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

		let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(87.0)).unwrap();
		let expected =
			S::from_str("0.000000000000000000000000006462348535570528709932880406796584793482907116413116455078125")
				.unwrap();
		assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(87.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000045695704718928756756748258251567705634033287607311770188985").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(88.0)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000032311742677852643549664402033982923967414535582065582275390625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(88.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000022847852359464378378374129125783852817016643803655885094492").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(89.0)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000161558713389263217748322010169914619837072677910327911376953125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(89.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000011423926179732189189187064562891926408508321901827942547246").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(90.0)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000807793566946316088741610050849573099185363389551639556884765625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(90.5)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000057119630898660945945935322814459632042541609509139712736231").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(91.0)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000004038967834731580443708050254247865495926816947758197784423828125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(91.5)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000028559815449330472972967661407229816021270804754569856368115").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(92.0)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000020194839173657902218540251271239327479634084738790988922119140625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(92.5)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000014279907724665236486483830703614908010635402377284928184057").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(93.0)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000100974195868289511092701256356196637398170423693954944610595703125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(93.5)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000071399538623326182432419153518074540053177011886424640920289").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(94.0)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000504870979341447555463506281780983186990852118469774723052978515625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(94.5)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000035699769311663091216209576759037270026588505943212320460144").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(95.0)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000002524354896707237777317531408904915934954260592348873615264892578125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(95.5)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000017849884655831545608104788379518635013294252971606160230072").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(96.0)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000012621774483536188886587657044524579674771302961744368076324462890625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(96.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000089249423279157728040523941897593175066471264858030801150361").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(97.0)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000063108872417680944432938285222622898373856514808721840381622314453125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(97.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000044624711639578864020261970948796587533235632429015400575180").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(98.0)).unwrap();
	   let expected = S::from_str("0.00000000000000000000000000000315544362088404722164691426113114491869282574043609201908111572265625").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(98.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000022312355819789432010130985474398293766617816214507700287590").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(99.0)).unwrap();
	   let expected = S::from_str("0.000000000000000000000000000001577721810442023610823457130565572459346412870218046009540557861328125").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	   let result: D = pow::<S, D>(S::from_num(0.5), S::from_num(99.5)).unwrap();
	   let expected = S::from_str("0.0000000000000000000000000000011156177909894716005065492737199146883308908107253850143795").unwrap();
	   assert!(ensure_accuracy(result, expected, tolerance));

	*/

	let result: D = pow::<S, D>(S::from_num(0.67), S::from_num(53.0)).unwrap();
	let expected = S::from_str(
		"0.0000000006052914552722019591898711030314941034235684285865095543437428583423360086532851192983977435055987",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	/*
	let result: D = pow::<S, D>(S::from_num(0.67), S::from_num(54.0)).unwrap();
	let expected = S::from_str("0.0000000004055452750323753126").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	*/

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(0.01)).unwrap();
	let expected = S::from_str("0.9971273133589335063736433138321697561416509622715937703756356260").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(0.1)).unwrap();
	let expected = S::from_str("0.9716416578630735005730455612486887640126529295738304341186096827").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(0.5)).unwrap();
	let expected = S::from_str("0.8660254037844386467637231707529361834714026269051903140279034897").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(0.75)).unwrap();
	let expected = S::from_str("0.8059274488676564396650036175294479328528122153879514906666908881").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(1.0)).unwrap();
	let expected = S::from_str("0.75").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(1.5)).unwrap();
	let expected = S::from_str("0.6495190528383289850727923780647021376035519701788927355209276172").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(2.0)).unwrap();
	let expected = S::from_str("0.5625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(2.5)).unwrap();
	let expected = S::from_str("0.4871392896287467388045942835485266032026639776341695516406957129").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(3.0)).unwrap();
	let expected = S::from_str("0.421875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(3.5)).unwrap();
	let expected = S::from_str("0.3653544672215600541034457126613949524019979832256271637305217847").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(4.0)).unwrap();
	let expected = S::from_str("0.31640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(4.5)).unwrap();
	let expected = S::from_str("0.2740158504161700405775842844960462143014984874192203727978913385").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(5.0)).unwrap();
	let expected = S::from_str("0.2373046875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(5.5)).unwrap();
	let expected = S::from_str("0.2055118878121275304331882133720346607261238655644152795984185039").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(6.0)).unwrap();
	let expected = S::from_str("0.177978515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(6.5)).unwrap();
	let expected = S::from_str("0.1541339158590956478248911600290259955445928991733114596988138779").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(7.0)).unwrap();
	let expected = S::from_str("0.13348388671875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(7.5)).unwrap();
	let expected = S::from_str("0.1156004368943217358686683700217694966584446743799835947741104084").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(8.0)).unwrap();
	let expected = S::from_str("0.1001129150390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(8.5)).unwrap();
	let expected = S::from_str("0.0867003276707413019015012775163271224938335057849876960805828063").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(9.0)).unwrap();
	let expected = S::from_str("0.075084686279296875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(9.5)).unwrap();
	let expected = S::from_str("0.0650252457530559764261259581372453418703751293387407720604371047").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(10.0)).unwrap();
	let expected = S::from_str("0.05631351470947265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(10.5)).unwrap();
	let expected = S::from_str("0.0487689343147919823195944686029340064027813470040555790453278285").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(11.0)).unwrap();
	let expected = S::from_str("0.0422351360321044921875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(11.5)).unwrap();
	let expected = S::from_str("0.0365767007360939867396958514522005048020860102530416842839958714").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(12.0)).unwrap();
	let expected = S::from_str("0.031676352024078369140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(12.5)).unwrap();
	let expected = S::from_str("0.0274325255520704900547718885891503786015645076897812632129969035").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(13.0)).unwrap();
	let expected = S::from_str("0.02375726401805877685546875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(13.5)).unwrap();
	let expected = S::from_str("0.0205743941640528675410789164418627839511733807673359474097476776").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(14.0)).unwrap();
	let expected = S::from_str("0.0178179480135440826416015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(14.5)).unwrap();
	let expected = S::from_str("0.0154307956230396506558091873313970879633800355755019605573107582").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(15.0)).unwrap();
	let expected = S::from_str("0.013363461010158061981201171875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(15.5)).unwrap();
	let expected = S::from_str("0.0115730967172797379918568904985478159725350266816264704179830686").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(16.0)).unwrap();
	let expected = S::from_str("0.01002259575761854648590087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(16.5)).unwrap();
	let expected = S::from_str("0.0086798225379598034938926678739108619794012700112198528134873015").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(17.0)).unwrap();
	let expected = S::from_str("0.0075169468182139098644256591796875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(17.5)).unwrap();
	let expected = S::from_str("0.0065098669034698526204195009054331464845509525084148896101154761").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(18.0)).unwrap();
	let expected = S::from_str("0.005637710113660432398319244384765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(18.5)).unwrap();
	let expected = S::from_str("0.0048824001776023894653146256790748598634132143813111672075866071").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(19.0)).unwrap();
	let expected = S::from_str("0.00422828258524532429873943328857421875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(19.5)).unwrap();
	let expected = S::from_str("0.0036618001332017920989859692593061448975599107859833754056899553").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(20.0)).unwrap();
	let expected = S::from_str("0.0031712119389339932240545749664306640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(20.5)).unwrap();
	let expected = S::from_str("0.0027463500999013440742394769444796086731699330894875315542674664").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(21.0)).unwrap();
	let expected = S::from_str("0.002378408954200494918040931224822998046875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(21.5)).unwrap();
	let expected = S::from_str("0.0020597625749260080556796077083597065048774498171156486657005998").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(22.0)).unwrap();
	let expected = S::from_str("0.00178380671565037118853069841861724853515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(22.5)).unwrap();
	let expected = S::from_str("0.0015448219311945060417597057812697798786580873628367364992754499").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(23.0)).unwrap();
	let expected = S::from_str("0.0013378550367377783913980238139629364013671875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(23.5)).unwrap();
	let expected = S::from_str("0.0011586164483958795313197793359523349089935655221275523744565874").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(24.0)).unwrap();
	let expected = S::from_str("0.001003391277553333793548517860472202301025390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(24.5)).unwrap();
	let expected = S::from_str("0.0008689623362969096484898345019642511817451741415956642808424405").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(25.0)).unwrap();
	let expected = S::from_str("0.00075254345816500034516138839535415172576904296875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(25.5)).unwrap();
	let expected = S::from_str("0.0006517217522226822363673758764731883863088806061967482106318304").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(26.0)).unwrap();
	let expected = S::from_str("0.0005644075936237502588710412965156137943267822265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(26.5)).unwrap();
	let expected = S::from_str("0.0004887913141670116772755319073548912897316604546475611579738728").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(27.0)).unwrap();
	let expected = S::from_str("0.000423305695217812694153280972386710345745086669921875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(27.5)).unwrap();
	let expected = S::from_str("0.0003665934856252587579566489305161684672987453409856708684804046").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(28.0)).unwrap();
	let expected = S::from_str("0.00031747927141335952061496072929003275930881500244140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(28.5)).unwrap();
	let expected = S::from_str("0.0002749451142189440684674866978871263504740590057392531513603034").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(29.0)).unwrap();
	let expected = S::from_str("0.0002381094535600196404612205469675245694816112518310546875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(29.5)).unwrap();
	let expected = S::from_str("0.0002062088356642080513506150234153447628555442543044398635202275").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(30.0)).unwrap();
	let expected = S::from_str("0.000178582090170014730345915410225643427111208438873291015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(30.5)).unwrap();
	let expected = S::from_str("0.0001546566267481560385129612675615085721416581907283298976401706").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(31.0)).unwrap();
	let expected = S::from_str("0.00013393656762751104775943655766923257033340632915496826171875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(31.5)).unwrap();
	let expected = S::from_str("0.0001159924700611170288847209506711314291062436430462474232301280").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(32.0)).unwrap();
	let expected = S::from_str("0.0001004524257206332858195774182519244277500547468662261962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(32.5)).unwrap();
	let expected = S::from_str("0.0000869943525458377716635407130033485718296827322846855674225960").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(33.0)).unwrap();
	let expected = S::from_str("0.000075339319290474964364683063688943320812541060149669647216796875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(33.5)).unwrap();
	let expected = S::from_str("0.0000652457644093783287476555347525114288722620492135141755669470").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(34.0)).unwrap();
	let expected = S::from_str("0.00005650448946785622327351229776670749060940579511225223541259765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(34.5)).unwrap();
	let expected = S::from_str("0.0000489343233070337465607416510643835716541965369101356316752102").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(35.0)).unwrap();
	let expected = S::from_str("0.0000423783671008921674551342233250306179570543463341891765594482421875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(35.5)).unwrap();
	let expected = S::from_str("0.0000367007424802753099205562382982876787406474026826017237564076").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(36.0)).unwrap();
	let expected = S::from_str("0.000031783775325669125591350667493772963467790759750641882419586181640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(36.5)).unwrap();
	let expected = S::from_str("0.0000275255568602064824404171787237157590554855520119512928173057").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(37.0)).unwrap();
	let expected = S::from_str("0.00002383783149425184419351300062032972260084306981298141181468963623046875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(37.5)).unwrap();
	let expected = S::from_str("0.0000206441676451548618303128840427868192916141640089634696129793").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(38.0)).unwrap();
	let expected =
		S::from_str("0.0000178783736206888831451347504652472919506323023597360588610172271728515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(38.5)).unwrap();
	let expected = S::from_str("0.0000154831257338661463727346630320901144687106230067226022097344").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(39.0)).unwrap();
	let expected =
		S::from_str("0.000013408780215516662358851062848935468962974226769802044145762920379638671875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(39.5)).unwrap();
	let expected = S::from_str("0.0000116123443003996097795509972740675858515329672550419516573008").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(40.0)).unwrap();
	let expected =
		S::from_str("0.00001005658516163749676913829713670160172223067007735153310932219028472900390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(40.5)).unwrap();
	let expected = S::from_str("0.0000087092582252997073346632479555506893886497254412814637429756").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(41.0)).unwrap();
	let expected =
		S::from_str("0.0000075424388712281225768537228525262012916730025580136498319916427135467529296875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(41.5)).unwrap();
	let expected = S::from_str("0.0000065319436689747805009974359666630170414872940809610978072317").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(42.0)).unwrap();
	let expected =
		S::from_str("0.000005656829153421091932640292139394650968754751918510237373993732035160064697265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(42.5)).unwrap();
	let expected = S::from_str("0.0000048989577517310853757480769749972627811154705607208233554238").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(43.0)).unwrap();
	let expected =
		S::from_str("0.00000424262186506581894948021910454598822656606393888267803049529902637004852294921875")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(43.5)).unwrap();
	let expected = S::from_str("0.0000036742183137983140318110577312479470858366029205406175165678").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(44.0)).unwrap();
	let expected =
		S::from_str("0.0000031819663987993642121101643284094911699245479541620085228714742697775363922119140625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(44.5)).unwrap();
	let expected = S::from_str("0.0000027556637353487355238582932984359603143774521904054631374258").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(45.0)).unwrap();
	let expected =
		S::from_str("0.000002386474799099523159082623246307118377443410965621506392153605702333152294158935546875")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(45.5)).unwrap();
	let expected = S::from_str("0.0000020667478015115516428937199738269702357830891428040973530694").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(46.0)).unwrap();
	let expected =
		S::from_str("0.00000178985609932464236931196743473033878308255822421612979411520427674986422061920166015625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(46.5)).unwrap();
	let expected = S::from_str("0.0000015500608511336637321702899803702276768373168571030730148020").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(47.0)).unwrap();
	let expected =
		S::from_str("0.0000013423920744934817769839755760477540873119186681620973455864032075623981654644012451171875")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(47.5)).unwrap();
	let expected = S::from_str("0.0000011625456383502477991277174852776707576279876428273047611015").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(48.0)).unwrap();
	let expected = S::from_str(
		"0.000001006794055870111332737981682035815565483939001121573009189802405671798624098300933837890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(48.5)).unwrap();
	let expected = S::from_str("0.00000087190922876268584934578811395825306822099073212047857082616").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(49.0)).unwrap();
	let expected = S::from_str(
		"0.00000075509554190258349955348626152686167411295425084117975689235180425384896807372570037841796875",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(49.5)).unwrap();
	let expected = S::from_str("0.00000065393192157201438700934108546868980116574304909035892811962").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(50.0)).unwrap();
	let expected = S::from_str(
		"0.0000005663216564269376246651146961451462555847156881308848176692638531903867260552942752838134765625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(50.5)).unwrap();
	let expected = S::from_str("0.00000049044894117901079025700581410151735087430728681776919608971").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(51.0)).unwrap();
	let expected = S::from_str(
		"0.000000424741242320203218498836022108859691688536766098163613251947889892790044541470706462860107421875",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(51.5)).unwrap();
	let expected = S::from_str("0.00000036783670588425809269275436057613801315573046511332689706728").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(52.0)).unwrap();
	let expected = S::from_str(
		"0.00000031855593174015241387412701658164476876640257457362270993896091741959253340610302984714508056640625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(52.5)).unwrap();
	let expected = S::from_str("0.00000027587752941319356951956577043210350986679784883499517280046").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(53.0)).unwrap();
	let expected = S::from_str(
		"0.0000002389169488051143104055952624362335765748019309302170324542206880646944000545772723853588104248046875",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(53.5)).unwrap();
	let expected = S::from_str("0.00000020690814705989517713967432782407763240009838662624637960034").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(54.0)).unwrap();
	let expected = S::from_str("0.000000179187711603835732804196446827175182431101448197662774340665516048520800040932954289019107818603515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(54.5)).unwrap();
	let expected = S::from_str("0.00000015518111029492138285475574586805822430007378996968478470026").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(55.0)).unwrap();
	let expected = S::from_str("0.00000013439078370287679960314733512038138682332608614824708075549913703639060003069971571676433086395263671875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(55.5)).unwrap();
	let expected = S::from_str("0.00000011638583272119103714106680940104366822505534247726358852519").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(56.0)).unwrap();
	let expected = S::from_str("0.0000001007930877771575997023605013402860401174945646111853105666243527772929500230247867875732481479644775390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(56.5)).unwrap();
	let expected = S::from_str("0.000000087289374540893277855800107050782751168791506857947691393897").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(57.0)).unwrap();
	let expected = S::from_str("0.000000075594815832868199776770376005214530088120923458388982924968264582969712517268590090679936110973358154296875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(57.5)).unwrap();
	let expected = S::from_str("0.000000065467030905669958391850080288087063376593630143460768545422").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(58.0)).unwrap();
	let expected = S::from_str("0.00000005669611187465114983257778200391089756609069259379173719372619843722728438795144256800995208323001861572265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(58.5)).unwrap();
	let expected = S::from_str("0.000000049100273179252468793887560216065297532445222607595576409067").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(59.0)).unwrap();
	let expected = S::from_str("0.0000000425220839059883623744333365029331731745680194453438028952946488279204632909635819260074640624225139617919921875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(59.5)).unwrap();
	let expected = S::from_str("0.000000036825204884439351595415670162048973149333916955696682306800").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(60.0)).unwrap();
	let expected = S::from_str("0.000000031891562929491271780825002377199879880926014584007852171470986620940347468222686444505598046816885471343994140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(60.5)).unwrap();
	let expected = S::from_str("0.000000027618903663329513696561752621536729862000437716772511730100").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(61.0)).unwrap();
	let expected = S::from_str("0.00000002391867219711845383561875178289990991069451093800588912860323996570526060116701483337919853511266410350799560546875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(61.5)).unwrap();
	let expected = S::from_str("0.000000020714177747497135272421314466152547396500328287579383797575").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(62.0)).unwrap();
	let expected = S::from_str("0.0000000179390041478388403767140638371749324330208832035044168464524299742789454508752611250343989013344980776309967041015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(62.5)).unwrap();
	let expected = S::from_str("0.000000015535633310622851454315985849614410547375246215684537848181").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(63.0)).unwrap();
	let expected = S::from_str("0.000000013454253110879130282535547877881199324765662402628312634839322480709209088156445843775799176000873558223247528076171875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(63.5)).unwrap();
	let expected = S::from_str("0.000000011651724982967138590736989387210807910531434661763403386136").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(64.0)).unwrap();
	let expected = S::from_str("0.00000001009068983315934771190166090841089949357424680197123447612949186053190681611733438283184938200065516866743564605712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(64.5)).unwrap();
	let expected = S::from_str("0.0000000087387937372253539430527420404081059328985759963225525396020").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(65.0)).unwrap();
	let expected = S::from_str("0.0000000075680173748695107839262456813081746201806851014784258570971188953989301120880007871238870365004913765005767345428466796875").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(65.5)).unwrap();
	let expected = S::from_str("0.0000000065540953029190154572895565303060794496739319972419144047015").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(66.0)).unwrap();
	let expected = S::from_str("0.0000000056760130311521330879446842609811309651355138261088193928228391715491975840660005903429152773753685323754325509071350097656").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(66.5)).unwrap();
	let expected = S::from_str("0.0000000049155714771892615929671673977295595872554489979314358035261").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(67.0)).unwrap();
	let expected = S::from_str("0.0000000042570097733640998159585131957358482238516353695816145446171293786618981880495004427571864580315263992815744131803512573242").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(67.5)).unwrap();
	let expected = S::from_str("0.0000000036866786078919461947253755482971696904415867484485768526446").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(68.0)).unwrap();
	let expected = S::from_str("0.0000000031927573300230748619688848968018861678887265271862109084628470339964236410371253320678898435236447994611808098852634429931").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(68.5)).unwrap();
	let expected = S::from_str("0.0000000027650089559189596460440316612228772678311900613364326394834").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(69.0)).unwrap();
	let expected = S::from_str("0.0000000023945679975173061464766636726014146259165448953896581813471352754973177307778439990509173826427335995958856074139475822448").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(69.5)).unwrap();
	let expected = S::from_str("0.0000000020737567169392197345330237459171579508733925460023244796125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(70.0)).unwrap();
	let expected = S::from_str("0.0000000017959259981379796098574977544510609694374086715422436360103514566229882980833829992881880369820501996969142055604606866836").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(70.5)).unwrap();
	let expected = S::from_str("0.0000000015553175377044148008997678094378684631550444095017433597094").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(71.0)).unwrap();
	let expected = S::from_str("0.0000000013469444986034847073931233158382957270780565036566827270077635924672412235625372494661410277365376497726856541703455150127").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(71.5)).unwrap();
	let expected = S::from_str("0.0000000011664881532783111006748258570784013473662833071263075197820").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(72.0)).unwrap();
	let expected = S::from_str("0.0000000010102083739526135305448424868787217953085423777425120452558226943504309176719029370996057708024032373295142406277591362595").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(72.5)).unwrap();
	let expected = S::from_str("0.00000000087486611495873332550611939280880101052471248034473063983656").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(73.0)).unwrap();
	let expected = S::from_str("0.00000000075765628046446014790863186515904134648140678330688403394186702076282318825392720282470432810180242799713568047081935219466").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(73.5)).unwrap();
	let expected = S::from_str("0.00000000065614958621904999412958954460660075789353436025854797987742").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(74.0)).unwrap();
	let expected = S::from_str("0.00000000056824221034834511093147389886928100986105508748016302545640026557211739119044540211852824607635182099785176035311451414600").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(74.5)).unwrap();
	let expected = S::from_str("0.00000000049211218966428749559719215845495056842015077019391098490806").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	/*
	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(75.0)).unwrap();
	let expected = S::from_str("0.00000000042618165776125883319860542415196075739579131561012226909230019917908804339283405158889618455726386574838882026483588560950").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(75.5)).unwrap();
	let expected = S::from_str("0.00000000036908414224821562169789411884121292631511307764543323868104").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(76.0)).unwrap();
	let expected = S::from_str("0.00000000031963624332094412489895406811397056804684348670759170181922514938431603254462553869167213841794789931129161519862691420712").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(76.5)).unwrap();
	let expected = S::from_str("0.00000000027681310668616171627342058913090969473633480823407492901078").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(77.0)).unwrap();
	let expected = S::from_str("0.00000000023972718249070809367421555108547792603513261503069377636441886203823702440846915401875410381346092448346871139897018565534").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(77.5)).unwrap();
	let expected = S::from_str("0.00000000020760983001462128720506544184818227105225110617555619675809").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(78.0)).unwrap();
	let expected = S::from_str("0.00000000017979538686803107025566166331410844452634946127302033227331414652867776830635186551406557786009569336260153354922763924150").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(78.5)).unwrap();
	let expected = S::from_str("0.00000000015570737251096596540379908138613670328918832963166714756856").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(79.0)).unwrap();
	let expected = S::from_str("0.00000000013484654015102330269174624748558133339476209595476524920498560989650832622976389913554918339507177002195115016192072943113").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(79.5)).unwrap();
	let expected = S::from_str("0.00000000011678052938322447405284931103960252746689124722375036067642").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(80.0)).unwrap();
	let expected = S::from_str("0.00000000010113490511326747701880968561418600004607157196607393690373920742238124467232292435166188754630382751646336262144054707334").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(80.5)).unwrap();
	let expected = S::from_str("0.000000000087585397037418355539636983279701895600168435417812770507319").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(81.0)).unwrap();
	let expected = S::from_str("0.000000000075851178834950607764107264210639500034553678974555452677804405566785933504242193263746415659727870637347521966080410305011").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(81.5)).unwrap();
	let expected = S::from_str("0.000000000065689047778063766654727737459776421700126326563359577880489").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(82.0)).unwrap();
	let expected = S::from_str("0.000000000056888384126212955823080448157979625025915259230916589508353304175089450128181644947809811744795902978010641474560307728758").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(82.5)).unwrap();
	let expected = S::from_str("0.000000000049266785833547824991045803094832316275094744922519683410367").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(83.0)).unwrap();
	let expected = S::from_str("0.000000000042666288094659716867310336118484718769436444423187442131264978131317087596136233710857358808596927233507981105920230796568").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(83.5)).unwrap();
	let expected = S::from_str("0.000000000036950089375160868743284352321124237206321058691889762557775").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(84.0)).unwrap();
	let expected = S::from_str("0.000000000031999716070994787650482752088863539077077333317390581598448733598487815697102175283143019106447695425130985829440173097426").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(84.5)).unwrap();
	let expected = S::from_str("0.000000000027712567031370651557463264240843177904740794018917321918331").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(85.0)).unwrap();
	let expected = S::from_str("0.000000000023999787053246090737862064066647654307807999988042936198836550198865861772826631462357264329835771568848239372080129823069").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(85.5)).unwrap();
	let expected = S::from_str("0.000000000020784425273527988668097448180632383428555595514187991438748").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(86.0)).unwrap();
	let expected = S::from_str("0.000000000017999840289934568053396548049985740730855999991032202149127412649149396329619973596767948247376828676636179529060097367302").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(86.5)).unwrap();
	let expected = S::from_str("0.000000000015588318955145991501073086135474287571416696635640993579061").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(87.0)).unwrap();
	let expected = S::from_str("0.000000000013499880217450926040047411037489305548141999993274151611845559486862047247214980197575961185532621507477134646795073025476").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(87.5)).unwrap();
	let expected = S::from_str("0.000000000011691239216359493625804814601605715678562522476730745184296").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(88.0)).unwrap();
	let expected = S::from_str("0.000000000010124910163088194530035558278116979161106499994955613708884169615146535435411235148181970889149466130607850985096304769107").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(88.5)).unwrap();
	let expected = S::from_str("0.0000000000087684294122696202193536109512042867589218918575480588882220").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(89.0)).unwrap();
	let expected = S::from_str("0.0000000000075936826223161458975266687085877343708298749962167102816631272113599015765584263611364781668620995979558882388222285768307").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(89.5)).unwrap();
	let expected = S::from_str("0.0000000000065763220592022151645152082134032150691914188931610441661665").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(90.0)).unwrap();
	let expected = S::from_str("0.0000000000056952619667371094231450015314408007781224062471625327112473454085199261824188197708523586251465746984669161791166714326230").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(90.5)).unwrap();
	let expected = S::from_str("0.0000000000049322415444016613733864061600524113018935641698707831246249").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(91.0)).unwrap();
	let expected = S::from_str("0.0000000000042714464750528320673587511485806005835918046853718995334355090563899446368141148281392689688599310238501871343375035744672").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(91.5)).unwrap();
	let expected = S::from_str("0.0000000000036991811583012460300398046200393084764201731274030873434686").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(92.0)).unwrap();
	let expected = S::from_str("0.0000000000032035848562896240505190633614354504376938535140289246500766317922924584776105861211044517266449482678876403507531276808504").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(92.5)).unwrap();
	let expected = S::from_str("0.0000000000027743858687259345225298534650294813573151298455523155076015").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(93.0)).unwrap();
	let expected = S::from_str("0.0000000000024026886422172180378892975210765878282703901355216934875574738442193438582079395908283387949837112009157302630648457606378").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(93.5)).unwrap();
	let expected = S::from_str("0.0000000000020807894015444508918973900987721110179863473841642366307011").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(94.0)).unwrap();
	let expected = S::from_str("0.0000000000018020164816629135284169731408074408712027926016412701156681053831645078936559546931212540962377834006867976972986343204783").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(94.5)).unwrap();
	let expected = S::from_str("0.0000000000015605920511583381689230425740790832634897605381231774730258").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(95.0)).unwrap();
	let expected = S::from_str("0.0000000000013515123612471851463127298556055806534020944512309525867510790373733809202419660198409405721783375505150982729739757403587").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(95.5)).unwrap();
	let expected = S::from_str("0.0000000000011704440383687536266922819305593124476173204035923831047693").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(96.0)).unwrap();
	let expected = S::from_str("0.0000000000010136342709353888597345473917041854900515708384232144400633092780300356901814745148807054291337531628863237047304818052690").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(96.5)).unwrap();
	let expected = S::from_str("0.00000000000087783302877656522001921144791948433571299030269428732857704").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(97.0)).unwrap();
	let expected = S::from_str("0.00000000000076022570320154164480091054377813911753867812881741083004748195852252676763610588616052907185031487216474277854786135395181").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(97.5)).unwrap();
	let expected = S::from_str("0.00000000000065837477158242391501440858593961325178474272702071549643278").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(98.0)).unwrap();
	let expected = S::from_str("0.00000000000057016927740115623360068290783360433815400859661305812253561146889189507572707941462039680388773615412355708391089601546386").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(98.5)).unwrap();
	let expected = S::from_str("0.00000000000049378107868681793626080643945470993883855704526553662232458").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(99.0)).unwrap();
	let expected = S::from_str("0.00000000000042762695805086717520051218087520325361550644745979359190170860166892130679530956096529760291580211559266781293317201159789").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.75), S::from_num(99.5)).unwrap();
	let expected = S::from_str("0.00000000000037033580901511345219560482959103245412891778394915246674344").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	 */

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(0.01)).unwrap();
	let expected = S::from_str("0.9983761306100158559947980311004829357566152460099328545491436682").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(0.1)).unwrap();
	let expected = S::from_str("0.9838794565405262890851617779221820042511646481521991509941114123").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(0.5)).unwrap();
	let expected = S::from_str("0.9219544457292887310002274281762793157246805048722464008007752205").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(0.75)).unwrap();
	let expected = S::from_str("0.8852464509219426525236080073563532529674134415306102174568717413").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(1.0)).unwrap();
	let expected = S::from_str("0.85").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(1.5)).unwrap();
	let expected = S::from_str("0.7836612788698954213501933139498374183659784291414094406806589374").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(2.0)).unwrap();
	let expected = S::from_str("0.7225").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(2.5)).unwrap();
	let expected = S::from_str("0.6661120870394111081476643168573618056110816647701980245785600968").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(3.0)).unwrap();
	let expected = S::from_str("0.614125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(3.5)).unwrap();
	let expected = S::from_str("0.5661952739834994419255146693287575347694194150546683208917760823").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(4.0)).unwrap();
	let expected = S::from_str("0.52200625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(4.5)).unwrap();
	let expected = S::from_str("0.4812659828859745256366874689294439045540065027964680727580096699").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(5.0)).unwrap();
	let expected = S::from_str("0.4437053125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(5.5)).unwrap();
	let expected = S::from_str("0.4090760854530783467911843485900273188709055273769978618443082194").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(6.0)).unwrap();
	let expected = S::from_str("0.377149515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(6.5)).unwrap();
	let expected = S::from_str("0.3477146726351165947725066963015232210402696982704481825676619865").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(7.0)).unwrap();
	let expected = S::from_str("0.32057708828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(7.5)).unwrap();
	let expected = S::from_str("0.2955574717398491055566306918562947378842292435298809551825126885").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(8.0)).unwrap();
	let expected = S::from_str("0.2724905250390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(8.5)).unwrap();
	let expected = S::from_str("0.2512238509788717397231360880778505272015948570003988119051357852").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(9.0)).unwrap();
	let expected = S::from_str("0.231616946283203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(9.5)).unwrap();
	let expected = S::from_str("0.2135402733320409787646656748661729481213556284503389901193654174").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(10.0)).unwrap();
	let expected = S::from_str("0.19687440434072265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(10.5)).unwrap();
	let expected = S::from_str("0.1815092323322348319499658236362470059031522841827881416014606048").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(11.0)).unwrap();
	let expected = S::from_str("0.1673432436896142578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(11.5)).unwrap();
	let expected = S::from_str("0.1542828474823996071574709500908099550176794415553699203612415141").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(12.0)).unwrap();
	let expected = S::from_str("0.142241757136172119140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(12.5)).unwrap();
	let expected = S::from_str("0.1311404203600396660838503075771884617650275253220644323070552870").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(13.0)).unwrap();
	let expected = S::from_str("0.12090549356574630126953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(13.5)).unwrap();
	let expected = S::from_str("0.1114693573060337161712727614406101925002733965237547674609969939").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(14.0)).unwrap();
	let expected = S::from_str("0.1027696695308843560791015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(14.5)).unwrap();
	let expected = S::from_str("0.0947489537101286587455818472245186636252323870451915523418474448").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(15.0)).unwrap();
	let expected = S::from_str("0.087354219101251702667236328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(15.5)).unwrap();
	let expected = S::from_str("0.0805366106536093599337445701408408640814475289884128194905703281").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(16.0)).unwrap();
	let expected = S::from_str("0.07425108623606394726715087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(16.5)).unwrap();
	let expected = S::from_str("0.0684561190555679559436828846197147344692303996401508965669847789").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(17.0)).unwrap();
	let expected = S::from_str("0.0631134233006543551770782470703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(17.5)).unwrap();
	let expected = S::from_str("0.0581877011972327625521304519267575242988458396941282620819370620").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(18.0)).unwrap();
	let expected = S::from_str("0.053646409805556201900516510009765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(18.5)).unwrap();
	let expected = S::from_str("0.0494595460176478481693108841377438956540189637400090227696465027").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(19.0)).unwrap();
	let expected = S::from_str("0.04559944833472277161543903350830078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(19.5)).unwrap();
	let expected = S::from_str("0.0420406141150006709439142515170823113059161191790076693541995273").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(20.0)).unwrap();
	let expected = S::from_str("0.0387595310845143558731231784820556640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(20.5)).unwrap();
	let expected = S::from_str("0.0357345219977505703023271137895199646100287013021565189510695982").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(21.0)).unwrap();
	let expected = S::from_str("0.032945601421837202492154701709747314453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(21.5)).unwrap();
	let expected = S::from_str("0.0303743436980879847569780467210919699185243961068330411084091585").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(22.0)).unwrap();
	let expected = S::from_str("0.02800376120856162211833149645328521728515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(22.5)).unwrap();
	let expected = S::from_str("0.0258181921433747870434313397129281744307457366908080849421477847").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(23.0)).unwrap();
	let expected = S::from_str("0.0238031970272773788005817719852924346923828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(23.5)).unwrap();
	let expected = S::from_str("0.0219454633218685689869166387559889482661338761871868722008256170").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(24.0)).unwrap();
	let expected = S::from_str("0.020232717473185771980494506187498569488525390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(24.5)).unwrap();
	let expected = S::from_str("0.0186536438235882836388791429425906060262137947591088413707017744").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(25.0)).unwrap();
	let expected = S::from_str("0.01719780985220790618342033025937378406524658203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(25.5)).unwrap();
	let expected = S::from_str("0.0158555972500500410930472715012020151222817255452425151650965083").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(26.0)).unwrap();
	let expected = S::from_str("0.0146181383743767202559072807204677164554595947265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(26.5)).unwrap();
	let expected = S::from_str("0.0134772576625425349290901807760217128539394667134561378903320320").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(27.0)).unwrap();
	let expected = S::from_str("0.012425417618220212217521188612397558987140655517578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(27.5)).unwrap();
	let expected = S::from_str("0.0114556690131611546897266536596184559258485467064377172067822272").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(28.0)).unwrap();
	let expected = S::from_str("0.01056160497548718038489301032053792513906955718994140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(28.5)).unwrap();
	let expected = S::from_str("0.0097373186611869814862676556106756875369712647004720596257648931").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(29.0)).unwrap();
	let expected = S::from_str("0.0089773642291641033271590587724572363682091236114501953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(29.5)).unwrap();
	let expected = S::from_str("0.0082767208620089342633275072690743344064255749954012506819001591").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(30.0)).unwrap();
	let expected = S::from_str("0.007630759594789487828085199956588650912977755069732666015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(30.5)).unwrap();
	let expected = S::from_str("0.0070352127327075941238283811787131842454617387460910630796151353").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(31.0)).unwrap();
	let expected = S::from_str("0.00648614565557106465387241996310035327603109180927276611328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(31.5)).unwrap();
	let expected = S::from_str("0.0059799308228014550052541240019062066086424779341774036176728650").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(32.0)).unwrap();
	let expected = S::from_str("0.0055132238072354049557915569686353002846264280378818511962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(32.5)).unwrap();
	let expected = S::from_str("0.0050829411993812367544660054016202756173461062440507930750219352").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(33.0)).unwrap();
	let expected = S::from_str("0.004686240236150094212422823423340005241932463832199573516845703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(33.5)).unwrap();
	let expected = S::from_str("0.0043205000194740512412961045913772342747441903074431741137686449").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(34.0)).unwrap();
	let expected = S::from_str("0.00398330420072758008055939990983900445564259425736963748931884765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(34.5)).unwrap();
	let expected = S::from_str("0.0036724250165529435551016889026706491335325617613266979967033482").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(35.0)).unwrap();
	let expected = S::from_str("0.0033858085706184430684754899233631537872962051187641918659210205078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(35.5)).unwrap();
	let expected = S::from_str("0.0031215612640700020218364355672700517635026774971276932971978459").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(36.0)).unwrap();
	let expected = S::from_str("0.002877937285025676608204166434858680719201774350949563086032867431640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(36.5)).unwrap();
	let expected = S::from_str("0.0026533270744595017185609702321795439989772758725585393026181690").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(37.0)).unwrap();
	let expected = S::from_str("0.00244624669227182511697354146962987861132150819830712862312793731689453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(37.5)).unwrap();
	let expected = S::from_str("0.0022553280132905764607768246973526123991306844916747584072254437").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(38.0)).unwrap();
	let expected =
		S::from_str("0.0020793096884310513494275102491853968196232819685610593296587467193603515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(38.5)).unwrap();
	let expected = S::from_str("0.0019170288112969899916603009927497205392610818179235446461416271").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(39.0)).unwrap();
	let expected =
		S::from_str("0.001767413235166393647013383711807587296679789673276900430209934711456298828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(39.5)).unwrap();
	let expected = S::from_str("0.0016294744896024414929112558438372624583719195452350129492203830").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(40.0)).unwrap();
	let expected =
		S::from_str("0.00150230124989143459996137615503644920217782122228536536567844450473785400390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(40.5)).unwrap();
	let expected = S::from_str("0.0013850533161620752689745674672616730896161316134497610068373256").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(41.0)).unwrap();
	let expected =
		S::from_str("0.0012769560624077194099671697317809818218511480389425605608266778290271759033203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(41.5)).unwrap();
	let expected = S::from_str("0.0011772953187377639786283823471724221261737118714322968558117267").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(42.0)).unwrap();
	let expected =
		S::from_str("0.001085412653046561498472094272013834548573475833101176476702676154673099517822265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(42.5)).unwrap();
	let expected = S::from_str("0.0010007010209270993818341249950965588072476550907174523274399677").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(43.0)).unwrap();
	let expected =
		S::from_str("0.00092260075508957727370128013121175936628745445813600000519727473147213459014892578125")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(43.5)).unwrap();
	let expected = S::from_str("0.0008505958677880344745590062458320749861605068271098344783239726").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(44.0)).unwrap();
	let expected =
		S::from_str("0.0007842106418261406826460881115299954613443362894156000044176835217513144016265869140625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(44.5)).unwrap();
	let expected = S::from_str("0.0007230064876198293033751553089572637382364308030433593065753767").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(45.0)).unwrap();
	let expected =
		S::from_str("0.000666579045552219580249174894800496142142685846003260003755030993488617241382598876953125")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(45.5)).unwrap();
	let expected = S::from_str("0.0006145555144768549078688820126136741775009661825868554105890702").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(46.0)).unwrap();
	let expected =
		S::from_str("0.00056659218871938664321179866058042172082128296910277100319177634446532465517520904541015625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(46.5)).unwrap();
	let expected = S::from_str("0.0005223721873053266716885497107216230508758212551988270990007096").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(47.0)).unwrap();
	let expected =
		S::from_str("0.0004816033604114786467300288614933584626980905237373553527130098927955259568989276885986328125")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(47.5)).unwrap();
	let expected = S::from_str("0.0004440163592095276709352672541133795932444480669190030341506032").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(48.0)).unwrap();
	let expected = S::from_str(
		"0.000409362856349756849720524532269354693293376945176752049806058408876197063364088535308837890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(48.5)).unwrap();
	let expected = S::from_str("0.0003774139053280985202949771659963726542577808568811525790280127").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(49.0)).unwrap();
	let expected = S::from_str(
		"0.00034795842789729332226244585242895148929937040340023924233514964754476750385947525501251220703125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(49.5)).unwrap();
	let expected = S::from_str("0.0003208018195288837422507305910969167561191137283489796921738108").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(50.0)).unwrap();
	let expected = S::from_str(
		"0.0002957646637126993239230789745646087659044648428902033559848772004130523782805539667606353759765625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(50.5)).unwrap();
	let expected = S::from_str("0.0002726815465995511809131210024323792427012466690966327383477392").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(51.0)).unwrap();
	let expected = S::from_str(
		"0.000251399964155794425334617128379917451018795116456672852587145620351094521538470871746540069580078125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(51.5)).unwrap();
	let expected = S::from_str("0.0002317793146096185037761528520675223562960596687321378275955783").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(52.0)).unwrap();
	let expected = S::from_str(
		"0.00021368996953242526153442455912292983336597584898817192469907377729843034330770024098455905914306640625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(52.5)).unwrap();
	let expected = S::from_str("0.0001970124174181757282097299242573940028516507184223171534562415").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(53.0)).unwrap();
	let expected = S::from_str(
		"0.0001816364741025614723042608752544903583610794716399461359942127107036657918115452048368752002716064453125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(53.5)).unwrap();
	let expected = S::from_str("0.0001674605548054493689782704356187849024239031106589695804378053").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(54.0)).unwrap();
	let expected = S::from_str("0.000154391002987177251458621743966316804606917550893954215595080804098115923039813424111343920230865478515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(54.5)).unwrap();
	let expected = S::from_str("0.0001423414715846319636315298702759671670603176440601241433721345").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(55.0)).unwrap();
	let expected = S::from_str("0.00013123235253910066373982848237136928391587991825986108325581868348339853458384141049464233219623565673828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(55.5)).unwrap();
	let expected = S::from_str("0.0001209902508469371690868003897345720920012699974511055218663143").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(56.0)).unwrap();
	let expected = S::from_str("0.0001115474996582355641788542100156638913284979305208819207674458809608887543962651989204459823668003082275390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(56.5)).unwrap();
	let expected = S::from_str("0.0001028417132198965937237803312743862782010794978334396935863672").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(57.0)).unwrap();
	let expected = S::from_str("0.000094815374709500229552026078513314307629223240942749632652328998816755441236825419082379085011780261993408203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(57.5)).unwrap();
	let expected = S::from_str("0.0000874154562369121046652132815832283364709175731584237395484121").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(58.0)).unwrap();
	let expected = S::from_str("0.00008059306850307519511922216673631716148483975480133718775447964899424212505130160622002222226001322269439697265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(58.5)).unwrap();
	let expected = S::from_str("0.0000743031378013752889654312893457440860002799371846601786161503").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(59.0)).unwrap();
	let expected = S::from_str("0.0000685041082276139158513388417258695872621137915811366095913077016451058062936063652870188889210112392902374267578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(59.5)).unwrap();
	let expected = S::from_str("0.0000631576671311689956206165959438824731002379466069611518237277").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(60.0)).unwrap();
	let expected = S::from_str("0.000058228491993471828473638015466989149172796722843966118152611546398339935349565410493966055582859553396701812744140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(60.5)).unwrap();
	let expected = S::from_str("0.0000536840170614936462775241065523001021352022546159169790501685").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(61.0)).unwrap();
	let expected = S::from_str("0.00004949421819445105420259231314694077679687721441737120042971981443858894504713059891987114724543062038719654083251953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(61.5)).unwrap();
	let expected = S::from_str("0.0000456314145022695993358954905694550868149219164235294321926433").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(62.0)).unwrap();
	let expected = S::from_str("0.0000420700854652833960722034661748996602773456322547655203652618422728006032900610090818904751586160273291170597076416015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(62.5)).unwrap();
	let expected = S::from_str("0.0000387867023269291594355111669840368237926836289600000173637468").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(63.0)).unwrap();
	let expected = S::from_str("0.000035759572645490886661372946248664711235743787416550692310472565931880512796551857719606903884823623229749500751495361328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(63.5)).unwrap();
	let expected = S::from_str("0.0000329686969778897855201844919364313002237810846160000147591847").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(64.0)).unwrap();
	let expected = S::from_str("0.0000303956367486672536621670043113650045503822193040680884639016810420984358770690790616658683021000797452870756387710571289062").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(64.5)).unwrap();
	let expected = S::from_str("0.0000280233924312063176921568181459666051902139219236000125453070").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(65.0)).unwrap();
	let expected = S::from_str("0.0000258362912363671656128419536646602538678248864084578751943164288857836704955087172024159880567850677834940142929553985595703").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(65.5)).unwrap();
	let expected = S::from_str("0.0000238198835665253700383332954240716144116818336350600106635110").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(66.0)).unwrap();
	let expected = S::from_str("0.0000219608475509120907709156606149612157876511534471891939151689645529161199211824096220535898482673076159699121490120887756347").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(66.5)).unwrap();
	let expected = S::from_str("0.0000202469010315465645325833011104608722499295585898010090639843").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(67.0)).unwrap();
	let expected = S::from_str("0.0000186667204182752771552783115227170334195034804301108148278936198699787019330050481787455513710272114735744253266602754592895").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(67.5)).unwrap();
	let expected = S::from_str("0.0000172098658768145798526958059438917414124401248013308577043867").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(68.0)).unwrap();
	let expected = S::from_str("0.0000158667123555339855819865647943094784065779583655941926037095768894818966430542909519337186653731297525382615276612341403961").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(68.5)).unwrap();
	let expected = S::from_str("0.0000146283859952923928747914350523079802005741060811312290487286").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(69.0)).unwrap();
	let expected = S::from_str("0.0000134867055022038877446885800751630566455912646107550637131531403560596121465961473091436608655671602896575222985120490193367").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(69.5)).unwrap();
	let expected = S::from_str("0.0000124341280959985339435727197944617831704879901689615446914193").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(70.0)).unwrap();
	let expected = S::from_str("0.0000114636996768733045829852930638885981487525749191418041561801693026506703246067252127721117357320862462088939537352416664361").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(70.5)).unwrap();
	let expected = S::from_str("0.0000105690088815987538520368118252925156949147916436173129877064").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(71.0)).unwrap();
	let expected = S::from_str("0.0000097441447253423088955374991043053084264396886812705335327531439072530697759157164308562949753722733092775598606749554164707").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(71.5)).unwrap();
	let expected = S::from_str("0.0000089836575493589407742312900514986383406775728970747160395505").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(72.0)).unwrap();
	let expected = S::from_str("0.0000082825230165409625612068742386595121624737353790799535028401723211651093095283589662278507290664323128859258815737121040001").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(72.5)).unwrap();
	let expected = S::from_str("0.0000076361089169550996580965965437738425895759369625135086336179").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(73.0)).unwrap();
	let expected = S::from_str("0.0000070401445640598181770258431028605853381026750722179604774141464729903429130991051212936731197064674659530369993376552884001").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(73.5)).unwrap();
	let expected = S::from_str("0.0000064906925794118347093821070622077662011395464181364823385752").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(74.0)).unwrap();
	let expected = S::from_str("0.0000059841228794508454504719666374314975373872738113852664058020245020417914761342393530996221517504973460600814494370069951401").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(74.5)).unwrap();
	let expected = S::from_str("0.0000055170886925000595029747910028766012709686144554160099877889").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(75.0)).unwrap();
	let expected = S::from_str("0.0000050865044475332186329011716418167729067791827396774764449317208267355227547141034501346788289879227441510692320214559458690").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(75.5)).unwrap();
	let expected = S::from_str("0.0000046895253886250505775285723524451110803233222871036084896206").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(76.0)).unwrap();
	let expected = S::from_str("0.0000043235287804032358379659958955442569707623053287258549781919627027251943415069879326144770046397343325284088472182375539887").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(76.5)).unwrap();
	let expected = S::from_str("0.0000039860965803312929908992864995783444182748239440380672161775").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(77.0)).unwrap();
	let expected = S::from_str("0.0000036749994633427504622710965112126184251479595294169767314631682973164151902809397427223054539437741826491475201355019208904").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(77.5)).unwrap();
	let expected = S::from_str("0.0000033881820932815990422643935246415927555336003524323571337508").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(78.0)).unwrap();
	let expected = S::from_str("0.0000031237495438413378929304320345307256613757656000044302217436930527189529117387987813139596358522080552517753921151766327568").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(78.5)).unwrap();
	let expected = S::from_str("0.0000028799547792893591859247344959453538422035602995675035636882").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(79.0)).unwrap();
	let expected = S::from_str("0.0000026551871122651372089908672293511168121694007600037656884821390948111099749779789641168656904743768469640090832979001378433").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(79.5)).unwrap();
	let expected = S::from_str("0.0000024479615623959553080360243215535507658730262546323780291350").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(80.0)).unwrap();
	let expected = S::from_str("0.0000022569090454253666276422371449484492903439906460032008352098182305894434787312821194993358369032203199194077208032151171668").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(80.5)).unwrap();
	let expected = S::from_str("0.0000020807673280365620118306206733205181509920723164375213247647").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(81.0)).unwrap();
	let expected = S::from_str("0.0000019183726886115616334959015732061818967923920491027207099283454960010269569215898015744354613677372719314965626827328495918").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(81.5)).unwrap();
	let expected = S::from_str("0.0000017686522288310777100560275723224404283432614689718931260500").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(82.0)).unwrap();
	let expected = S::from_str("0.0000016306167853198273884715163372252546122735332417373126034390936716008729133833513313382701421625766811417720782803229221530").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(82.5)).unwrap();
	let expected = S::from_str("0.0000015033543945064160535476234364740743640917722486261091571425").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(83.0)).unwrap();
	let expected = S::from_str("0.0000013860242675218532802007888866414664204325032554767157129232296208607419763758486316375296208381901789705062665382744838300").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(83.5)).unwrap();
	let expected = S::from_str("0.0000012778512353304536455154799210029632094780064113321927835711").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(84.0)).unwrap();
	let expected = S::from_str("0.0000011781206273935752881706705536452464573676277671552083559847451777316306799194713368919001777124616521249303265575333112555").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(84.5)).unwrap();
	let expected = S::from_str("0.0000010861735500308855986881579328525187280563054496323638660354").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(85.0)).unwrap();
	let expected = S::from_str("0.0000010014025332845389949450699705984594887624836020819271025870334010718860779315506363581151510555924043061907775739033145672").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(85.5)).unwrap();
	let expected = S::from_str("0.00000092324751752625275888493424292464091884785963218750928613016").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(86.0)).unwrap();
	let expected = S::from_str("0.00000085119215329185814570330947500869056544811106176963803719897839091110316624181804090439787839725354366026216093781781738214").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(86.5)).unwrap();
	let expected = S::from_str("0.00000078476038989731484505219410648594478102068068735938289321064").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(87.0)).unwrap();
	let expected = S::from_str("0.00000072351333029807942384781305375738698063089440250419233161913163227443769130554533476873819663766551211122283679714514477482").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(87.5)).unwrap();
	let expected = S::from_str("0.00000066704633141271761829436499051305306386757858425547545922904").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(88.0)).unwrap();
	let expected = S::from_str("0.00000061498633075336751027064109569377893353626024212856348187626188743327203760971353455342746714201568529453941127757337305860").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(88.5)).unwrap();
	let expected = S::from_str("0.00000056698938170080997555021024193609510428744179661715414034468").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(89.0)).unwrap();
	let expected = S::from_str("0.00000052273838114036238373004493133971209350582120580927895959482260431828123196825650437041334707071333250035849958593736709981").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(89.5)).unwrap();
	let expected = S::from_str("0.00000048194097444568847921767870564568083864432552712458101929298").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(90.0)).unwrap();
	let expected = S::from_str("0.00000044432762396930802617053819163875527947994802493788711565559921367053904717301802871485134501010633262530472464804676203483").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(90.5)).unwrap();
	let expected = S::from_str("0.00000040964982827883520733502689979882871284767669805589386639903").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(91.0)).unwrap();
	let expected = S::from_str("0.00000037767848037391182224495746289294198755795582119720404830725933161995819009706532440762364325859038273150901595083974772961").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(91.5)).unwrap();
	let expected = S::from_str("0.00000034820235403700992623477286482900440592052519334750978643918").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(92.0)).unwrap();
	let expected = S::from_str("0.00000032102670831782504890821384345900068942426244801762344106117043187696446158250552574648009676980182532178266355821378557017").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(92.5)).unwrap();
	let expected = S::from_str("0.00000029597200093145843729955693510465374503244641434538331847330").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(93.0)).unwrap();
	let expected = S::from_str("0.00000027287270207015129157198176694015058601062308081497992490199486709541979234512969688450808225433155152351526402448171773464").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(93.5)).unwrap();
	let expected = S::from_str("0.00000025157620079173967170462339483895568327757945219357582070230").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(94.0)).unwrap();
	let expected = S::from_str("0.00000023194179675962859783618450189912799810902961869273293616669563703110682349336024235183186991618181879498797442080946007444").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(94.5)).unwrap();
	let expected = S::from_str("0.00000021383977067297872094892988561311233078594253436453944759696").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(95.0)).unwrap();
	let expected = S::from_str("0.00000019715052724568430816075682661425879839267517588882299574169129147644079996935620599905708942875454597573977825768804106328").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(95.5)).unwrap();
	let expected = S::from_str("0.00000018176380507203191280659040277114548116805115420985853045741").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(96.0)).unwrap();
	let expected = S::from_str("0.00000016757794815883166193664330262211997863377389950549954638043759775497467997395277509919852601444136407937881151903483490378").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(96.5)).unwrap();
	let expected = S::from_str("0.00000015449923431122712588560184235547365899284348107837975088880").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(97.0)).unwrap();
	let expected = S::from_str("0.00000014244125593500691264614680722880198183870781457967461442337195809172847797785985883431874711227515946747198979117960966822").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(97.5)).unwrap();
	let expected = S::from_str("0.00000013132434916454305700276156600215261014391695891662278825548").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(98.0)).unwrap();
	let expected = S::from_str("0.00000012107506754475587574922478614448168456290164239272342225986616437796920628118088000917093504543388554735119132250266821798").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(98.5)).unwrap();
	let expected = S::from_str("0.00000011162569678986159845234733110182971862232941507912937001716").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(99.0)).unwrap();
	let expected = S::from_str("0.00000010291380741304249438684106822280943187846639603381490892088623972127382533900374800779529478861880271524851262412726798528").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.85), S::from_num(99.5)).unwrap();
	let expected = S::from_str("0.000000094881842271382358684495231436555260828980002817259964514587").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(0.01)).unwrap();
	let expected = S::from_str("0.9994871985837377078975799407374566822258364591826758293399876070").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(0.1)).unwrap();
	let expected = S::from_str("0.9948838031081762988652584981702691935499943586161215693506918918").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(0.5)).unwrap();
	let expected = S::from_str("0.9746794344808963906838413199899600299252583900337491031991750005").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(0.75)).unwrap();
	let expected = S::from_str("0.9622606002309621588456940537583053989668830809621746604353297274").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(1.0)).unwrap();
	let expected = S::from_str("0.95").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(1.5)).unwrap();
	let expected = S::from_str("0.9259454627568515711496492539904620284289954705320616480392162505").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(2.0)).unwrap();
	let expected = S::from_str("0.9025").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(2.5)).unwrap();
	let expected = S::from_str("0.8796481896190089925921667912909389270075456970054585656372554380").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(3.0)).unwrap();
	let expected = S::from_str("0.857375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(3.5)).unwrap();
	let expected = S::from_str("0.8356657801380585429625584517263919806571684121551856373553926661").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(4.0)).unwrap();
	let expected = S::from_str("0.81450625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(4.5)).unwrap();
	let expected = S::from_str("0.7938824911311556158144305291400723816243099915474263554876230328").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(5.0)).unwrap();
	let expected = S::from_str("0.7737809375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(5.5)).unwrap();
	let expected = S::from_str("0.7541883665745978350237090026830687625430944919700550377132418811").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(6.0)).unwrap();
	let expected = S::from_str("0.735091890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(6.5)).unwrap();
	let expected = S::from_str("0.7164789482458679432725235525489153244159397673715522858275797871").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(7.0)).unwrap();
	let expected = S::from_str("0.69833729609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(7.5)).unwrap();
	let expected = S::from_str("0.6806550008335745461088973749214695581951427790029746715362007977").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(8.0)).unwrap();
	let expected = S::from_str("0.6634204312890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(8.5)).unwrap();
	let expected = S::from_str("0.6466222507918958188034525061753960802853856400528259379593907578").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(9.0)).unwrap();
	let expected = S::from_str("0.630249409724609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(9.5)).unwrap();
	let expected = S::from_str("0.6142911382523010278632798808666262762711163580501846410614212199").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(10.0)).unwrap();
	let expected = S::from_str("0.59873693923837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(10.5)).unwrap();
	let expected = S::from_str("0.5835765813396859764701158868232949624575605401476754090083501589").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(11.0)).unwrap();
	let expected = S::from_str("0.5688000922764599609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(11.5)).unwrap();
	let expected = S::from_str("0.5543977522727016776466100924821302143346825131402916385579326510").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(12.0)).unwrap();
	let expected = S::from_str("0.540360087662636962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(12.5)).unwrap();
	let expected = S::from_str("0.5266778646590665937642795878580237036179483874832770566300360184").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(13.0)).unwrap();
	let expected = S::from_str("0.51334208327950511474609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(13.5)).unwrap();
	let expected = S::from_str("0.5003439714261132640760656084651225184370509681091132037985342175").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(14.0)).unwrap();
	let expected = S::from_str("0.4876749791155298590087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(14.5)).unwrap();
	let expected = S::from_str("0.4753267728548076008722623280418663925151984197036575436086075066").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(15.0)).unwrap();
	let expected = S::from_str("0.463291230159753366058349609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(15.5)).unwrap();
	let expected = S::from_str("0.4515604342120672208286492116397730728894384987184746664281771313").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(16.0)).unwrap();
	let expected = S::from_str("0.44012666865176569775543212890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(16.5)).unwrap();
	let expected = S::from_str("0.4289824125014638597872167510577844192449665737825509331067682747").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(17.0)).unwrap();
	let expected = S::from_str("0.4181203352191774128676605224609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(17.5)).unwrap();
	let expected = S::from_str("0.4075332918763906667978559135048951982827182450934233864514298610").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(18.0)).unwrap();
	let expected = S::from_str("0.397214318458218542224277496337890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(18.5)).unwrap();
	let expected = S::from_str("0.3871566272825711334579631178296504383685823328387522171288583679").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(19.0)).unwrap();
	let expected = S::from_str("0.37735360253530761511306362152099609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(19.5)).unwrap();
	let expected = S::from_str("0.3677987959184425767850649619381679164501532161968146062724154495").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(20.0)).unwrap();
	let expected = S::from_str("0.3584859224085422343574104404449462890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(20.5)).unwrap();
	let expected = S::from_str("0.3494088561225204479458117138412595206276455553869738759587946771").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(21.0)).unwrap();
	let expected = S::from_str("0.340561626288115122639539918422698974609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(21.5)).unwrap();
	let expected = S::from_str("0.3319384133163944255485211281491965445962632776176251821608549432").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(22.0)).unwrap();
	let expected = S::from_str("0.32353354497370936650756292250156402587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(22.5)).unwrap();
	let expected = S::from_str("0.3153414926505747042710950717417367173664501137367439230528121960").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(23.0)).unwrap();
	let expected = S::from_str("0.3073568677250238981821847763764858245849609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(23.5)).unwrap();
	let expected = S::from_str("0.2995744180180459690575403181546498814981276080499067269001715862").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(24.0)).unwrap();
	let expected = S::from_str("0.291989024338772703273075537557661533355712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(24.5)).unwrap();
	let expected = S::from_str("0.2845956971171436706046633022469173874232212276474113905551630069").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(25.0)).unwrap();
	let expected = S::from_str("0.27738957312183406810942176067977845668792724609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(25.5)).unwrap();
	let expected = S::from_str("0.2703659122612864870744301371345715180520601662650408210274048566").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(26.0)).unwrap();
	let expected = S::from_str("0.2635200944657423647039506726457895338535308837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(26.5)).unwrap();
	let expected = S::from_str("0.2568476166482221627207086302778429421494571579517887799760346137").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(27.0)).unwrap();
	let expected = S::from_str("0.250344089742455246468753139013500057160854339599609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(27.5)).unwrap();
	let expected = S::from_str("0.2440052358158110545846731987639507950419843000541993409772328830").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(28.0)).unwrap();
	let expected = S::from_str("0.23782688525533248414531548206282505430281162261962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(28.5)).unwrap();
	let expected = S::from_str("0.2318049740250205018554395388257532552898850850514893739283712389").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(29.0)).unwrap();
	let expected = S::from_str("0.2259355409925658599380497079596838015876710414886474609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(29.5)).unwrap();
	let expected = S::from_str("0.2202147253237694767626675618844655925253908307989149052319526769").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(30.0)).unwrap();
	let expected = S::from_str("0.214638763942937566941147222561699611508287489414215087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(30.5)).unwrap();
	let expected = S::from_str("0.2092039890575810029245341837902423128991212892589691599703550431").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(31.0)).unwrap();
	let expected = S::from_str("0.20390682574579068859408986143361463093287311494350433349609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(31.5)).unwrap();
	let expected = S::from_str("0.1987437896047019527783074746007301972541652247960207019718372909").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(32.0)).unwrap();
	let expected = S::from_str("0.1937114844585011541643853683619338993862294591963291168212890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(32.5)).unwrap();
	let expected = S::from_str("0.1888066001244668551393921008706936873914569635562196668732454264").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(33.0)).unwrap();
	let expected = S::from_str("0.184025910235576096456166099943837204416917986236512660980224609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(33.5)).unwrap();
	let expected = S::from_str("0.1793662701182435123824224958271590030218841153784086835295831551").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(34.0)).unwrap();
	let expected = S::from_str("0.17482461472379729163335779494664534419607208692468702793121337890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(34.5)).unwrap();
	let expected = S::from_str("0.1703979566123313367633013710358010528707899096094882493531039973").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(35.0)).unwrap();
	let expected = S::from_str("0.1660833839876074270516899051993130769862684825784526765346527099609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(35.5)).unwrap();
	let expected = S::from_str("0.1618780587817147699251363024840110002272504141290138368854487974").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(36.0)).unwrap();
	let expected = S::from_str("0.157779214788227055699105409939347423136955058449530042707920074462890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(36.5)).unwrap();
	let expected = S::from_str("0.1537841558426290314288794873598104502158878934225631450411763576").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(37.0)).unwrap();
	let expected = S::from_str("0.14989025404881570291415013944238005198010730552705354057252407073974609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(37.5)).unwrap();
	let expected = S::from_str("0.1460949480504975798574355129918199277050934987514349877891175397").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(38.0)).unwrap();
	let expected =
		S::from_str("0.1423957413463749177684426324702610493811019402507008635438978672027587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(38.5)).unwrap();
	let expected = S::from_str("0.1387902006479727008645637373422289313198388238138632383996616627").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(39.0)).unwrap();
	let expected =
		S::from_str("0.135275954279056171880020500846747996912046843238165820366702973842620849609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(39.5)).unwrap();
	let expected = S::from_str("0.1318506906155740658213355504751174847538468826231700764796785796").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(40.0)).unwrap();
	let expected =
		S::from_str("0.12851215656510336328601947580441059706644450107625752934836782515048980712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(40.5)).unwrap();
	let expected = S::from_str("0.1252581560847953625302687729513616105161545384920115726556946506").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(41.0)).unwrap();
	let expected =
		S::from_str("0.1220865487368481951217185020141900672131222760224446528809494338929653167724609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(41.5)).unwrap();
	let expected = S::from_str("0.1189952482805555944037553343037935299903468115674109940229099181").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(42.0)).unwrap();
	let expected =
		S::from_str("0.115982221300005785365632576913480563852466162221322420236901962198317050933837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(42.5)).unwrap();
	let expected = S::from_str("0.1130454858665278146835675675886038534908294709890404443217644221").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(43.0)).unwrap();
	let expected =
		S::from_str("0.11018311023500549609735094806780653565984285411025629922505686408840119838714599609375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(43.5)).unwrap();
	let expected = S::from_str("0.1073932115732014239493891892091736608162879974395884221056762010").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(44.0)).unwrap();
	let expected =
		S::from_str("0.1046739547232552212924834006644162088768507114047434842638040208839811384677886962890625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(44.5)).unwrap();
	let expected = S::from_str("0.1020235509945413527519197297487149777754735975676090010003923910").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(45.0)).unwrap();
	let expected =
		S::from_str("0.099440256987092460227859230631195398433008175834506310050613819839782081544399261474609375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(45.5)).unwrap();
	let expected = S::from_str("0.0969223734448142851143237432612792288866999176892285509503727714").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(46.0)).unwrap();
	let expected =
		S::from_str("0.09446824413773783721646626909963562851135776704278099454808312884779297746717929840087890625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(46.5)).unwrap();
	let expected = S::from_str("0.0920762547725735708586075560982152674423649218047671234028541329").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(47.0)).unwrap();
	let expected =
		S::from_str("0.0897448319308509453556429556446538470857898786906419448206789724054033285938203334808349609375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(47.5)).unwrap();
	let expected = S::from_str("0.0874724420339448923156771782933045040702466757145287672327114262").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(48.0)).unwrap();
	let expected = S::from_str(
		"0.085257590334308398087860807862421154731500384756109847579645023785133162164129316806793212890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(48.5)).unwrap();
	let expected = S::from_str("0.0830988199322476476998933193786392788667343419288023288710758549").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(49.0)).unwrap();
	let expected = S::from_str(
		"0.08099471081759297818346776746930009699492536551830435520066277259587650405592285096645355224609375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(49.5)).unwrap();
	let expected = S::from_str("0.0789438789356352653148986534097073149233976248323622124275220622").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(50.0)).unwrap();
	let expected = S::from_str(
		"0.0769449752767133292742943790958350921451790972423891374406296339660826788531267084181308746337890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(50.5)).unwrap();
	let expected = S::from_str("0.0749966849888535020491537207392219491772277435907441018061459590").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(51.0)).unwrap();
	let expected = S::from_str(
		"0.073097726512877662810579660141043337537920142380269680568598152267778544910470372997224330902099609375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(51.5)).unwrap();
	let expected = S::from_str("0.0712468507394108269466960347022608517183663564112068967158386611").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(52.0)).unwrap();
	let expected = S::from_str(
		"0.06944284018723377967005067713399117066102413526125619654016824465438961766494685434736311435699462890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(52.5)).unwrap();
	let expected = S::from_str("0.0676845082024402855993612329671478091324480385906465518800467280").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(53.0)).unwrap();
	let expected = S::from_str(
		"0.0659706981778720906865481432772916121279729284981933867131598324216701367816995116299949586391448974609375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(53.5)).unwrap();
	let expected = S::from_str("0.0643002827923182713193931713187904186758256366611142242860443916").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(54.0)).unwrap();
	let expected = S::from_str("0.062672163268978486152220736113427031521574282073283717377501840800586629942614536048495210707187652587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(54.5)).unwrap();
	let expected = S::from_str("0.0610852686527023577534235127528508977420343548280585130717421720").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(55.0)).unwrap();
	let expected = S::from_str("0.05953855510552956184460969930775567994549556796961953150862674876055729844548380924607045017182826995849609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(55.5)).unwrap();
	let expected = S::from_str("0.0580310052200672398657523371152083528549326370866555874181550634").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(56.0)).unwrap();
	let expected = S::from_str("0.0565616273502530837523792143423678959482207895711385549331954113225294335232096187837669276632368564605712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(56.5)).unwrap();
	let expected = S::from_str("0.0551294549590638778724647202594479352121860052323228080472473103").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(57.0)).unwrap();
	let expected = S::from_str("0.053733545982740429564760253625249501150809750092581627186535640756402961847049137844578581280075013637542724609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(57.5)).unwrap();
	let expected = S::from_str("0.0523729822111106839788414842464755384515767049707066676448849447").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(58.0)).unwrap();
	let expected = S::from_str("0.05104686868360340808652224094398702609326926258795254582720885871858281375469668095234965221607126295566558837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(58.5)).unwrap();
	let expected = S::from_str("0.0497543331005551497798994100341517615289978697221713342626406975").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(59.0)).unwrap();
	let expected = S::from_str("0.0484945252494232376821961288967876747886057994585549185358484157826536730669618469047321696052676998078823089599609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(59.5)).unwrap();
	let expected = S::from_str("0.0472666164455273922909044395324441734525479762360627675495086626").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(60.0)).unwrap();
	let expected = S::from_str("0.046069798986952075798086322451948291049175509485627172609055994993520989413613754559495561125004314817488193511962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(60.5)).unwrap();
	let expected = S::from_str("0.0449032856232510226763592175558219647799205774242596291720332295").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(61.0)).unwrap();
	let expected = S::from_str("0.04376630903760447200818200632935087649671673401134581397860319524384493994293306683152078306875409907661378383636474609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(61.5)).unwrap();
	let expected = S::from_str("0.0426581213420884715425412566780308665409245485530466477134315680").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(62.0)).unwrap();
	let expected = S::from_str("0.0415779935857242484077729060128833326718808973107785232796730354816526929457864134899447439153163941227830946445465087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(62.5)).unwrap();
	let expected = S::from_str("0.0405252152749840479654141938441293232138783211253943153277599896").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(63.0)).unwrap();
	let expected = S::from_str("0.039499093906438035987384260712239166038286852445239597115689383707570058298497092815447506719550574416643939912319183349609375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(63.5)).unwrap();
	let expected = S::from_str("0.0384989545112348455671434841519228570531844050691245995613719901").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(64.0)).unwrap();
	let expected = S::from_str("0.0375241392111161341880150476766272077363725098229776172599049145221915553835722381746751313835730456958117429167032241821289062").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(64.5)).unwrap();
	let expected = S::from_str("0.0365740067856731032887863099443267142005251848156683695833033906").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(65.0)).unwrap();
	let expected = S::from_str("0.0356479322505603274786142952927958473495538843318287363969096687960819776143936262659413748143943934110211557708680629730224609").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(65.5)).unwrap();
	let expected = S::from_str("0.0347453064463894481243469944471103784904989255748849511041382211").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(66.0)).unwrap();
	let expected = S::from_str("0.0338655356380323111046835805281560549820761901152372995770641853562778787336739449526443060736746737404700979823246598243713378").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(66.5)).unwrap();
	let expected = S::from_str("0.0330080411240699757181296447247548595659739792961407035489313100").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(67.0)).unwrap();
	let expected = S::from_str("0.0321722588561306955494494015017482522329723806094754345982109760884639847969902477050120907699909400534465930832084268331527709").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(67.5)).unwrap();
	let expected = S::from_str("0.0313576390678664769322231624885171165876752803313336683714847445").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(68.0)).unwrap();
	let expected = S::from_str("0.0305636459133241607719769314266608396213237615790016628683004272840407855571407353197614862314913930507742634290480054914951324").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(68.5)).unwrap();
	let expected = S::from_str("0.0297897571144731530856120043640912607582915163147669849529105073").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(69.0)).unwrap();
	let expected = S::from_str("0.0290354636176579527333780848553277976402575735000515797248854059198387462792836985537734119199168233982355502575956052169203758").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(69.5)).unwrap();
	let expected = S::from_str("0.0283002692587494954313314041458866977203769404990286357052649819").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(70.0)).unwrap();
	let expected = S::from_str("0.0275836904367750550967091806125614077582446948250490007386411356238468089653195136260847413239209822283237727447158249560743570").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(70.5)).unwrap();
	let expected = S::from_str("0.0268852557958120206597648339385923628343580934740772039200017328").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(71.0)).unwrap();
	let expected = S::from_str("0.0262045059149363023418737215819333373703324600837965507017090788426544685170535379447805042577249331169075841074800337082706391").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(71.5)).unwrap();
	let expected = S::from_str("0.0255409930060214196267765922416627446926401888003733437240016462").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(72.0)).unwrap();
	let expected = S::from_str("0.0248942806191894872247800355028366705018158370796067231666236249005217450912008610475414790448386864610622049021060320228571072").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(72.5)).unwrap();
	let expected = S::from_str("0.0242639433557203486454377626295796074580081793603546765378015639").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(73.0)).unwrap();
	let expected = S::from_str("0.0236495665882300128635410337276948369767250452256263870082924436554956578366408179951644050925967521380090946570007304217142518").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(73.5)).unwrap();
	let expected = S::from_str("0.0230507461879343312131658744981006270851077703923369427109114857").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(74.0)).unwrap();
	let expected = S::from_str("0.0224670882588185122203639820413100951278887929643450676578778214727208749448087770954061848379669145311086399241506939006285392").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(74.5)).unwrap();
	let expected = S::from_str("0.0218982088785376146525075807731955957308523818727200955753659114").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(75.0)).unwrap();
	let expected = S::from_str("0.0213437338458775866093457829392445903714943533161278142749839303990848311975683382406358755960685688045532079279431592055971123").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(75.5)).unwrap();
	let expected = S::from_str("0.0208032984346107339198822017345358159443097627790840907965976158").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(76.0)).unwrap();
	let expected = S::from_str("0.0202765471535837072788784937922823608529196356503214235612347338791305896376899213286040818162651403643255475315460012453172566").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(76.5)).unwrap();
	let expected = S::from_str("0.0197631335128801972238880916478090251470942746401298862567677350").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(77.0)).unwrap();
	let expected = S::from_str("0.0192627197959045219149345691026682428102736538678053523831729971851740601558054252621738777254518833461092701549687011830513938").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(77.5)).unwrap();
	let expected = S::from_str("0.0187749768372361873626936870654185738897395609081233919439293483").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(78.0)).unwrap();
	let expected = S::from_str("0.0182995838061092958191878406475348306697599711744150847640143473259153571480151539990651838391792891788038066472202661238988241").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(78.5)).unwrap();
	let expected = S::from_str("0.0178362279953743779945590027121476451952525828627172223467328809").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(79.0)).unwrap();
	let expected = S::from_str("0.0173846046158038310282284486151580891362719726156943305258136299596195892906143962991119246472203247198636163148592528177038829").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(79.5)).unwrap();
	let expected = S::from_str("0.0169444165956056590948310525765402629354899537195813612293962368").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(80.0)).unwrap();
	let expected = S::from_str("0.0165153743850136394768170261844001846794583739849096139995229484616386098260836764841563284148593084838704354991162901768186888").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(80.5)).unwrap();
	let expected = S::from_str("0.0160971957658253761400894999477132497887154560336022931679264250").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(81.0)).unwrap();
	let expected = S::from_str("0.0156896056657629575029761748751801754454854552856641332995468010385566793347794926599485119941163430596769137241604756679777543").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(81.5)).unwrap();
	let expected = S::from_str("0.0152923359775341073330850249503275872992796832319221785095301037").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(82.0)).unwrap();
	let expected = S::from_str("0.0149051253824748096278273661314211666732111825213809266345694609866288453680405180269510863944105259066930680379524518845788666").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(82.5)).unwrap();
	let expected = S::from_str("0.0145277191786574019664307737028112079343156990703260695840535985").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(83.0)).unwrap();
	let expected = S::from_str("0.0141598691133510691464359978248501083395506233953118803028409879372974030996384921256035320746899996113584146360548292903499233").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(83.5)).unwrap();
	let expected = S::from_str("0.0138013332197245318681092350176706475375999141168097661048509186").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(84.0)).unwrap();
	let expected = S::from_str("0.0134518756576835156891141979336076029225730922255462862876989385404325329446565675193233554709554996307904939042520878258324271").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(84.5)).unwrap();
	let expected = S::from_str("0.0131112665587383052747037732667871151607199184109692777996083727").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(85.0)).unwrap();
	let expected = S::from_str("0.0127792818747993399046584880369272227764444376142689719733139916134109062974237391433571876974077246492509692090394834345408057").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(85.5)).unwrap();
	let expected = S::from_str("0.0124557032308013900109685846034477594026839224904208139096279540").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(86.0)).unwrap();
	let expected = S::from_str("0.0121403177810593729094255636350808616376222157335555233746482920327403609825525521861893283125373384167884207485875092628137655").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(86.5)).unwrap();
	let expected = S::from_str("0.0118329180692613205104201553732753714325497263658997732141465563").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(87.0)).unwrap();
	let expected = S::from_str("0.0115333018920064042639542854533268185557411049468777472059158774311033429334249245768798618969104714959489997111581337996730772").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(87.5)).unwrap();
	let expected = S::from_str("0.0112412721657982544848991476046116028609222400476047845534392285").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(88.0)).unwrap();
	let expected = S::from_str("0.0109566367974060840507565711806604776279540496995338598456200835595481757867536783480358688020649479211515497256002271096894233").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(88.5)).unwrap();
	let expected = S::from_str("0.0106792085575083417606541902243810227178761280452245453257672671").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(89.0)).unwrap();
	let expected = S::from_str("0.0104088049575357798482187426216274537465563472145571668533390793815707669974159944306340753619617005250939722393202157542049521").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(89.5)).unwrap();
	let expected = S::from_str("0.0101452481296329246726214807131619715819823216429633180594789037").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(90.0)).unwrap();
	let expected = S::from_str("0.0098883647096589908558078054905460810592285298538293085106721254124922286475451947091023715938636154988392736273542049664947045").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(90.5)).unwrap();
	let expected = S::from_str("0.0096379857231512784389904066775038730028832055608151521565049585").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(91.0)).unwrap();
	let expected = S::from_str("0.0093939464741760413130174152160187770062671033611378430851385191418676172151679349736472530141704347238973099459864947181699693").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(91.5)).unwrap();
	let expected = S::from_str("0.0091560864369937145170408863436286793527390452827743945486797106").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(92.0)).unwrap();
	let expected = S::from_str("0.0089242491504672392473665444552178381559537481930809509308815931847742363544095382249648903634619129877024444486871699822614708").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(92.5)).unwrap();
	let expected = S::from_str("0.0086982821151440287911888420264472453851020930186356748212457251").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(93.0)).unwrap();
	let expected = S::from_str("0.0084780366929438772849982172324569462481560607834269033843375135255355245366890613137166458452888173383173222262528114831483973").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(93.5)).unwrap();
	let expected = S::from_str("0.0082633680093868273516293999251248831158469883677038910801834388").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(94.0)).unwrap();
	let expected = S::from_str("0.0080541348582966834207483063708340989357482577442555582151206378492587483098546082480308135530243764714014561149401709089909774").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(94.5)).unwrap();
	let expected = S::from_str("0.0078501996089174859840479299288686389600546389493186965261742669").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(95.0)).unwrap();
	let expected = S::from_str("0.0076514281153818492497108910522923939889608448570427803043646059567958108943618778356292728753731576478313833091931623635414286").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(95.5)).unwrap();
	let expected = S::from_str("0.0074576896284716116848455334324252070120519070018527616998655535").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(96.0)).unwrap();
	let expected = S::from_str("0.0072688567096127567872253464996777742895128026141906412891463756589560203496437839438478092316044997654398141437335042453643571").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(96.5)).unwrap();
	let expected = S::from_str("0.0070848051470480311006032567608039466614493116517601236148722758").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(97.0)).unwrap();
	let expected = S::from_str("0.0069054138741321189478640791746938855750371624834811092246890568760082193321615947466554187700242747771678234365468290330961393").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(97.5)).unwrap();
	let expected = S::from_str("0.0067305648896956295455730939227637493283768460691721174341286621").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(98.0)).unwrap();
	let expected = S::from_str("0.0065601431804255130004708752159591912962853043593070537634546040322078083655535150093226478315230610383094322647194875814413323").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(98.5)).unwrap();
	let expected = S::from_str("0.0063940366452108480682944392266255618619580037657135115624222289").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(99.0)).unwrap();
	let expected = S::from_str("0.0062321360214042373504473314551612317314710391413417010752818738305974179472758392588565154399469079863939606514835132023692657").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(0.95), S::from_num(99.5)).unwrap();
	let expected = S::from_str("0.0060743348129503056648797172652942837688601035774278359843011175").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(0.01)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(0.1)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(0.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(0.75)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(1.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(1.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(2.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(2.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(3.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(3.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(4.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(4.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(5.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(5.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(6.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(6.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(7.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(7.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(8.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(8.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(9.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(9.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(10.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(10.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(11.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(11.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(12.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(12.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(13.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(13.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(14.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(14.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(15.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(15.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(16.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(16.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(17.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(17.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(18.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(18.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(19.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(19.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(20.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(20.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(21.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(21.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(22.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(22.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(23.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(23.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(24.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(24.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(25.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(25.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(26.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(26.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(27.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(27.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(28.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(28.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(29.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(29.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(30.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(30.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(31.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(31.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(32.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(32.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(33.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(33.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(34.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(34.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(35.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(35.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(36.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(36.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(37.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(37.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(38.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(38.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(39.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(39.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(40.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(40.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(41.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(41.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(42.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(42.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(43.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(43.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(44.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(44.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(45.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(45.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(46.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(46.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(47.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(47.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(48.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(48.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(49.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(49.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(50.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(50.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(51.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(51.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(52.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(52.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(53.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(53.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(54.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(54.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(55.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(55.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(56.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(56.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(57.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(57.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(58.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(58.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(59.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(59.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(60.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(60.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(61.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(61.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(62.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(62.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(63.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(63.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(64.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(64.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(65.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(65.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(66.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(66.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(67.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(67.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(68.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(68.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(69.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(69.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(70.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(70.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(71.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(71.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(72.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(72.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(73.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(73.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(74.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(74.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(75.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(75.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(76.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(76.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(77.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(77.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(78.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(78.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(79.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(79.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(80.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(80.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(81.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(81.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(82.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(82.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(83.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(83.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(84.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(84.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(85.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(85.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(86.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(86.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(87.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(87.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(88.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(88.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(89.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(89.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(90.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(90.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(91.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(91.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(92.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(92.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(93.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(93.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(94.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(94.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(95.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(95.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(96.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(96.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(97.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(97.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(98.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(98.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(99.0)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1), S::from_num(99.5)).unwrap();
	let expected = S::from_str("1").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(0.01)).unwrap();
	let expected = S::from_str("1.0022339270182330724959999259594153373003507281840127222791262512").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(0.1)).unwrap();
	let expected = S::from_str("1.0225651825635729274886316472598280722066861418237486636223965919").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(0.5)).unwrap();
	let expected = S::from_str("1.1180339887498948482045868343656381177203091798057628621354486227").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(0.75)).unwrap();
	let expected = S::from_str("1.1821770112539697666271201498590187757127460032646753267648841778").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(1.0)).unwrap();
	let expected = S::from_str("1.25").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(1.5)).unwrap();
	let expected = S::from_str("1.3975424859373685602557335429570476471503864747572035776693107783").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(2.0)).unwrap();
	let expected = S::from_str("1.5625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(2.5)).unwrap();
	let expected = S::from_str("1.7469281074217107003196669286963095589379830934465044720866384729").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(3.0)).unwrap();
	let expected = S::from_str("1.953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(3.5)).unwrap();
	let expected = S::from_str("2.1836601342771383753995836608703869486724788668081305901082980912").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(4.0)).unwrap();
	let expected = S::from_str("2.44140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(4.5)).unwrap();
	let expected = S::from_str("2.7295751678464229692494795760879836858405985835101632376353726140").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(5.0)).unwrap();
	let expected = S::from_str("3.0517578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(5.5)).unwrap();
	let expected = S::from_str("3.4119689598080287115618494701099796073007482293877040470442157675").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(6.0)).unwrap();
	let expected = S::from_str("3.814697265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(6.5)).unwrap();
	let expected = S::from_str("4.2649611997600358894523118376374745091259352867346300588052697094").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(7.0)).unwrap();
	let expected = S::from_str("4.76837158203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(7.5)).unwrap();
	let expected = S::from_str("5.3312014997000448618153897970468431364074191084182875735065871367").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(8.0)).unwrap();
	let expected = S::from_str("5.9604644775390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(8.5)).unwrap();
	let expected = S::from_str("6.6640018746250560772692372463085539205092738855228594668832339209").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(9.0)).unwrap();
	let expected = S::from_str("7.450580596923828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(9.5)).unwrap();
	let expected = S::from_str("8.3300023432813200965865465578856924006365923569035743336040424012").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(10.0)).unwrap();
	let expected = S::from_str("9.31322574615478515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(10.5)).unwrap();
	let expected = S::from_str("10.412502929101650120733183197357115500795740446129467917005053001").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(11.0)).unwrap();
	let expected = S::from_str("11.6415321826934814453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(11.5)).unwrap();
	let expected = S::from_str("13.015628661377062650916478996696394375994675557661834896256316251").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(12.0)).unwrap();
	let expected = S::from_str("14.551915228366851806640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(12.5)).unwrap();
	let expected = S::from_str("16.269535826721328313645598745870492969993344447077293620320395314").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(13.0)).unwrap();
	let expected = S::from_str("18.18989403545856475830078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(13.5)).unwrap();
	let expected = S::from_str("20.336919783401660392056998432338116212491680558846617025400494143").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(14.0)).unwrap();
	let expected = S::from_str("22.7373675443232059478759765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(14.5)).unwrap();
	let expected = S::from_str("25.421149729252075490071248040422645265614600698558271281750617679").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(15.0)).unwrap();
	let expected = S::from_str("28.421709430404007434844970703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(15.5)).unwrap();
	let expected = S::from_str("31.776437161565094362589060050528306582018250873197839102188272099").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(16.0)).unwrap();
	let expected = S::from_str("35.52713678800500929355621337890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(16.5)).unwrap();
	let expected = S::from_str("39.720546451956367953236325063160383227522813591497298877735340124").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(17.0)).unwrap();
	let expected = S::from_str("44.4089209850062616169452667236328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(17.5)).unwrap();
	let expected = S::from_str("49.650683064945459941545406328950479034403516989371623597169175155").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(18.0)).unwrap();
	let expected = S::from_str("55.511151231257827021181583404541015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(18.5)).unwrap();
	let expected = S::from_str("62.063353831181824926931757911188098793004396236714529496461468943").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(19.0)).unwrap();
	let expected = S::from_str("69.38893903907228377647697925567626953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(19.5)).unwrap();
	let expected = S::from_str("77.579192288977281158664697388985123491255495295893161870576836179").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(20.0)).unwrap();
	let expected = S::from_str("86.7361737988403547205962240695953369140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(20.5)).unwrap();
	let expected = S::from_str("96.973990361221601448330871736231404364069369119866452338221045224").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(21.0)).unwrap();
	let expected = S::from_str("108.420217248550443400745280086994171142578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(21.5)).unwrap();
	let expected = S::from_str("121.21748795152700181041358967028925545508671139983306542277630653").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(22.0)).unwrap();
	let expected = S::from_str("135.52527156068805425093160010874271392822265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(22.5)).unwrap();
	let expected = S::from_str("151.52185993940875226301698708786156931885838924979133177847038316").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(23.0)).unwrap();
	let expected = S::from_str("169.4065894508600678136645001359283924102783203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(23.5)).unwrap();
	let expected = S::from_str("189.40232492426094032877123385982696164857298656223916472308797895").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(24.0)).unwrap();
	let expected = S::from_str("211.758236813575084767080625169910490512847900390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(24.5)).unwrap();
	let expected = S::from_str("236.75290615532617541096404232478370206071623320279895590385997369").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(25.0)).unwrap();
	let expected = S::from_str("264.69779601696885595885078146238811314105987548828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(25.5)).unwrap();
	let expected = S::from_str("295.94113269415771926370505290597962757589529150349869487982496711").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(26.0)).unwrap();
	let expected = S::from_str("330.8722450212110699485634768279851414263248443603515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(26.5)).unwrap();
	let expected = S::from_str("369.92641586769714907963131613247453446986911437937336859978120889").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(27.0)).unwrap();
	let expected = S::from_str("413.590306276513837435704346034981426782906055450439453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(27.5)).unwrap();
	let expected = S::from_str("462.40801983462143634953914516559316808733639297421671074972651112").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(28.0)).unwrap();
	let expected = S::from_str("516.98788284564229679463043254372678347863256931304931640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(28.5)).unwrap();
	let expected = S::from_str("578.01002479327679543692393145699146010917049121777088843715813890").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(29.0)).unwrap();
	let expected = S::from_str("646.2348535570528709932880406796584793482907116413116455078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(29.5)).unwrap();
	let expected = S::from_str("722.51253099159599429615491432123932513646311402221361054644767362").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(30.0)).unwrap();
	let expected = S::from_str("807.793566946316088741610050849573099185363389551639556884765625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(30.5)).unwrap();
	let expected = S::from_str("903.14066373949499287019364290154915642057889252776701318305959203").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(31.0)).unwrap();
	let expected = S::from_str("1009.74195868289511092701256356196637398170423693954944610595703125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(31.5)).unwrap();
	let expected = S::from_str("1128.9258296743687410877420536269364455257236156597087664788244900").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(32.0)).unwrap();
	let expected = S::from_str("1262.1774483536188886587657044524579674771302961744368076324462890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(32.5)).unwrap();
	let expected = S::from_str("1411.1572870929609263596775670336705569071545195746359580985306125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(33.0)).unwrap();
	let expected = S::from_str("1577.721810442023610823457130565572459346412870218046009540557861328125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(33.5)).unwrap();
	let expected = S::from_str("1763.9466088662011579495969587920881961339431494682949476231632656").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(34.0)).unwrap();
	let expected = S::from_str("1972.15226305252951352932141320696557418301608777255751192569732666015625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(34.5)).unwrap();
	let expected = S::from_str("2204.9332610827514474369961984901102451674289368353686845289540821").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(35.0)).unwrap();
	let expected = S::from_str("2465.1903288156618919116517665087069677287701097156968899071216583251953125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(35.5)).unwrap();
	let expected = S::from_str("2756.1665763534393092962452481126378064592861710442108556611926026").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(36.0)).unwrap();
	let expected =
		S::from_str("3081.487911019577364889564708135883709660962637144621112383902072906494140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(36.5)).unwrap();
	let expected = S::from_str("3445.2082204417991366203065601407972580741077138052635695764907532").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(37.0)).unwrap();
	let expected =
		S::from_str("3851.85988877447170611195588516985463707620329643077639047987759113311767578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(37.5)).unwrap();
	let expected = S::from_str("4306.5102755522489207753832001759965725926346422565794619706134416").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(38.0)).unwrap();
	let expected =
		S::from_str("4814.8248609680896326399448564623182963452541205384704880998469889163970947265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(38.5)).unwrap();
	let expected = S::from_str("5383.1378444403111509692290002199957157407933028207243274632668020").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(39.0)).unwrap();
	let expected =
		S::from_str("6018.531076210112040799931070577897870431567650673088110124808736145496368408203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(39.5)).unwrap();
	let expected = S::from_str("6728.9223055503889387115362502749946446759916285259054093290835025").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(40.0)).unwrap();
	let expected =
		S::from_str("7523.16384526264005099991383822237233803945956334136013765601092018187046051025390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(40.5)).unwrap();
	let expected = S::from_str("8411.1528819379861733894203128437433058449895356573817616613543781").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(41.0)).unwrap();
	let expected =
		S::from_str("9403.9548065783000637498922977779654225493244541767001720700136502273380756378173828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(41.5)).unwrap();
	let expected = S::from_str("10513.941102422482716736775391054679132306236919571727202076692972").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(42.0)).unwrap();
	let expected =
		S::from_str("11754.943508222875079687365372222456778186655567720875215087517062784172594547271728515625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(42.5)).unwrap();
	let expected = S::from_str("13142.426378028103395920969238818348915382796149464659002595866215").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(43.0)).unwrap();
	let expected =
		S::from_str("14693.67938527859384960920671527807097273331945965109401885939632848021574318408966064453125")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(43.5)).unwrap();
	let expected = S::from_str("16428.032972535129244901211548522936144228495186830823753244832769").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(44.0)).unwrap();
	let expected =
		S::from_str("18367.0992315982423120115083940975887159166493245638675235742454106002696789801120758056640625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(44.5)).unwrap();
	let expected = S::from_str("20535.041215668911556126514435653670180285618983538529691556040962").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(45.0)).unwrap();
	let expected =
		S::from_str("22958.874039497802890014385492621985894895811655704834404467806763250337098725140094757080078125")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(45.5)).unwrap();
	let expected = S::from_str("25668.801519586139445158143044567087725357023729423162114445051202").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(46.0)).unwrap();
	let expected = S::from_str(
		"28698.59254937225361251798186577748236861976456963104300558475845406292137340642511844635009765625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(46.5)).unwrap();
	let expected = S::from_str("32086.001899482674306447678805708859656696279661778952643056314003").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(47.0)).unwrap();
	let expected = S::from_str(
		"35873.2406867153170156474773322218529607747057120388037569809480675786517167580313980579376220703125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(47.5)).unwrap();
	let expected = S::from_str("40107.502374353342883059598507136074570870349577223690803820392504").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(48.0)).unwrap();
	let expected = S::from_str(
		"44841.550858394146269559346665277316200968382140048504696226185084473314645947539247572422027587890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(48.5)).unwrap();
	let expected = S::from_str("50134.377967941678603824498133920093213587936971529613504775490630").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(49.0)).unwrap();
	let expected = S::from_str(
		"56051.93857299268283694918333159664525121047767506063087028273135559164330743442405946552753448486328125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(49.5)).unwrap();
	let expected = S::from_str("62667.972459927098254780622667400116516984921214412016880969363288").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(50.0)).unwrap();
	let expected = S::from_str(
		"70064.9232162408535461864791644958065640130970938257885878534141944895541342930300743319094181060791015625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(50.5)).unwrap();
	let expected = S::from_str("78334.965574908872818475778334250145646231151518015021101211704110").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(51.0)).unwrap();
	let expected = S::from_str(
		"87581.154020301066932733098955619758205016371367282235734816767743111942667866287592914886772632598876953125",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(51.5)).unwrap();
	let expected = S::from_str("97918.706968636091023094722917812682057788939397518776376514630138").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(52.0)).unwrap();
	let expected = S::from_str("109476.44252537633366591637369452469775627046420910279466852095967888992833483285949114360846579074859619140625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(52.5)).unwrap();
	let expected = S::from_str("122398.38371079511377886840364726585257223617424689847047064328767").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(53.0)).unwrap();
	let expected = S::from_str("136845.5531567204170823954671181558721953380802613784933356511995986124104185410743639295105822384357452392578125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(53.5)).unwrap();
	let expected = S::from_str("152997.97963849389222358550455908231571529521780862308808830410959").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(54.0)).unwrap();
	let expected = S::from_str("171056.941445900521352994333897694840244172600326723116669563999498265513023176342954911888227798044681549072265625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(54.5)).unwrap();
	let expected = S::from_str("191247.47454811736527948188069885289464411902226077886011038013698").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(55.0)).unwrap();
	let expected = S::from_str("213821.17680737565169124291737211855030521575040840389583695499937283189127897042869363986028474755585193634033203125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(55.5)).unwrap();
	let expected = S::from_str("239059.34318514670659935235087356611830514877782597357513797517123").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(56.0)).unwrap();
	let expected = S::from_str("267276.4710092195646140536467151481878815196880105048697961937492160398640987130358670498253559344448149204254150390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(56.5)).unwrap();
	let expected = S::from_str("298824.17898143338324919043859195764788143597228246696892246896404").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(57.0)).unwrap();
	let expected = S::from_str("334095.588761524455767567058393935234851899610013131087245242186520049830123391294833812281694918056018650531768798828125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(57.5)).unwrap();
	let expected = S::from_str("373530.22372679172906148804823994705985179496535308371115308620505").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(58.0)).unwrap();
	let expected = S::from_str("417619.48595190556970945882299241904356487451251641385905655273315006228765423911854226535211864757002331316471099853515625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(58.5)).unwrap();
	let expected = S::from_str("466912.77965848966132686006029993382481474370669135463894135775631").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(59.0)).unwrap();
	let expected = S::from_str("522024.3574398819621368235287405238044560931406455173238206909164375778595677988981778316901483094625291414558887481689453125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(59.5)).unwrap();
	let expected = S::from_str("583640.97457311207665857507537491728101842963336419329867669719539").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(60.0)).unwrap();
	let expected = S::from_str("652530.446799852452671029410925654755570116425806896654775863645546972324459748622722289612685386828161426819860935211181640625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(60.5)).unwrap();
	let expected = S::from_str("729551.21821639009582321884421864660127303704170524162334587149424").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(61.0)).unwrap();
	let expected = S::from_str("815663.05849981556583878676365706844446264553225862081846982955693371540557468577840286201585673353520178352482616901397705078125").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(61.5)).unwrap();
	let expected = S::from_str("911939.02277048761977902355527330825159129630213155202918233936781").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(62.0)).unwrap();
	let expected = S::from_str("1019578.82312476945729848345457133555557830691532327602308728694616714425696835722300357751982091691900222940603271126747131").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(62.5)).unwrap();
	let expected = S::from_str("1139923.77846310952472377944409163531448912037766444003647792").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(63.0)).unwrap();
	let expected = S::from_str("1274473.52890596182162310431821416944447288364415409502885910868270893032121044652875447189977614614875278675754088908433914").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(63.5)).unwrap();
	let expected = S::from_str("1424904.72307888690590472430511454414311140047208055004559740").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(64.0)).unwrap();
	let expected = S::from_str("1593091.91113245227702888039776771180559110455519261878607388585338616290151305816094308987472018268594098344692611135542392").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(64.5)).unwrap();
	let expected = S::from_str("1781130.90384860863238090538139318017888925059010068755699675").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(65.0)).unwrap();
	let expected = S::from_str("1991364.88891556534628610049720963975698888069399077348259235731673270362689132270117886234340022835742622930865763919427990").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(65.5)).unwrap();
	let expected = S::from_str("2226413.62981076079047613172674147522361156323762585944624594").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(66.0)).unwrap();
	let expected = S::from_str("2489206.11114445668285762562151204969623610086748846685324044664591587953361415337647357792925028544678278663582204899284988").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(66.5)).unwrap();
	let expected = S::from_str("2783017.03726345098809516465842684402951445404703232430780743").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(67.0)).unwrap();
	let expected = S::from_str("3111507.63893057085357203202689006212029512608436058356655055830739484941701769172059197241156285680847848329477756124106235").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(67.5)).unwrap();
	let expected = S::from_str("3478771.29657931373511895582303355503689306755879040538475929").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(68.0)).unwrap();
	let expected = S::from_str("3889384.54866321356696504003361257765036890760545072945818819788424356177127211465073996551445357101059810411847195155132794").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(68.5)).unwrap();
	let expected = S::from_str("4348464.12072414216889869477879194379611633444848800673094911").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(69.0)).unwrap();
	let expected = S::from_str("4861730.68582901695870630004201572206296113450681341182273524735530445221409014331342495689306696376324763014808993943915993").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(69.5)).unwrap();
	let expected = S::from_str("5435580.15090517771112336847348992974514541806061000841368639").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(70.0)).unwrap();
	let expected = S::from_str("6077163.35728627119838287505251965257870141813351676477841905919413056526761267914178119611633370470405953768511242429894991").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(70.5)).unwrap();
	let expected = S::from_str("6794475.18863147213890421059186241218143177257576251051710798").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(71.0)).unwrap();
	let expected = S::from_str("7596454.19660783899797859381564956572337677266689595597302382399266320658451584892722649514541713088007442210639053037368739").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(71.5)).unwrap();
	let expected = S::from_str("8493093.98578934017363026323982801522678971571970313814638498").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(72.0)).unwrap();
	let expected = S::from_str("9495567.74575979874747324226956195715422096583361994496627977999082900823064481115903311893177141360009302763298816296710924").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(72.5)).unwrap();
	let expected = S::from_str("10616367.4822366752170378290497850190334871446496289226829812").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(73.0)).unwrap();
	let expected = S::from_str("11869459.6821997484343415528369524464427762072920249312078497249885362602883060139487913986647142670001162845412352037088865").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(73.5)).unwrap();
	let expected = S::from_str("13270459.3527958440212972863122312737918589308120361533537265").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(74.0)).unwrap();
	let expected = S::from_str("14836824.6027496855429269410461905580534702591150311640098121562356703253603825174359892483308928337501453556765440046361081").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(74.5)).unwrap();
	let expected = S::from_str("16588074.1909948050266216078902890922398236635150451916921581").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(75.0)).unwrap();
	let expected = S::from_str("18546030.7534371069286586763077381975668378238937889550122651952945879067004781467949865604136160421876816945956800057951352").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(75.5)).unwrap();
	let expected = S::from_str("20735092.7387435062832770098628613652997795793938064896151977").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(76.0)).unwrap();
	let expected = S::from_str("23182538.4417963836608233453846727469585472798672361937653314941182348833755976834937332005170200527346021182446000072439190").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(76.5)).unwrap();
	let expected = S::from_str("25918865.9234293828540962623285767066247244742422581120189971").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(77.0)).unwrap();
	let expected = S::from_str("28978173.0522454795760291817308409336981840998340452422066643676477936042194971043671665006462750659182526478057500090548988").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(77.5)).unwrap();
	let expected = S::from_str("32398582.4042867285676203279107208832809055928028226400237464").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(78.0)).unwrap();
	let expected = S::from_str("36222716.3153068494700364771635511671227301247925565527583304595597420052743713804589581258078438323978158097571875113186235").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(78.5)).unwrap();
	let expected = S::from_str("40498228.0053584107095254098884011041011319910035283000296830").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(79.0)).unwrap();
	let expected = S::from_str("45278395.3941335618375455964544389589034126559906956909479130744496775065929642255736976572598047904972697621964843891482794").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(79.5)).unwrap();
	let expected = S::from_str("50622785.0066980133869067623605013801264149887544103750371038").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(80.0)).unwrap();
	let expected = S::from_str("56597994.2426669522969319955680486986292658199883696136848913430620968832412052819671220715747559881215872027456054864353492").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(80.5)).unwrap();
	let expected = S::from_str("63278481.2583725167336334529506267251580187359430129687963797").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(81.0)).unwrap();
	let expected = S::from_str("70747492.8033336903711649944600608732865822749854620171061141788276211040515066024589025894684449851519840034320068580441865").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(81.5)).unwrap();
	let expected = S::from_str("79098101.5729656459170418161882834064475234199287662109954746").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(82.0)).unwrap();
	let expected = S::from_str("88434366.0041671129639562430750760916082278437318275213826427235345263800643832530736282368355562314399800042900085725552332").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(82.5)).unwrap();
	let expected = S::from_str("98872626.9662070573963022702353542580594042749109577637443433").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(83.0)).unwrap();
	let expected = S::from_str("110542957.505208891204945303843845114510284804664784401728303404418157975080479066342035296044445289299975005362510715694041").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(83.5)).unwrap();
	let expected = S::from_str("123590783.707758821745377837794192822574255343638697204680429").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(84.0)).unwrap();
	let expected = S::from_str("138178696.881511114006181629804806393137856005830980502160379255522697468850598832927544120055556611624968756703138394617551").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(84.5)).unwrap();
	let expected = S::from_str("154488479.634698527181722297242741028217819179548371505850536").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(85.0)).unwrap();
	let expected = S::from_str("172723371.101888892507727037256007991422320007288725627700474069403371836063248541159430150069445764531210945878922993271939").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(85.5)).unwrap();
	let expected = S::from_str("193110599.543373158977152871553426285272273974435464382313170").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(86.0)).unwrap();
	let expected = S::from_str("215904213.877361115634658796570009989277900009110907034625592586754214795079060676449287687586807205664013682348653741589924").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(86.5)).unwrap();
	let expected = S::from_str("241388249.429216448721441089441782856590342468044330477891463").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(87.0)).unwrap();
	let expected = S::from_str("269880267.346701394543323495712512486597375011388633793281990733442768493848825845561609609483509007080017102935817176987406").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(87.5)).unwrap();
	let expected = S::from_str("301735311.786520560901801361802228570737928085055413097364329").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(88.0)).unwrap();
	let expected = S::from_str("337350334.183376743179154369640640608246718764235792241602488416803460617311032306952012011854386258850021378669771471234257").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(88.5)).unwrap();
	let expected = S::from_str("377169139.733150701127251702252785713422410106319266371705411").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(89.0)).unwrap();
	let expected = S::from_str("421687917.729220928973942962050800760308398455294740302003110521004325771638790383690015014817982823562526723337214339042822").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(89.5)).unwrap();
	let expected = S::from_str("471461424.666438376409064627815982141778012632899082964631764").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(90.0)).unwrap();
	let expected = S::from_str("527109897.161526161217428702563500950385498069118425377503888151255407214548487979612518768522478529453158404171517923803527").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(90.5)).unwrap();
	let expected = S::from_str("589326780.833047970511330784769977677222515791123853705789705").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(91.0)).unwrap();
	let expected = S::from_str("658887371.451907701521785878204376187981872586398031721879860189069259018185609974515648460653098161816448005214397404754409").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(91.5)).unwrap();
	let expected = S::from_str("736658476.041309963139163480962472096528144738904817132237131").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(92.0)).unwrap();
	let expected = S::from_str("823609214.314884626902232347755470234977340732997539652349825236336573772732012468144560575816372702270560006517996755943011").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(92.5)).unwrap();
	let expected = S::from_str("920823095.051637453923954351203090120660180923631021415296414").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(93.0)).unwrap();
	let expected = S::from_str("1029511517.89360578362779043469433779372167591624692456543728154542071721591501558518070071977046587783820000814749594492876").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(93.5)).unwrap();
	let expected = S::from_str("1151028868.81454681740494293900386265082522615453877676912051").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(94.0)).unwrap();
	let expected = S::from_str("1286889397.36700722953473804336792224215209489530865570679660193177589651989376948147587589971308234729775001018436993116095").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(94.5)).unwrap();
	let expected = S::from_str("1438786086.01818352175617867375482831353153269317347096140064").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(95.0)).unwrap();
	let expected = S::from_str("1608611746.70875903691842255420990280269011861913581963349575241471987064986721185184484487464135293412218751273046241395119").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(95.5)).unwrap();
	let expected = S::from_str("1798482607.52272940219522334219353539191441586646683870175080").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(96.0)).unwrap();
	let expected = S::from_str("2010764683.38594879614802819276237850336264827391977454186969051839983831233401481480605609330169116765273439091307801743899").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(96.5)).unwrap();
	let expected = S::from_str("2248103259.40341175274402917774191923989301983308354837718851").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	/*
	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(97.0)).unwrap();
	let expected = S::from_str("2513455854.23243599518503524095297312920331034239971817733711314799979789041751851850757011662711395956591798864134752179874").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(97.5)).unwrap();
	let expected = S::from_str("2810129074.25426469093003647217739904986627479135443547148564").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(98.0)).unwrap();
	let expected = S::from_str("3141819817.79054499398129405119121641150413792799964772167139143499974736302189814813446264578389244945739748580168440224842").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(98.5)).unwrap();
	let expected = S::from_str("3512661342.81783086366254559022174881233284348919304433935705").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(99.0)).unwrap();
	let expected = S::from_str("3927274772.23818124247661756398902051438017240999955965208923929374968420377737268516807830722986556182174685725210550281053").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.25), S::from_num(99.5)).unwrap();
	let expected = S::from_str("4390826678.52228857957818198777718601541605436149130542419631").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	*/

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(0.01)).unwrap();
	let expected = S::from_str("1.0040628822999231097921678262939853106034341255439779432223661978").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(0.1)).unwrap();
	let expected = S::from_str("1.0413797439924105868461910102311153381211443341764803061983768766").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(0.5)).unwrap();
	let expected = S::from_str("1.2247448713915890490986420373529456959829737403283350642163462836").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(0.75)).unwrap();
	let expected = S::from_str("1.3554030054147672479433270793371662853330255609047152735564844293").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(1.0)).unwrap();
	let expected = S::from_str("1.5").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(1.5)).unwrap();
	let expected = S::from_str("1.8371173070873835736479630560294185439744606104925025963245194254").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(2.0)).unwrap();
	let expected = S::from_str("2.25").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(2.5)).unwrap();
	let expected = S::from_str("2.7556759606310753604719445840441278159616909157387538944867791381").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(3.0)).unwrap();
	let expected = S::from_str("3.375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(3.5)).unwrap();
	let expected = S::from_str("4.1335139409466130407079168760661917239425363736081308417301687072").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(4.0)).unwrap();
	let expected = S::from_str("5.0625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(4.5)).unwrap();
	let expected = S::from_str("6.2002709114199195610618753140992875859138045604121962625952530608").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(5.0)).unwrap();
	let expected = S::from_str("7.59375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(5.5)).unwrap();
	let expected = S::from_str("9.3004063671298793415928129711489313788707068406182943938928795912").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(6.0)).unwrap();
	let expected = S::from_str("11.390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(6.5)).unwrap();
	let expected = S::from_str("13.950609550694819012389219456723397068306060260927441590839319386").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(7.0)).unwrap();
	let expected = S::from_str("17.0859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(7.5)).unwrap();
	let expected = S::from_str("20.925914326042228518583829185085095602459090391391162386258979080").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(8.0)).unwrap();
	let expected = S::from_str("25.62890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(8.5)).unwrap();
	let expected = S::from_str("31.388871489063342777875743777627643403688635587086743579388468620").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(9.0)).unwrap();
	let expected = S::from_str("38.443359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(9.5)).unwrap();
	let expected = S::from_str("47.083307233595014166813615666441465105532953380630115369082702930").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(10.0)).unwrap();
	let expected = S::from_str("57.6650390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(10.5)).unwrap();
	let expected = S::from_str("70.624960850392521250220423499662197658299430070945173053624054396").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(11.0)).unwrap();
	let expected = S::from_str("86.49755859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(11.5)).unwrap();
	let expected = S::from_str("105.93744127558878187533063524949329648744914510641775958043608159").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(12.0)).unwrap();
	let expected = S::from_str("129.746337890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(12.5)).unwrap();
	let expected = S::from_str("158.90616191338317281299595287423994473117371765962663937065412239").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(13.0)).unwrap();
	let expected = S::from_str("194.6195068359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(13.5)).unwrap();
	let expected = S::from_str("238.35924287007475921949392931135991709676057648943995905598118358").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(14.0)).unwrap();
	let expected = S::from_str("291.92926025390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(14.5)).unwrap();
	let expected = S::from_str("357.53886430511213882924089396703987564514086473415993858397177538").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(15.0)).unwrap();
	let expected = S::from_str("437.893890380859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(15.5)).unwrap();
	let expected = S::from_str("536.30829645766820824386134095055981346771129710123990787595766307").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(16.0)).unwrap();
	let expected = S::from_str("656.8408355712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(16.5)).unwrap();
	let expected = S::from_str("804.46244468650231236579201142583972020156694565185986181393649460").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(17.0)).unwrap();
	let expected = S::from_str("985.26125335693359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(17.5)).unwrap();
	let expected = S::from_str("1206.6936670297534685486880171387595803023504184777897927209047419").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(18.0)).unwrap();
	let expected = S::from_str("1477.891880035400390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(18.5)).unwrap();
	let expected = S::from_str("1810.0405005446302028230320257081393704535256277166846890813571128").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(19.0)).unwrap();
	let expected = S::from_str("2216.8378200531005859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(19.5)).unwrap();
	let expected = S::from_str("2715.0607508169453042345480385622090556802884415750270336220356693").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(20.0)).unwrap();
	let expected = S::from_str("3325.25673007965087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(20.5)).unwrap();
	let expected = S::from_str("4072.5911262254179563518220578433135835204326623625405504330535039").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(21.0)).unwrap();
	let expected = S::from_str("4987.885095119476318359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(21.5)).unwrap();
	let expected = S::from_str("6108.8866893381269345277330867649703752806489935438108256495802559").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(22.0)).unwrap();
	let expected = S::from_str("7481.8276426792144775390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(22.5)).unwrap();
	let expected = S::from_str("9163.3300340071904017915996301474555629209734903157162384743703838").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(23.0)).unwrap();
	let expected = S::from_str("11222.74146401882171630859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(23.5)).unwrap();
	let expected = S::from_str("13744.995051010785602687399445221183344381460235473574357711555575").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(24.0)).unwrap();
	let expected = S::from_str("16834.112196028232574462890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(24.5)).unwrap();
	let expected = S::from_str("20617.492576516178404031099167831775016572190353210361536567333363").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(25.0)).unwrap();
	let expected = S::from_str("25251.1682940423488616943359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(25.5)).unwrap();
	let expected = S::from_str("30926.238864774267606046648751747662524858285529815542304851000045").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(26.0)).unwrap();
	let expected = S::from_str("37876.75244106352329254150390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(26.5)).unwrap();
	let expected = S::from_str("46389.358297161401409069973127621493787287428294723313457276500068").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(27.0)).unwrap();
	let expected = S::from_str("56815.128661595284938812255859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(27.5)).unwrap();
	let expected = S::from_str("69584.037445742102113604959691432240680931142442084970185914750102").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(28.0)).unwrap();
	let expected = S::from_str("85222.6929923929274082183837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(28.5)).unwrap();
	let expected = S::from_str("104376.05616861315317040743953714836102139671366312745527887212515").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(29.0)).unwrap();
	let expected = S::from_str("127834.03948858939111232757568359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(29.5)).unwrap();
	let expected = S::from_str("156564.08425291972975561115930572254153209507049469118291830818773").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(30.0)).unwrap();
	let expected = S::from_str("191751.059232884086668491363525390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(30.5)).unwrap();
	let expected = S::from_str("234846.12637937959463341673895858381229814260574203677437746228159").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(31.0)).unwrap();
	let expected = S::from_str("287626.5888493261300027370452880859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(31.5)).unwrap();
	let expected = S::from_str("352269.18956906939195012510843787571844721390861305516156619342239").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(32.0)).unwrap();
	let expected = S::from_str("431439.88327398919500410556793212890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(32.5)).unwrap();
	let expected = S::from_str("528403.78435360408792518766265681357767082086291958274234929013359").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(33.0)).unwrap();
	let expected = S::from_str("647159.824910983792506158351898193359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(33.5)).unwrap();
	let expected = S::from_str("792605.67653040613188778149398522036650623129437937411352393520038").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(34.0)).unwrap();
	let expected = S::from_str("970739.7373664756887592375278472900390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(34.5)).unwrap();
	let expected = S::from_str("1188908.51479560919783167224097783054975934694156906117028590").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(35.0)).unwrap();
	let expected = S::from_str("1456109.60604971353313885629177093505859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(35.5)).unwrap();
	let expected = S::from_str("1783362.77219341379674750836146674582463902041235359175542885").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(36.0)).unwrap();
	let expected = S::from_str("2184164.409074570299708284437656402587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(36.5)).unwrap();
	let expected = S::from_str("2675044.15829012069512126254220011873695853061853038763314328").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(37.0)).unwrap();
	let expected = S::from_str("3276246.6136118554495624266564846038818359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(37.5)).unwrap();
	let expected = S::from_str("4012566.23743518104268189381330017810543779592779558144971492").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(38.0)).unwrap();
	let expected = S::from_str("4914369.92041778317434363998472690582275390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(38.5)).unwrap();
	let expected = S::from_str("6018849.35615277156402284071995026715815669389169337217457238").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(39.0)).unwrap();
	let expected = S::from_str("7371554.880626674761515459977090358734130859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(39.5)).unwrap();
	let expected = S::from_str("9028274.03422915734603426107992540073723504083754005826185857").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(40.0)).unwrap();
	let expected = S::from_str("11057332.3209400121422731899656355381011962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(40.5)).unwrap();
	let expected = S::from_str("13542411.0513437360190513916198881011058525612563100873927878").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(41.0)).unwrap();
	let expected = S::from_str("16585998.48141001821340978494845330715179443359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(41.5)).unwrap();
	let expected = S::from_str("20313616.5770156040285770874298321516587788418844651310891817").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(42.0)).unwrap();
	let expected = S::from_str("24878997.722115027320114677422679960727691650390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(42.5)).unwrap();
	let expected = S::from_str("30470424.8655234060428656311447482274881682628266976966337726").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(43.0)).unwrap();
	let expected = S::from_str("37318496.5831725409801720161340199410915374755859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(43.5)).unwrap();
	let expected = S::from_str("45705637.2982851090642984467171223412322523942400465449506590").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(44.0)).unwrap();
	let expected = S::from_str("55977744.87475881147025802420102991163730621337890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(44.5)).unwrap();
	let expected = S::from_str("68558455.9474276635964476700756835118483785913600698174259885").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(45.0)).unwrap();
	let expected = S::from_str("83966617.312138217205387036301544867455959320068359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(45.5)).unwrap();
	let expected = S::from_str("102837683.921141495394671505113525267772567887040104726138982").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(46.0)).unwrap();
	let expected = S::from_str("125949925.9682073258080805544523173011839389801025390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(46.5)).unwrap();
	let expected = S::from_str("154256525.881712243092007257670287901658851830560157089208474").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(47.0)).unwrap();
	let expected = S::from_str("188924888.95231098871212083167847595177590847015380859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(47.5)).unwrap();
	let expected = S::from_str("231384788.822568364638010886505431852488277745840235633812711").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(48.0)).unwrap();
	let expected = S::from_str("283387333.428466483068181247517713927663862705230712890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(48.5)).unwrap();
	let expected = S::from_str("347077183.233852546957016329758147778732416618760353450719067").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(49.0)).unwrap();
	let expected = S::from_str("425081000.1426997246022718712765708914957940578460693359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(49.5)).unwrap();
	let expected = S::from_str("520615774.850778820435524494637221668098624928140530176078600").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(50.0)).unwrap();
	let expected = S::from_str("637621500.21404958690340780691485633724369108676910400390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(50.5)).unwrap();
	let expected = S::from_str("780923662.276168230653286741955832502147937392210795264117900").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(51.0)).unwrap();
	let expected = S::from_str("956432250.321074380355111710372284505865536630153656005859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(51.5)).unwrap();
	let expected = S::from_str("1171385493.41425234597993011293374875322190608831619289617685").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(52.0)).unwrap();
	let expected = S::from_str("1434648375.4816115705326675655584267587983049452304840087890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(52.5)).unwrap();
	let expected = S::from_str("1757078240.12137851896989516940062312983285913247428934426527").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(53.0)).unwrap();
	let expected = S::from_str("2151972563.22241735579900134833764013819745741784572601318359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	/*
	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(53.5)).unwrap();
	let expected = S::from_str("2635617360.18206777845484275410093469474928869871143401639791").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(54.0)).unwrap();
	let expected = S::from_str("3227958844.833626033698502022506460207296186126768589019775390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(54.5)).unwrap();
	let expected = S::from_str("3953426040.27310166768226413115140204212393304806715102459687").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(55.0)).unwrap();
	let expected = S::from_str("4841938267.2504390505477530337596903109442791901528835296630859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(55.5)).unwrap();
	let expected = S::from_str("5930139060.40965250152339619672710306318589957210072653689530").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(56.0)).unwrap();
	let expected = S::from_str("7262907400.87565857582162955063953546641641878522932529449462890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(56.5)).unwrap();
	let expected = S::from_str("8895208590.61447875228509429509065459477884935815108980534296").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(57.0)).unwrap();
	let expected = S::from_str("10894361101.313487863732444325959303199624628177843987941741943359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(57.5)).unwrap();
	let expected = S::from_str("13342812885.9217181284276414426359818921682740372266347080144").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(58.0)).unwrap();
	let expected = S::from_str("16341541651.9702317955986664889389547994369422667659819126129150390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(58.5)).unwrap();
	let expected = S::from_str("20014219328.8825771926414621639539728382524110558399520620216").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(59.0)).unwrap();
	let expected = S::from_str("24512312477.95534769339799973340843219915541340014897286891937255859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(59.5)).unwrap();
	let expected = S::from_str("30021328993.3238657889621932459309592573786165837599280930325").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(60.0)).unwrap();
	let expected = S::from_str("36768468716.933021540096999600112648298733120100223459303379058837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(60.5)).unwrap();
	let expected = S::from_str("45031993489.9857986834432898688964388860679248756398921395487").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(61.0)).unwrap();
	let expected = S::from_str("55152703075.3995323101454994001689724480996801503351889550685882568359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(61.5)).unwrap();
	let expected = S::from_str("67547990234.9786980251649348033446583291018873134598382093231").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(62.0)).unwrap();
	let expected = S::from_str("82729054613.09929846521824910025345867214952022550278343260288238525390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(62.5)).unwrap();
	let expected = S::from_str("101321985352.468047037747402205016987493652830970189757313984").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(63.0)).unwrap();
	let expected = S::from_str("124093581919.648947697827373650380188008224280338254175148904323577880859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(63.5)).unwrap();
	let expected = S::from_str("151982978028.702070556621103307525481240479246455284635970977").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(64.0)).unwrap();
	let expected =
		S::from_str("186140372879.4734215467410604755702820123364205073812627233564853668212890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(64.5)).unwrap();
	let expected = S::from_str("227974467043.053105834931654961288221860718869682926953956465").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(65.0)).unwrap();
	let expected =
		S::from_str("279210559319.21013232011159071335542301850463076107189408503472805023193359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(65.5)).unwrap();
	let expected = S::from_str("341961700564.579658752397482441932332791078304524390430934698").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(66.0)).unwrap();
	let expected =
		S::from_str("418815838978.815198480167386070033134527756946141607841127552092075347900390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(66.5)).unwrap();
	let expected = S::from_str("512942550846.869488128596223662898499186617456786585646402047").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(67.0)).unwrap();
	let expected =
		S::from_str("628223758468.2227977202510791050497017916354192124117616913281381130218505859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(67.5)).unwrap();
	let expected = S::from_str("769413826270.304232192894335494347748779926185179878469603071").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(68.0)).unwrap();
	let expected =
		S::from_str("942335637702.33419658037661865757455268745312881861764253699220716953277587890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(68.5)).unwrap();
	let expected = S::from_str("1154120739405.45634828934150324152162316988927776981770440460").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(69.0)).unwrap();
	let expected =
		S::from_str("1413503456553.501294870564927986361829031179693227926463805488310754299163818359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(69.5)).unwrap();
	let expected = S::from_str("1731181109108.18452243401225486228243475483391665472655660691").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(70.0)).unwrap();
	let expected =
		S::from_str("2120255184830.2519423058473919795427435467695398418896957082324661314487457275390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(70.5)).unwrap();
	let expected = S::from_str("2596771663662.27678365101838229342365213225087498208983491036").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(71.0)).unwrap();
	let expected =
		S::from_str("3180382777245.37791345877108796931411532015430976283454356234869919717311859130859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(71.5)).unwrap();
	let expected = S::from_str("3895157495493.41517547652757344013547819837631247313475236554").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(72.0)).unwrap();
	let expected =
		S::from_str("4770574165868.066870188156631953971172980231464644251815343523048795759677886962890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(72.5)).unwrap();
	let expected = S::from_str("5842736243240.12276321479136016020321729756446870970212854832").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(73.0)).unwrap();
	let expected =
		S::from_str("7155861248802.1003052822349479309567594703471969663777230152845731936395168304443359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(73.5)).unwrap();
	let expected = S::from_str("8764104364860.18414482218704024030482594634670306455319282248").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(74.0)).unwrap();
	let expected =
		S::from_str("10733791873203.15045792335242189643513920552079544956658452292685979045927524566650390625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(74.5)).unwrap();
	let expected = S::from_str("13146156547290.2762172332805603604572389195200545968297892337").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(75.0)).unwrap();
	let expected =
		S::from_str("16100687809804.725686885028632844652708808281193174349876784390289685688912868499755859375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(75.5)).unwrap();
	let expected = S::from_str("19719234820935.4143258499208405406858583792800818952446838505").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(76.0)).unwrap();
	let expected =
		S::from_str("24151031714707.0885303275429492669790632124217897615248151765854345285333693027496337890625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(76.5)).unwrap();
	let expected = S::from_str("29578852231403.1214887748812608110287875689201228428670257758").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(77.0)).unwrap();
	let expected =
		S::from_str("36226547572060.63279549131442390046859481863268464228722276487815179280005395412445068359375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(77.5)).unwrap();
	let expected = S::from_str("44368278347104.6822331623218912165431813533801842643005386638").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(78.0)).unwrap();
	let expected =
		S::from_str("54339821358090.949193236971635850702892227949026963430834147317227689200080931186676025390625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(78.5)).unwrap();
	let expected = S::from_str("66552417520657.0233497434828368248147720300702763964508079957").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(79.0)).unwrap();
	let expected =
		S::from_str("81509732037136.4237898554574537760543383419235404451462512209758415338001213967800140380859375")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(79.5)).unwrap();
	let expected = S::from_str("99828626280985.5350246152242552372221580451054145946762119936").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(80.0)).unwrap();
	let expected =
		S::from_str("122264598055704.63568478318618066408150751288531066771937683146376230070018209517002105712890625")
			.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(80.5)).unwrap();
	let expected = S::from_str("149742939421478.302536922836382855833237067658121892014317990").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(81.0)).unwrap();
	let expected = S::from_str(
		"183396897083556.953527174779270996122261269327966001579065247195643451050273142755031585693359375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(81.5)).unwrap();
	let expected = S::from_str("224614409132217.453805384254574283749855601487182838021476985").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(82.0)).unwrap();
	let expected = S::from_str(
		"275095345625335.4302907621689064941833919039919490023685978707934651765754097141325473785400390625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(82.5)).unwrap();
	let expected = S::from_str("336921613698326.180708076381861425624783402230774257032215478").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(83.0)).unwrap();
	let expected = S::from_str(
		"412643018438003.14543614325335974127508785598792350355289680619019776486311457119882106781005859375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(83.5)).unwrap();
	let expected = S::from_str("505382420547489.271062114572792138437175103346161385548323217").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(84.0)).unwrap();
	let expected = S::from_str(
		"618964527657004.718154214880039611912631783981885255329345209285296647294671856798231601715087890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(84.5)).unwrap();
	let expected = S::from_str("758073630821233.906593171859188207655762655019242078322484826").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(85.0)).unwrap();
	let expected = S::from_str(
		"928446791485507.0772313223200594178689476759728278829940178139279449709420077851973474025726318359375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(85.5)).unwrap();
	let expected = S::from_str("1137110446231850.85988975778878231148364398252886311748372723").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(86.0)).unwrap();
	let expected = S::from_str(
		"1392670187228260.61584698348008912680342151395924182449102672089191745641301167779602110385894775390625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(86.5)).unwrap();
	let expected = S::from_str("1705665669347776.28983463668317346722546597379329467622559085").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(87.0)).unwrap();
	let expected = S::from_str(
		"2089005280842390.923770475220133690205132270938862736736540081337876184619517516694031655788421630859375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(87.5)).unwrap();
	let expected = S::from_str("2558498504021664.43475195502476020083819896068994201433838628").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(88.0)).unwrap();
	let expected = S::from_str(
		"3133507921263586.3856557128302005353076984064082941051048101220068142769292762750410474836826324462890625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(88.5)).unwrap();
	let expected = S::from_str("3837747756032496.65212793253714030125729844103491302150757943").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(89.0)).unwrap();
	let expected = S::from_str(
		"4700261881895379.57848356924530080296154760961244115765721518301022141539391441256157122552394866943359375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(89.5)).unwrap();
	let expected = S::from_str("5756621634048744.97819189880571045188594766155236953226136915").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(90.0)).unwrap();
	let expected = S::from_str(
		"7050392822843069.367725353867951204442321414418661736485822774515332123090871618842356838285923004150390625",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(90.5)).unwrap();
	let expected = S::from_str("8634932451073117.46728784820856567782892149232855429839205372").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(91.0)).unwrap();
	let expected = S::from_str(
		"10575589234264604.0515880308019268066634821216279926047287341617729981846363074282635352574288845062255859375",
	)
	.unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(91.5)).unwrap();
	let expected = S::from_str("12952398676609676.2009317723128485167433822384928314475880805").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(92.0)).unwrap();
	let expected = S::from_str("15863383851396906.07738204620289020999522318244198890709310124265949727695446114239530288614332675933837890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(92.5)).unwrap();
	let expected = S::from_str("19428598014914514.3013976584692727751150733577392471713821208").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(93.0)).unwrap();
	let expected = S::from_str("23795075777095359.116073069304335314992834773662983360639651863989245915431691713592954329214990139007568359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(93.5)).unwrap();
	let expected = S::from_str("29142897022371771.4520964877039091626726100366088707570731813").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(94.0)).unwrap();
	let expected = S::from_str("35692613665643038.6741096039565029724892521604944750409594777959838688731475375703894314938224852085113525390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(94.5)).unwrap();
	let expected = S::from_str("43714345533557657.1781447315558637440089150549133061356097719").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(95.0)).unwrap();
	let expected = S::from_str("53538920498464558.01116440593475445873387824074171256143921669397580330972130635558414724073372781276702880859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(95.5)).unwrap();
	let expected = S::from_str("65571518300336485.7672170973337956160133725823699592034146579").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(96.0)).unwrap();
	let expected = S::from_str("80308380747696837.016746608902131688100817361112568842158825040963704964581959533376220861100591719150543212890625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(96.5)).unwrap();
	let expected = S::from_str("98357277450504728.6508256460006934240200588735549388051219869").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(97.0)).unwrap();
	let expected = S::from_str("120462571121545255.5251199133531975321512260416688532632382375614455574468729393000643312916508875787258148193359375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(97.5)).unwrap();
	let expected = S::from_str("147535916175757092.976238469001040136030088310332408207682980").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(98.0)).unwrap();
	let expected = S::from_str("180693856682317883.28767987002979629822683906250327989485735634216833617030940895009649693747633136808872222900390625").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(98.5)).unwrap();
	let expected = S::from_str("221303874263635639.464357703501560204045132465498612311524470").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(99.0)).unwrap();
	let expected = S::from_str("271040785023476824.931519805044694447340258593754919842286034513252504255464113425144745406214497052133083343505859375").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	let result: D = pow::<S, D>(S::from_num(1.5), S::from_num(99.5)).unwrap();
	let expected = S::from_str("331955811395453459.196536555252340306067698698247918467286706").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	*/

	let result: D = pow::<S, D>(S::from_num(2.0), S::from_num(31.0)).unwrap();
	let expected = S::from_str("2147483648").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));

	/*
	let result: D = pow::<S, D>(S::from_num(2.0), S::from_num(32.0)).unwrap();
	let expected = S::from_str("4294967296").unwrap();
	assert!(ensure_accuracy(result, expected, tolerance));
	*/
}
