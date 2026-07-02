use crate::*;

// Live smoke test: hits the real omniwatch endpoint over the network, so it is
// #[ignore]d by default (flaky, and the borrower count is non-deterministic). Run it
// manually with `cargo test -p pepl-worker -- --ignored` to check omniwatch
// reachability. Deterministic failure-mode coverage lands in W3, against a local mock.
#[tokio::test]
#[ignore = "hits live omniwatch over the network; run with --ignored"]
async fn fetch_borrowers_list_should_return_borrowers_when_omniwatch_reachable() {
	let https = https::new();

	let url = OMNIWATCH_URL.parse().expect("OMNIWATCH_URL to be valid");
	let borrowers = fetch_borrowers_list(&https, url, "test")
		.await
		.expect("fetch borrowers from omniwatch to work");

	assert!(!borrowers.is_empty());
}

// W3: a down/unreachable omniwatch must return `None` (never panic). Port 1 is not listening, so
// the connection is refused immediately — deterministic, no network dependency.
#[tokio::test]
async fn fetch_borrowers_list_should_return_none_when_endpoint_unreachable() {
	let https = https::new();
	let url = "http://127.0.0.1:1/borrowers".parse().expect("valid uri");

	assert_eq!(fetch_borrowers_list(&https, url, "test").await, None);
}

// W3: on total fetch failure the retry wrapper degrades to an empty list (never panics), so the
// worker keeps running and relies on event-driven discovery.
#[tokio::test]
async fn fetch_borrowers_list_with_retry_should_return_empty_when_unreachable() {
	let https = https::new();
	let url = "http://127.0.0.1:1/borrowers".parse().expect("valid uri");

	let borrowers =
		fetch_borrowers_list_with_retry(&https, url, "test", 2, std::time::Duration::from_millis(1)).await;

	assert_eq!(borrowers, Vec::<EvmAddress>::new());
}
