pub trait SubmitIntent<IntentId, Intent>{
    type Error;
    fn submit_intent(intent: Intent) -> Result<IntentId, Self::Error>;
}