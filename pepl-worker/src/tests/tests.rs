use crate::*;

#[tokio::test]
async fn fetch_borrowers_listh_should_works() {
	let https = https::new();

	let url = OMNIWATCH_URL.parse().expect("OMNIWATCH_URL to be valid");
	let borrowers = fetch_borrowers_list(&https, url)
		.await
		.expect("fetch borrowers from omniwatch to work");

	assert!(borrowers.len() > 1);
}
