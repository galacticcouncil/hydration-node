use crate::*;

#[tokio::test]
async fn fetch_borrowers_listh_should_works() {
	let https = hyper_rustls::HttpsConnectorBuilder::new()
		.with_webpki_roots()
		.https_or_http()
		.enable_http1()
		.enable_http2()
		.build();

	let https_client = Client::builder().build(https);

	let url = OMNIWATCH_URL.parse().expect("OMNIWATCH_URL to be valid");
	let borrowers = fetch_borrowers_list(&https_client, url)
		.await
		.expect("omniwatch to serve borrowers list");

	assert!(borrowers.len() > 1);
}
