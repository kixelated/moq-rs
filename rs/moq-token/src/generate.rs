use crate::Algorithm;

use aws_lc_rs::{
	encoding::{AsDer, Pkcs8V1Der},
	rand, rsa,
	signature::{self, KeyPair},
};

/// Generate a key pair for the given algorithm, returning the private and public keys.
pub fn generate(algorithm: Algorithm) -> (Vec<u8>, Vec<u8>) {
	match algorithm {
		Algorithm::HS256 => generate_hmac_key::<32>(),
		Algorithm::HS384 => generate_hmac_key::<48>(),
		Algorithm::HS512 => generate_hmac_key::<64>(),
		Algorithm::RS256 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::RS384 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::RS512 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::ES256 => generate_ec_key(&signature::ECDSA_P256_SHA256_FIXED_SIGNING),
		Algorithm::ES384 => generate_ec_key(&signature::ECDSA_P384_SHA384_FIXED_SIGNING),
		Algorithm::PS256 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::PS384 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::PS512 => generate_rsa_key(rsa::KeySize::Rsa2048),
		Algorithm::EdDSA => generate_ed25519_key(),
	}
}

fn generate_hmac_key<const SIZE: usize>() -> (Vec<u8>, Vec<u8>) {
	let mut key = [0u8; SIZE];
	rand::fill(&mut key).unwrap();
	(key.to_vec(), key.to_vec())
}

fn generate_rsa_key(size: rsa::KeySize) -> (Vec<u8>, Vec<u8>) {
	let key = rsa::KeyPair::generate(size).unwrap();
	let private_key = key.as_der().unwrap().as_ref().to_vec();
	let public_key = key.public_key().as_der().unwrap().as_ref().to_vec();

	(private_key, public_key)
}

fn generate_ec_key(size: &'static signature::EcdsaSigningAlgorithm) -> (Vec<u8>, Vec<u8>) {
	let key = signature::EcdsaKeyPair::generate(size).unwrap();
	let private_key = key.private_key().as_der().unwrap().as_ref().to_vec();
	let public_key = key.public_key().as_der().unwrap().as_ref().to_vec();

	(private_key, public_key)
}

fn generate_ed25519_key() -> (Vec<u8>, Vec<u8>) {
	let key = signature::Ed25519KeyPair::generate().unwrap();
	let private_key: Pkcs8V1Der = key.as_der().unwrap();
	let private_key = private_key.as_ref().to_vec();
	let public_key = key.public_key().as_der().unwrap().as_ref().to_vec();

	(private_key, public_key)
}
