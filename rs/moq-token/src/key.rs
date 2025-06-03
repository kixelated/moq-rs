use serde_with::base64::{Base64, UrlSafe};
use std::{collections::HashSet, fmt, fs::File, io::BufReader, path::Path, sync::OnceLock};

use jsonwebtoken::{DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::{Algorithm, Payload};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum KeyOperation {
	Sign,
	Verify,
	Decrypt,
	Encrypt,
}

/// Similar to JWK but not quite the same because it's annoying to implement.
#[serde_with::serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct Key {
	/// The algorithm used by the key.
	#[serde(rename = "alg")]
	pub algorithm: Algorithm,

	/// The operations that the key can perform.
	#[serde(rename = "key_ops")]
	pub operations: HashSet<KeyOperation>,

	/// The secret key as base64.
	#[serde(rename = "k")]
	#[serde_as(as = "Base64<UrlSafe>")]
	pub secret: Vec<u8>,

	/// The key ID, useful for rotating keys.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub kid: Option<String>,

	// Cached for performance reasons, unfortunately.
	#[serde(skip)]
	pub(crate) decode: OnceLock<DecodingKey>,

	#[serde(skip)]
	pub(crate) encode: OnceLock<EncodingKey>,
}

impl fmt::Debug for Key {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Key")
			.field("algorithm", &self.algorithm)
			.field("operations", &self.operations)
			.field("kid", &self.kid)
			.finish()
	}
}

impl Key {
	#[allow(clippy::should_implement_trait)]
	pub fn from_str(s: &str) -> anyhow::Result<Self> {
		Ok(serde_json::from_str(s)?)
	}

	pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
		let file = File::open(path)?;
		let reader = BufReader::new(file);
		Ok(serde_json::from_reader(reader)?)
	}

	pub fn to_str(&self) -> anyhow::Result<String> {
		Ok(serde_json::to_string(self)?)
	}

	pub fn to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
		let file = File::create(path)?;
		serde_json::to_writer(file, self)?;
		Ok(())
	}

	pub fn verify(&self, token: &str) -> anyhow::Result<Payload> {
		if !self.operations.contains(&KeyOperation::Verify) {
			return Err(anyhow::anyhow!("key does not support verification"));
		}

		let decode = self.decode.get_or_init(|| match self.algorithm {
			Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => DecodingKey::from_secret(&self.secret),
			/*
			Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => DecodingKey::from_rsa_der(&self.der),
			Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => DecodingKey::from_rsa_der(&self.der),
			Algorithm::ES256 | Algorithm::ES384 => DecodingKey::from_ec_der(&self.der),
			Algorithm::EdDSA => DecodingKey::from_ed_der(&self.der),
			*/
		});

		let mut validation = jsonwebtoken::Validation::new(self.algorithm.into());
		validation.required_spec_claims = Default::default(); // Don't require exp, but still validate it if present

		let token = jsonwebtoken::decode::<Payload>(token, decode, &validation)?;
		Ok(token.claims)
	}

	pub fn sign(&self, payload: &Payload) -> anyhow::Result<String> {
		if !self.operations.contains(&KeyOperation::Sign) {
			return Err(anyhow::anyhow!("key does not support signing"));
		}

		let encode = self.encode.get_or_init(|| match self.algorithm {
			Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => EncodingKey::from_secret(&self.secret),
			/*
			Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => EncodingKey::from_rsa_der(&self.der),
			Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => EncodingKey::from_rsa_der(&self.der),
			Algorithm::ES256 | Algorithm::ES384 => EncodingKey::from_ec_der(&self.der),
			Algorithm::EdDSA => EncodingKey::from_ed_der(&self.der),
			*/
		});

		let mut header = Header::new(self.algorithm.into());
		header.kid = self.kid.clone();
		let token = jsonwebtoken::encode(&header, &payload, encode)?;
		Ok(token)
	}

	/// Generate a key pair for the given algorithm, returning the private and public keys.
	pub fn generate(algorithm: Algorithm, id: Option<String>) -> Self {
		let private_key = match algorithm {
			Algorithm::HS256 => generate_hmac_key::<32>(),
			Algorithm::HS384 => generate_hmac_key::<48>(),
			Algorithm::HS512 => generate_hmac_key::<64>(),
			/*
			Algorithm::RS256 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::RS384 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::RS512 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::ES256 => generate_ec_key(&signature::ECDSA_P256_SHA256_FIXED_SIGNING),
			Algorithm::ES384 => generate_ec_key(&signature::ECDSA_P384_SHA384_FIXED_SIGNING),
			Algorithm::PS256 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::PS384 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::PS512 => generate_rsa_key(rsa::KeySize::Rsa2048),
			Algorithm::EdDSA => generate_ed25519_key(),
			*/
		};

		Key {
			kid: id.clone(),
			operations: [KeyOperation::Sign, KeyOperation::Verify].into(),
			algorithm,
			secret: private_key,
			decode: Default::default(),
			encode: Default::default(),
		}

		/*
		let public_key = Key {
			kid: id,
			operations: [KeyOperation::Verify].into(),
			algorithm,
			der: public_key,
			decode: Default::default(),
			encode: Default::default(),
		};

		(private_key, public_key)
		*/
	}
}

fn generate_hmac_key<const SIZE: usize>() -> Vec<u8> {
	let mut key = [0u8; SIZE];
	aws_lc_rs::rand::fill(&mut key).unwrap();
	key.to_vec()
}
