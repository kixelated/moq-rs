use jsonwebtoken::{EncodingKey, Header};

use crate::{Algorithm, Payload};

pub struct Encoder {
	algorithm: Algorithm,
	key: EncodingKey,
}

impl Encoder {
	pub fn new(algorithm: Algorithm, key: &[u8]) -> Self {
		let key = match algorithm {
			Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => EncodingKey::from_secret(key),
			Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => EncodingKey::from_rsa_der(key),
			Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => EncodingKey::from_rsa_der(key),
			Algorithm::ES256 | Algorithm::ES384 => EncodingKey::from_ec_der(key),
			Algorithm::EdDSA => EncodingKey::from_ed_der(key),
		};

		Self { key, algorithm }
	}

	pub fn sign(&self, payload: &Payload) -> String {
		let header = Header::new(self.algorithm);
		jsonwebtoken::encode(&header, &payload, &self.key).unwrap()
	}
}
