use jsonwebtoken::DecodingKey;

use crate::{Algorithm, Payload, Result};

pub struct Decoder {
	algorithm: Algorithm,
	key: DecodingKey,
}

impl Decoder {
	pub fn new(algorithm: Algorithm, key: &[u8]) -> Self {
		let key = match algorithm {
			Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => DecodingKey::from_secret(key),
			Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => DecodingKey::from_rsa_der(key),
			Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => DecodingKey::from_rsa_der(key),
			Algorithm::ES256 | Algorithm::ES384 => DecodingKey::from_ec_der(key),
			Algorithm::EdDSA => DecodingKey::from_ed_der(key),
		};

		Self { key, algorithm }
	}

	pub fn decode(&self, token: &str) -> Result<Payload> {
		let mut validation = jsonwebtoken::Validation::new(self.algorithm);
		validation.required_spec_claims = Default::default(); // Don't require exp, but still validate it

		let token = jsonwebtoken::decode::<Payload>(token, &self.key, &validation)?;
		Ok(token.claims)
	}
}
