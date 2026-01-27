use crate::{
    crypto::encryption_manager::encryption_interface::Encryption,
    error::{self, ContainerError},
};
use josekit::{jwe, jws};
use masking::PeekInterface;

pub struct JWEncryption {
    pub(crate) private_key: masking::Secret<String>,
    pub(crate) public_key: masking::Secret<String>,
    pub(crate) encryption_algo: jwe::alg::rsaes::RsaesJweAlgorithm,
    pub(crate) decryption_algo: jwe::alg::rsaes::RsaesJweAlgorithm,
}

impl JWEncryption {
    pub fn new(
        private_key: String,
        public_key: String,
        enc_algo: jwe::alg::rsaes::RsaesJweAlgorithm,
        dec_algo: jwe::alg::rsaes::RsaesJweAlgorithm,
    ) -> Self {
        Self {
            private_key: private_key.into(),
            public_key: public_key.into(),
            encryption_algo: enc_algo,
            decryption_algo: dec_algo,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JwsBody {
    pub header: String,
    pub payload: String,
    pub signature: String,
}

impl JwsBody {
    fn from_dotted_str(input: &str) -> Option<Self> {
        let mut data = input.split('.');
        let header = data.next()?.to_string();
        let payload = data.next()?.to_string();
        let signature = data.next()?.to_string();
        Some(Self {
            header,
            payload,
            signature,
        })
    }

    pub fn get_dotted_jws(self) -> String {
        let header = self.header;
        let payload = self.payload;
        let signature = self.signature;
        format!("{header}.{payload}.{signature}")
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JweBody {
    pub header: String,
    pub iv: String,
    pub encrypted_payload: String,
    pub tag: String,
    pub encrypted_key: String,
}

impl JweBody {
    fn from_str(input: &str) -> Option<Self> {
        let mut data = input.split('.');
        let header = data.next()?.to_string();
        let encrypted_key = data.next()?.to_string();
        let iv = data.next()?.to_string();
        let encrypted_payload = data.next()?.to_string();
        let tag = data.next()?.to_string();
        Some(Self {
            header,
            iv,
            encrypted_payload,
            tag,
            encrypted_key,
        })
    }

    pub fn get_dotted_jwe(self) -> String {
        let header = self.header;
        let encryption_key = self.encrypted_key;
        let iv = self.iv;
        let encryption_payload = self.encrypted_payload;
        let tag = self.tag;
        format!("{header}.{encryption_key}.{iv}.{encryption_payload}.{tag}")
    }
}

impl Encryption<Vec<u8>, JweBody> for JWEncryption {
    type ReturnType<'a, T> = Result<T, ContainerError<error::CryptoError>>;

    fn encrypt(&self, input: Vec<u8>) -> Self::ReturnType<'_, JweBody> {
        let payload = input;
        let jws_encoded = jws_sign_payload(&payload, self.private_key.peek().as_bytes())?;
        let jws_body = JwsBody::from_dotted_str(&jws_encoded).ok_or(
            error::CryptoError::InvalidData("JWS encoded data is incomplete"),
        )?;
        let jws_payload = serde_json::to_vec(&jws_body).map_err(error::CryptoError::from)?;
        let jwe_encrypted = encrypt_jwe(
            &jws_payload,
            self.public_key.peek().as_bytes(),
            self.encryption_algo,
        )?;
        let jwe_body = JweBody::from_str(&jwe_encrypted)
            .ok_or(error::CryptoError::InvalidData("JWE data incomplete"))?;
        Ok(jwe_body)
    }

    fn decrypt(&self, input: JweBody) -> Self::ReturnType<'_, Vec<u8>> {
        let jwe_encoded = input.get_dotted_jwe();
        let jwe_decrypted =
            decrypt_jwe(&jwe_encoded, self.private_key.peek(), self.decryption_algo)?;

        let jws_parsed: JwsBody = serde_json::from_str(&jwe_decrypted)
            .map_err(|_| error::CryptoError::InvalidData("Failed while extracting jws body"))?;

        let jws_encoded = jws_parsed.get_dotted_jws();
        let output = verify_sign(jws_encoded, self.public_key.peek().as_bytes())?;
        Ok(output.as_bytes().to_vec())
    }
}

pub fn jws_sign_payload(
    payload: &[u8],
    private_key: impl AsRef<[u8]>,
) -> Result<String, error::CryptoError> {
    let alg = jws::RS256;
    let src_header = jws::JwsHeader::new();
    let signer = alg.signer_from_pem(private_key)?;
    Ok(jws::serialize_compact(payload, &src_header, &signer)?)
}

pub fn encrypt_jwe(
    payload: &[u8],
    public_key: impl AsRef<[u8]>,
    alg: jwe::alg::rsaes::RsaesJweAlgorithm,
) -> Result<String, error::CryptoError> {
    let enc = "A256GCM";
    let mut src_header = jwe::JweHeader::new();
    src_header.set_content_encryption(enc);
    src_header.set_token_type("JWT");
    let encrypter = alg.encrypter_from_pem(public_key)?;

    Ok(jwe::serialize_compact(payload, &src_header, &encrypter)?)
}

pub fn decrypt_jwe(
    jwt: &str,
    private_key: impl AsRef<[u8]>,
    alg: jwe::alg::rsaes::RsaesJweAlgorithm,
) -> Result<String, error::CryptoError> {
    let decrypter = alg.decrypter_from_pem(private_key)?;

    let (dst_payload, _dst_header) = jwe::deserialize_compact(jwt, &decrypter)?;

    Ok(String::from_utf8(dst_payload)?)
}

pub fn verify_sign(jws_body: String, key: impl AsRef<[u8]>) -> Result<String, error::CryptoError> {
    let alg = jws::RS256;
    let input = jws_body.as_bytes();
    let verifier = alg.verifier_from_pem(key)?;
    let (dst_payload, _dst_header) = jws::deserialize_compact(input, &verifier)?;
    let resp = String::from_utf8(dst_payload)?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // Keys used for tests
    // Can be generated using the following commands:
    // `openssl genrsa -out private_self.pem 2048`
    // `openssl rsa -in private_key.pem -pubout -out public_self.pem`
    const ENCRYPTION_KEY: &str = "\
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwa6siKaSYqD1o4J3AbHq
Km8oVTvep7GoN/C45qY60C7DO72H1O7Ujt6ZsSiK83EyI0CaUg3ORPS3ayobFNmu
zR366ckK8GIf3BG7sVI6u/9751z4OvBHZMM9JFWa7Bx/RCPQ8aeM+iJoqf9auuQm
3NCTlfaZJif45pShswR+xuZTR/bqnsOSP/MFROI9ch0NE7KRogy0tvrZe21lP24i
Ro2LJJG+bYshxBddhxQf2ryJ85+/Trxdu16PunodGzCl6EMT3bvb4ZC41i15omqU
aXXV1Z1wYUhlsO0jyd1bVvjyuE/KE1TbBS0gfR/RkacODmmE2zEdZ0EyyiXwqkmc
oQIDAQAB
-----END PUBLIC KEY-----
";
    const DECRYPTION_KEY: &str = "\
-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAwa6siKaSYqD1o4J3AbHqKm8oVTvep7GoN/C45qY60C7DO72H
1O7Ujt6ZsSiK83EyI0CaUg3ORPS3ayobFNmuzR366ckK8GIf3BG7sVI6u/9751z4
OvBHZMM9JFWa7Bx/RCPQ8aeM+iJoqf9auuQm3NCTlfaZJif45pShswR+xuZTR/bq
nsOSP/MFROI9ch0NE7KRogy0tvrZe21lP24iRo2LJJG+bYshxBddhxQf2ryJ85+/
Trxdu16PunodGzCl6EMT3bvb4ZC41i15omqUaXXV1Z1wYUhlsO0jyd1bVvjyuE/K
E1TbBS0gfR/RkacODmmE2zEdZ0EyyiXwqkmcoQIDAQABAoIBAEavZwxgLmCMedl4
zdHyipF+C+w/c10kO05fLjwPQrujtWDiJOaTW0Pg/ZpoP33lO/UdqLR1kWgdH6ue
rE+Jun/lhyM3WiSsyw/X8PYgGotuDFw90+I+uu+NSY0vKOEu7UuC/siS66KGWEhi
h0xZ480G2jYKz43bXL1aVUEuTM5tsjtt0a/zm08DEluYwrmxaaTvHW2+8FOn3z8g
UMClV2mN9X3rwlRhKAI1RVlymV95LmkTgzA4wW/M4j0kk108ouY8bo9vowoqidpo
0zKGfnqbQCIZP1QY6Xj8f3fqMY7IrFDdFHCEBXs29DnRz4oS8gYCAUXDx5iEVa1R
KVxic5kCgYEA4vGWOANuv+RoxSnNQNnZjqHKjhd+9lXARnK6qVVXcJGTps72ILGJ
CNrS/L6ndBePQGhtLtVyrvtS3ZvYhsAzJeMeeFUSZhQ2SOP5SCFWRnLJIBObJ5/x
fFwrCbp38qsEBlqJXue4JQCOxqO4P6YYUmeE8fxLPmdVNWq5fNe2YtsCgYEA2nrl
iMfttvNfQGX4pB3yEh/eWwqq4InFQdmWVDYPKJFG4TtUKJ48vzQXJqKfCBZ2q387
bH4KaKNWD7rYz4NBfE6z6lUc8We9w1tjVaqs5omBKUuovz8/8miUtxf2W5T2ta1/
zl9NyQ57duO423PeaCgPKKz3ftaxlz8G1CKYMTMCgYEAqkR7YhchNpOWD6cnOeq4
kYzNvgHe3c7EbZaSeY1wByMR1mscura4i44yEjKwzCcI8Vfn4uV+H86sA1xz/dWi
CmD2cW3SWgf8GoAAfZ+VbVGdmJVdKUOVGKrGF4xxhf3NDT9MJYpQ3GIovNwE1qw1
P04vrqaNhYpdobAq7oGhc1UCgYAkimNzcgTHEYM/0Q453KxM7bmRvoH/1esA7XRg
Fz6HyWxyZSrZNEXysLKiipZQkvk8C6aTqazx/Ud6kASNCGXedYdPzPZvRauOTe2a
OVZ7pEnO71GE0v5N+8HLsZ1JieuNTTxP9s6aruplYwba5VEwWGrYob0vIJdJNYhd
2H9d0wKBgFzqGPvG8u1lVOLYDU9BjhA/3l00C97WHIG0Aal70PVyhFhm5ILNSHU1
Sau7H1Bhzy5G7rwt05LNpU6nFcAGVaZtzl4/+FYfYIulubYjuSEh72yuBHHyvi1/
4Zql8DXhF5kkKx75cMcIxZ/ceiRiQyjzYv3LoTTADHHjzsiBEiQY
-----END RSA PRIVATE KEY-----
";

    const SIGNATURE_VERIFICATION_KEY: &str = "\
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA5Z/K0JWds8iHhWCa+rj0
rhOQX1nVs/ArQ1D0vh3UlSPR2vZUTrkdP7i3amv4d2XDC+3+5/YWExTkpxqnfl1T
9J37leN2guAARed6oYoTDEP/OoKtnUrKK2xk/+V5DNOWcRiSpcCrJOOIEoACOlPI
rXQSg16KDZQb0QTMntnsiPIJDbsOGcdKytRAcNaokiKLnia1v13N3bk6dSplrj1Y
zawslZfgqD0eov4FjzBMoA19yNtlVLLf6kOkLcFQjTKXJLP1tLflLUBPTg8fm9wg
APK2BjMQ2AMkUxx0ubbtw/9CeJ+bFWrqGnEhlvfDMlyAV77sAiIdQ4mXs3TLcLb/
AQIDAQAB
-----END PUBLIC KEY-----
";
    const SIGNING_KEY: &str = "\
-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA5Z/K0JWds8iHhWCa+rj0rhOQX1nVs/ArQ1D0vh3UlSPR2vZU
TrkdP7i3amv4d2XDC+3+5/YWExTkpxqnfl1T9J37leN2guAARed6oYoTDEP/OoKt
nUrKK2xk/+V5DNOWcRiSpcCrJOOIEoACOlPIrXQSg16KDZQb0QTMntnsiPIJDbsO
GcdKytRAcNaokiKLnia1v13N3bk6dSplrj1YzawslZfgqD0eov4FjzBMoA19yNtl
VLLf6kOkLcFQjTKXJLP1tLflLUBPTg8fm9wgAPK2BjMQ2AMkUxx0ubbtw/9CeJ+b
FWrqGnEhlvfDMlyAV77sAiIdQ4mXs3TLcLb/AQIDAQABAoIBAGNekD1N0e5AZG1S
zh6cNb6zVrH8xV9WGtLJ0PAJJrrXwnQYT4m10DOIM0+Jo/+/ePXLq5kkRI9DZmPu
Q/eKWc+tInfN9LZUS6n0r3wCrZWMQ4JFlO5RtEWwZdDbtFPZqOwObz/treKL2JHw
9YXaRijR50UUf3e61YLRqd9AfX0RNuuG8H+WgU3Gwuh5TwRnljM3JGaDPHsf7HLS
tNkqJuByp26FEbOLTokZDbHN0sy7/6hufxnIS9AK4vp8y9mZAjghG26Rbg/H71mp
Z+Q6P1y7xdgAKbhq7usG3/o4Y1e9wnghHvFS7DPwGekHQH2+LsYNEYOir1iRjXxH
GUXOhfUCgYEA+cR9jONQFco8Zp6z0wdGlBeYoUHVMjThQWEblWL2j4RG/qQd/y0j
uhVeU0/PmkYK2mgcjrh/pgDTtPymc+QuxBi+lexs89ppuJIAgMvLUiJT67SBHguP
l4+oL9U78KGh7PfJpMKH+Pk5yc1xucAevk0wWmr5Tz2vKRDavFTPV+MCgYEA61qg
Y7yN0cDtxtqlZdMC8BJPFCQ1+s3uB0wetAY3BEKjfYc2O/4sMbixXzt5PkZqZy96
QBUBxhcM/rIolpM3nrgN7h1nmJdk9ljCTjWoTJ6fDk8BUh8+0GrVhTbe7xZ+bFUN
UioIqvfapr/q/k7Ah2mCBE04wTZFry9fndrH2ssCgYEAh1T2Cj6oiAX6UEgxe2h3
z4oxgz6efAO3AavSPFFQ81Zi+VqHflpA/3TQlSerfxXwj4LV5mcFkzbjfy9eKXE7
/bjCm41tQ3vWyNEjQKYr1qcO/aniRBtThHWsVa6eObX6fOGN+p4E+txfeX693j3A
6q/8QSGxUERGAmRFgMIbTq0CgYAmuTeQkXKII4U75be3BDwEgg6u0rJq/L0ASF74
4djlg41g1wFuZ4if+bJ9Z8ywGWfiaGZl6s7q59oEgg25kKljHQd1uTLVYXuEKOB3
e86gJK0o7ojaGTf9lMZi779IeVv9uRTDAxWAA93e987TXuPAo/R3frkq2SIoC9Rg
paGidwKBgBqYd/iOJWsUZ8cWEhSE1Huu5rDEpjra8JPXHqQdILirxt1iCA5aEQou
BdDGaDr8sepJbGtjwTyiG8gEaX1DD+KsF2+dQRQdQfcYC40n8fKkvpFwrKjDj1ac
VuY3OeNxi+dC2r7HppP3O/MJ4gX/RJJfSrcaGP8/Ke1W5+jE97Qy
-----END RSA PRIVATE KEY-----
";

    #[test]
    fn test_jwe() {
        let jwt = encrypt_jwe("request_payload".as_bytes(), ENCRYPTION_KEY, jwe::RSA_OAEP).unwrap();
        let alg = jwe::RSA_OAEP;
        let payload = decrypt_jwe(&jwt, DECRYPTION_KEY, alg).unwrap();
        assert_eq!("request_payload".to_string(), payload)
    }

    #[test]
    fn test_jws() {
        let jwt = jws_sign_payload("jws payload".as_bytes(), SIGNING_KEY).unwrap();
        let payload = verify_sign(jwt, SIGNATURE_VERIFICATION_KEY).unwrap();
        assert_eq!("jws payload".to_string(), payload)
    }
}
