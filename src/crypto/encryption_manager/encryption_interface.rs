///
/// Encryption
///
/// A trait to be used internally for maintaining and managing encryption algorithms
///
pub trait Encryption<I, O> {
    type ReturnType<'b, T>
    where
        Self: 'b;
    fn encrypt(&self, input: I) -> Self::ReturnType<'_, O>;
    fn decrypt(&self, input: O) -> Self::ReturnType<'_, I>;
}
