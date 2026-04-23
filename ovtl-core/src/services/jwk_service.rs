use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{DecodingKey, EncodingKey};
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding},
    traits::PublicKeyParts,
    RsaPrivateKey, RsaPublicKey,
};
use sha2::{Digest, Sha256};

pub struct JwkService {
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub kid: String,
    pub jwks_json: serde_json::Value,
}

impl JwkService {
    pub fn from_pem(pem_b64: &str) -> Result<Self, String> {
        let pem_bytes = base64::engine::general_purpose::STANDARD
            .decode(pem_b64)
            .map_err(|e| format!("RSA_PRIVATE_KEY base64 decode failed: {e}"))?;
        let pem_str =
            String::from_utf8(pem_bytes).map_err(|e| format!("RSA_PRIVATE_KEY not UTF-8: {e}"))?;
        let private_key = RsaPrivateKey::from_pkcs8_pem(&pem_str)
            .map_err(|e| format!("RSA_PRIVATE_KEY parse failed: {e}"))?;
        Self::build(private_key)
    }

    pub fn generate() -> Result<Self, String> {
        tracing::warn!(
            "RSA_PRIVATE_KEY not set — generating ephemeral key. \
             id_tokens will be invalid after restart. Set RSA_PRIVATE_KEY in production."
        );
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048)
            .map_err(|e| format!("RSA key generation failed: {e}"))?;
        Self::build(private_key)
    }

    fn build(private_key: RsaPrivateKey) -> Result<Self, String> {
        let pem = private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| format!("PKCS8 PEM export failed: {e}"))?;

        let public_key = RsaPublicKey::from(&private_key);

        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|e| format!("EncodingKey build failed: {e}"))?;

        let public_pem = public_key
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| format!("public PEM export failed: {e}"))?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| format!("DecodingKey build failed: {e}"))?;

        let n_bytes = public_key.n().to_bytes_be();
        let e_bytes = public_key.e().to_bytes_be();
        let kid = hex::encode(&Sha256::digest(&n_bytes)[..4]);

        let jwks_json = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": kid,
                "n": URL_SAFE_NO_PAD.encode(&n_bytes),
                "e": URL_SAFE_NO_PAD.encode(&e_bytes),
            }]
        });

        Ok(Self {
            encoding_key,
            decoding_key,
            kid,
            jwks_json,
        })
    }
}
