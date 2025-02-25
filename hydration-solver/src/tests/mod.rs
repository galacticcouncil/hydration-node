extern crate rand;
mod v3;
use crate::types::Intent;
use crate::types::{Asset, AssetId, OmnipoolAsset};
use rand::Rng;

const DATA: &str = r##"[{"Omnipool":{"asset_id":100,"reserve":1270943150463997444837375,"hub_reserve":50853671234155699,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":0,"reserve":83592523593731567499,"hub_reserve":40853940161804370,"decimals":12,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":28,"reserve":4444581557775981481383,"hub_reserve":9867914236001391,"decimals":15,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":20,"reserve":839402843839653672974,"hub_reserve":87617024245417446,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":101,"reserve":629572314354454914,"hub_reserve":2197799364666895,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":16,"reserve":13830709039242499338536071,"hub_reserve":65517044346108249,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":11,"reserve":984121260,"hub_reserve":36404599848875067,"decimals":8,"fee":4166,"hub_fee":500}},{"Omnipool":{"asset_id":14,"reserve":6557500346217376443,"hub_reserve":43555962781559649,"decimals":12,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":19,"reserve":960749125,"hub_reserve":35480714305769130,"decimals":8,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":31,"reserve":31045853803931756127987667,"hub_reserve":1957518273282525,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":33,"reserve":7002428183209846203807494,"hub_reserve":13152800813262614,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":15,"reserve":9077381717604021,"hub_reserve":245563870516993185,"decimals":10,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":13,"reserve":5303765501109563123458773,"hub_reserve":35368913823769879,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":27,"reserve":415903156101558577,"hub_reserve":2993841030740697,"decimals":12,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":102,"reserve":14890614296802581841121227,"hub_reserve":583613992958006756,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":5,"reserve":28257160599310227,"hub_reserve":520096306936030392,"decimals":10,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":1000624,"reserve":895399907425460194070,"hub_reserve":8229794081405306,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":8,"reserve":4956589123156450856,"hub_reserve":29923412066769426,"decimals":12,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":1000765,"reserve":10345100937330576668,"hub_reserve":38354389448420649,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":12,"reserve":86962201989061098,"hub_reserve":3518716760727916,"decimals":10,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":17,"reserve":687214470346292401,"hub_reserve":15195426436782832,"decimals":10,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":9,"reserve":46139630828497220817523427,"hub_reserve":72920802445903885,"decimals":18,"fee":1500,"hub_fee":500}},{"Omnipool":{"asset_id":1000752,"reserve":703013009404,"hub_reserve":4331316417449755,"decimals":9,"fee":1500,"hub_fee":500}}]"##;

pub(crate) fn load_amm_state() -> Vec<Asset> {
	serde_json::from_str(DATA).unwrap()
}

#[test]
fn test_data() {
	let d = load_amm_state();
	assert_eq!(d.len(), 23);
}
