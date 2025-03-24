pub trait Hashing {
    type Key;
    type Input;
    type Output;

    fn generate(input: Self::Input, key: &Self::Key) -> Self::Output;

    #[allow(dead_code)]
    fn verify(input: Self::Input, key: &Self::Key, value: Self::Output) -> bool;
}

pub enum Blake3 {}

impl Hashing for Blake3 {
    type Key = [u8; 32];
    type Input = String;
    type Output = String;

    fn generate(input: Self::Input, key: &Self::Key) -> Self::Output {
        blake3::keyed_hash(key, input.as_bytes())
            .to_hex()
            .to_string()
    }

    fn verify(input: Self::Input, key: &Self::Key, value: Self::Output) -> bool {
        let hash = blake3::keyed_hash(key, input.as_bytes())
            .to_hex()
            .to_string();
        hash == value
    }
}

pub mod time {
    pub fn now() -> time::PrimitiveDateTime {
        let utc_date_time = time::OffsetDateTime::now_utc();
        time::PrimitiveDateTime::new(utc_date_time.date(), utc_date_time.time())
    }
}
