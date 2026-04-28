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
        let pem_bytes = STANDARD
            .decode(b64.trim())
            .map_err(|e| format!("invalid base64 for RSA_PRIVATE_KEY: {e}"))?;
        let pem_str = String::from_utf8(pem_bytes)
            .map_err(|e| format!("RSA_PRIVATE_KEY not valid UTF-8: {e}"))?;
        let private_key = RsaPrivateKey::from_pkcs8_pem(&pem_str)
            .map_err(|e| format!("failed to parse RSA private key: {e}"))?;
        Self::build(private_key)
    }

    pub fn generate() -> Self {
        tracing::warn!(
            "RSA_PRIVATE_KEY not set — generating ephemeral key. \
             JWKS will change on restart. Set RSA_PRIVATE_KEY for production."
        );
        let private_key =
            RsaPrivateKey::new(&mut OsRng, 2048).expect("failed to generate RSA-2048 key");
        Self::build(private_key).expect("failed to build JwkService from generated key")
    }

    fn build(private_key: RsaPrivateKey) -> Result<Self, String> {
        let pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("failed to export RSA key to PEM: {e}"))?;

        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|e| format!("failed to build EncodingKey: {e}"))?;

        let pub_key = private_key.to_public_key();
        let n_bytes = pub_key.n().to_bytes_be();
        let e_bytes = pub_key.e().to_bytes_be();

        let n_b64 = URL_SAFE_NO_PAD.encode(&n_bytes);
        let e_b64 = URL_SAFE_NO_PAD.encode(&e_bytes);

        let kid = hex::encode(&Sha256::digest(&n_bytes)[..8]);

        let jwks_json = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": kid,
                "n": n_b64,
                "e": e_b64
            }]
        })
        .to_string();

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
