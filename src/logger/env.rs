#[macro_export]
macro_rules! service_name {
    () => {
        env!("CARGO_BIN_NAME")
    };
}

/// The deployment environment (`APP_ENV`, `development` when unset), read once: request logs
/// carry it on every line.
pub fn app_env() -> &'static str {
    static APP_ENV: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    APP_ENV.get_or_init(|| get_env_var("APP_ENV", "development"))
}

/// Retrieve an environment variable or return a default value if not set.
pub fn get_env_var(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
