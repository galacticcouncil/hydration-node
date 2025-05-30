use frame_system::Config;
use pretty_assertions::assert_eq;

/// Check if an event of type `TEvent` is present anywhere in system events.
pub fn expect_event<TEvent: std::fmt::Debug + PartialEq, TRuntime: Config>(e: TEvent)
where
	Vec<TEvent>: FromIterator<<TRuntime as Config>::RuntimeEvent>,
{
	// Dev note: 1000 is an arbitrary number, it should be large enough to cover the number of events
	assert!(
		last_events::<TEvent, TRuntime>(1000).contains(&e),
		"Expected event {:?} not found in the system events",
		e
	);
}

/// Check if given list of event matches the last `n` events in the system.
pub fn expect_events<TEvent: std::fmt::Debug + PartialEq, TRuntime: Config>(e: Vec<TEvent>)
where
	Vec<TEvent>: FromIterator<<TRuntime as Config>::RuntimeEvent>,
{
	let last_events: Vec<TEvent> = last_events::<TEvent, TRuntime>(e.len());
	assert_eq!(last_events, e);
}

pub fn last_events<TEvent: std::fmt::Debug, TRuntime>(n: usize) -> Vec<TEvent>
where
	TRuntime: Config,
	Vec<TEvent>: FromIterator<<TRuntime as Config>::RuntimeEvent>,
{
	frame_system::Pallet::<TRuntime>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

#[macro_export]
macro_rules! assert_eq_approx {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\nleft: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

#[macro_export]
macro_rules! assert_balance {
	($who:expr, $asset_id:expr,  $expected_balance:expr) => {{
		assert_eq!(Tokens::free_balance($asset_id, &$who), $expected_balance);
	}};
}

#[macro_export]
macro_rules! assert_balance_approx {
	( $who:expr, $asset:expr, $expected_balance:expr, $delta:expr) => {{
		let balance = Tokens::free_balance($asset, &$who);

		let diff = if balance >= $expected_balance {
			balance - $expected_balance
		} else {
			$expected_balance - balance
		};
		if diff > $delta {
			panic!(
				"\n{} not equal\nleft: {:?}\nright: {:?}\n",
				"The balances are not equal", balance, $expected_balance
			);
		}
	}};
}

#[macro_export]
macro_rules! assert_transact_ok {
	( $call:expr) => {{
		assert_ok!(with_transaction(|| { TransactionOutcome::Commit($call) }));
	}};
}
