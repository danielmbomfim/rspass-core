use std::{fs::File, io::Read};

use chrono::Utc;
use pgp::{
    types::{SecretKeyRepr, SecretKeyTrait},
    ArmorOptions, KeyType, SecretKeyParamsBuilder,
};
use rand::{rngs::OsRng, thread_rng};
use rsa::{
    pkcs1::{DecodeRsaPublicKey, EncodeRsaPublicKey},
    RsaPrivateKey, RsaPublicKey,
};

use super::{Error, ErrorKind, Result};

pub struct Keys {
    pub pub_key: String,
    pub private_key: String,
    pub rsa_pub_key: String,
}

pub(crate) fn generate_key(name: &str, email: &str, password: &str) -> Result<Keys> {
    let params = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Rsa(2048))
        .primary_user_id(format!("{} <{}>", name, email))
        .passphrase(Some(password.to_owned()))
        .can_sign(true)
        .can_certify(true)
        .can_encrypt(true)
        .created_at(Utc::now())
        .build()
        .unwrap();

    let key = params.generate(OsRng).unwrap();
    let secret_key = key.sign(OsRng, || password.to_owned()).unwrap();

    let pub_key = secret_key
        .public_key()
        .sign(OsRng, &secret_key, || password.to_owned())
        .unwrap();

    let rsa_pub_key = secret_key
        .unlock(
            || password.to_owned(),
            |unlocked_key| match unlocked_key {
                SecretKeyRepr::RSA(key) => {
                    let key: RsaPrivateKey = (*key).clone();
                    let pub_key: RsaPublicKey = key.into();

                    Ok(pub_key)
                }
                _ => panic!("invalid private key data"),
            },
        )
        .map_err(|err| match err {
            pgp::errors::Error::RSAError(err) => {
                Error::new(ErrorKind::EncryptationError, err.to_string())
            }
            _ => panic!("unexpected error while encrypting message"),
        })?;

    let rsa_pub_key = rsa_pub_key
        .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
        .unwrap();

    let armored_pub_key = pub_key.to_armored_string(ArmorOptions::default());
    let armored_secret_key = secret_key.to_armored_string(ArmorOptions::default());

    Ok(Keys {
        private_key: armored_secret_key.unwrap(),
        pub_key: armored_pub_key.unwrap(),
        rsa_pub_key,
    })
}

pub(crate) fn recover_pub_key() -> Result<String> {
    let config_dir = super::get_config_path();
    let mut pub_key = String::new();

    File::open(config_dir.join("rspass.pub"))
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                Error::new(ErrorKind::NotInitialized, "Public key not found")
            }
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid public key")
            }
            _ => panic!("Unexpected error when opening public key"),
        })?
        .read_to_string(&mut pub_key)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid public key")
            }
            _ => panic!("Unexpected error when reading public key"),
        })?;

    Ok(pub_key)
}

pub(crate) fn recover_rsa_pub_key() -> Result<String> {
    let config_dir = super::get_config_path();

    let mut rsa_key = String::new();

    File::open(config_dir.join("rspass.pem"))
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                Error::new(ErrorKind::NotInitialized, "Public RSA key not found")
            }
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid RSA public key")
            }
            _ => panic!("Unexpected error when opening public RSA key"),
        })?
        .read_to_string(&mut rsa_key)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid RSA public key")
            }
            _ => panic!("Unexpected error when reading RSA public key"),
        })?;

    Ok(rsa_key)
}

pub(crate) fn encrypt(value: String, pub_key: String) -> Result<Vec<u8>> {
    let pub_key =
        RsaPublicKey::from_pkcs1_pem(&pub_key).expect("value should be a valid public key");

    let mut rng = thread_rng();

    let encrypted_data = pub_key
        .encrypt(&mut rng, rsa::Pkcs1v15Encrypt, value.as_bytes())
        .unwrap();

    Ok(encrypted_data)
}
