/// Where to load the GSM CSV from at startup.
///
/// ```toml
/// # default — rules compiled into the binary
/// [gsm]
/// source = "bundled"
///
/// # local file — useful for mounting a volume in Docker
/// [gsm]
/// source = "file"
/// path = "config/gsm.csv"
///
/// # S3 — for sandbox / production environments
/// [gsm]
/// source = "s3"
/// bucket = "my-bucket"
/// key = "gsm/gsm.csv"
/// region = "us-east-1"   # optional; falls back to AWS_REGION env var
/// ```
#[derive(Clone, Debug, serde::Deserialize, Default)]
pub struct GsmConfig {
    #[serde(default)]
    pub source: GsmSourceKind,
    /// Path on disk — required when `source = "file"`.
    pub path: Option<String>,
    /// S3 bucket name — required when `source = "s3"`.
    pub bucket: Option<String>,
    /// S3 object key — required when `source = "s3"`.
    pub key: Option<String>,
    /// AWS region — optional when `source = "s3"`, falls back to `AWS_REGION` env var.
    pub region: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GsmSourceKind {
    #[default]
    Bundled,
    File,
    S3,
}
