pub trait ICESolver<Intent> {
	type Solution;
	type Error;

	fn solve(intents: Vec<Intent>) -> Result<Self::Solution, Self::Error>;
}
