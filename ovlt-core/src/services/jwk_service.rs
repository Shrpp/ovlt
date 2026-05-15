use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use jsonwebtoken::EncodingKey;
use rand::rngs::OsRng;
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
    traits::PublicKeyParts,
    RsaPrivateKey,
};
use sha2::{Digest, Sha256};

pub struct JwkService {
    pub encoding_key: EncodingKey,
    pub kid: String,
    pub jwks_json: String,
}

impl JwkService {
    pub fn from_pem_b64(b64: &str) -> Result<Self, String> {
        Self::build(Self::parse_pem_b64(b64)?, None)
    }

    pub fn from_pem_b64_with_previous(
        current_b64: &str,
        previous_b64: &str,
    ) -> Result<Self, String> {
        Self::build(
            Self::parse_pem_b64(current_b64)?,
            Some(Self::parse_pem_b64(previous_b64)?),
        )
    }

    fn parse_pem_b64(b64: &str) -> Result<RsaPrivateKey, String> {
        let pem_bytes = STANDARD
            .decode(b64.trim())
            .map_err(|e| format!("invalid base64 for RSA key: {e}"))?;
        let pem_str =
            String::from_utf8(pem_bytes).map_err(|e| format!("RSA key not valid UTF-8: {e}"))?;
        RsaPrivateKey::from_pkcs8_pem(&pem_str)
            .map_err(|e| format!("failed to parse RSA private key: {e}"))
    }

    pub fn generate() -> Self {
        tracing::warn!(
            "RSA_PRIVATE_KEY not set — generating ephemeral key. \
             JWKS will change on restart. Set RSA_PRIVATE_KEY for production."
        );
        let private_key =
            RsaPrivateKey::new(&mut OsRng, 2048).expect("failed to generate RSA-2048 key");
        Self::build(private_key, None).expect("failed to build JwkService from generated key")
    }

    fn build(current: RsaPrivateKey, previous: Option<RsaPrivateKey>) -> Result<Self, String> {
        let pem = current
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("failed to export RSA key to PEM: {e}"))?;

        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|e| format!("failed to build EncodingKey: {e}"))?;

        let pub_key = current.to_public_key();
        let n_bytes = pub_key.n().to_bytes_be();
        let e_bytes = pub_key.e().to_bytes_be();
        let kid = hex::encode(&Sha256::digest(&n_bytes)[..8]);

        let mut keys = vec![serde_json::json!({
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": kid,
            "n": URL_SAFE_NO_PAD.encode(&n_bytes),
            "e": URL_SAFE_NO_PAD.encode(&e_bytes),
        })];

        if let Some(prev_key) = previous {
            let prev_pub = prev_key.to_public_key();
            let prev_n = prev_pub.n().to_bytes_be();
            let prev_e = prev_pub.e().to_bytes_be();
            let prev_kid = hex::encode(&Sha256::digest(&prev_n)[..8]);
            keys.push(serde_json::json!({
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": prev_kid,
                "n": URL_SAFE_NO_PAD.encode(&prev_n),
                "e": URL_SAFE_NO_PAD.encode(&prev_e),
            }));
        }

        let jwks_json = serde_json::json!({ "keys": keys }).to_string();

        Ok(Self {
            encoding_key,
            kid,
            jwks_json,
        })
    }

    pub fn sign_id_token(
        &self,
        claims: &impl serde::Serialize,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        jsonwebtoken::encode(&header, claims, &self.encoding_key)
    }
}
