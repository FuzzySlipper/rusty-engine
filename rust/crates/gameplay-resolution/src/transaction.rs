pub trait ResolutionTransaction {
    type Effect;
    type Error;

    /// Stage one already-validated downstream effect without mutating authority.
    fn stage(&mut self, effect: &Self::Effect) -> Result<(), Self::Error>;

    /// Publish all staged effects once, or return an error without mutation.
    fn commit(&mut self) -> Result<(), Self::Error>;

    /// Discard all staging. This is called for preview and transaction failure.
    fn abort(&mut self);
}
