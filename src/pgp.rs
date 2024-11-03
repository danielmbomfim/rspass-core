use std::{fs::File, io::Read};

use chrono::Utc;
use pgp::{types::SecretKeyTrait, ArmorOptions, KeyType, SecretKeyParamsBuilder};
use rand::rngs::OsRng;

use super::{Error, ErrorKind, Result};

pub(crate) fn generate_key(name: &str, email: &str, password: &str) -> Result<(String, String)> {
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

    let armored_pub_key = pub_key.to_armored_string(ArmorOptions::default());
    let armored_secret_key = secret_key.to_armored_string(ArmorOptions::default());

    Ok((armored_pub_key.unwrap(), armored_secret_key.unwrap()))
}

pub(crate) fn recover_keys() -> Result<(String, String)> {
    let config_dir = dirs::config_dir().unwrap();

    let mut pub_key = String::new();
    let mut private_key = String::new();

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

    File::open(config_dir.join("rspass.key"))
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                Error::new(ErrorKind::NotInitialized, "Private key not found")
            }
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid private key")
            }
            _ => panic!("Unexpected error when opening private key"),
        })?
        .read_to_string(&mut private_key)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid private key")
            }
            _ => panic!("Unexpected error when reading private key"),
        })?;

    Ok((pub_key, private_key))
}
