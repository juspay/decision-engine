pub trait Encode<I, O> {
    type ReturnType<T>;
    fn encode(&self, input: I) -> Self::ReturnType<O>;
}
