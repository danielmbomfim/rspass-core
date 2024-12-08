use dirs::config_dir;
use git::{commit_changes, open_repository};
use pgp::{decrypt, recover_private_key, recover_rsa_pub_key, Keys};
use rand::distributions::Alphanumeric;
use rand::prelude::SliceRandom;
use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::HashMap;
use std::fs::{self, create_dir, create_dir_all, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::{fs::File, io};

pub use git::{get_repo_path, initialize_repository};

mod git;
mod pgp;

#[derive(Debug)]
pub enum ErrorKind {
    InitializationError,
    PermissionDenied,
    NotInitialized,
    BadConfig,
    InsertionError,
    EditionError,
    RemovalError,
    AlreadyExists,
    EncryptationError,
    DecryptationError,
    NotFound,
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Error {
            kind,
            message: message.into(),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub fn get_config_path() -> PathBuf {
    config_dir().unwrap().join("rspass")
}

fn get_credential_file(path: &PathBuf, write_mode: bool) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(write_mode)
        .open(path)
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => Error::new(
                ErrorKind::NotFound,
                format!("no credential found for {:?}", path).as_str(),
            ),
            _ => panic!("unexpected error while reading credential"),
        })
}

pub fn generate_password(length: usize) -> String {
    let uppercase = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let lowercase = "abcdefghijklmnopqrstuvwxyz";
    let digits = "0123456789";
    let special_chars = "!@#$%^&*()";

    let mut password = String::new();
    password.push(uppercase.chars().choose(&mut rand::thread_rng()).unwrap());
    password.push(lowercase.chars().choose(&mut rand::thread_rng()).unwrap());
    password.push(digits.chars().choose(&mut rand::thread_rng()).unwrap());
    password.push(
        special_chars
            .chars()
            .choose(&mut rand::thread_rng())
            .unwrap(),
    );

    let remaining_length = length.saturating_sub(password.len());
    let additional_chars: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(remaining_length)
        .map(char::from)
        .collect();
    password.push_str(&additional_chars);

    let mut password_chars: Vec<char> = password.chars().collect();
    password_chars.shuffle(&mut rand::thread_rng());

    let password = password_chars.into_iter().collect();

    password
}

pub fn generate_keys(name: &str, email: &str, password: &str) -> Result<String> {
    let config_dir = get_config_path();

    match create_dir(&config_dir) {
        Ok(_) => {
            let Keys {
                pub_key,
                private_key,
                rsa_pub_key,
            } = pgp::generate_key(name, email, password)?;

            File::create_new(config_dir.clone().join("rspass.pub"))
                .unwrap()
                .write_all(pub_key.as_bytes())
                .unwrap();

            File::create_new(config_dir.clone().join("rspass.key"))
                .unwrap()
                .write_all(private_key.as_bytes())
                .unwrap();

            File::create_new(config_dir.clone().join("rspass.pem"))
                .unwrap()
                .write_all(rsa_pub_key.as_bytes())
                .unwrap();
        }
        Err(err) => match err.kind() {
            io::ErrorKind::AlreadyExists => {}
            io::ErrorKind::PermissionDenied => {
                return Err(Error::new(
                    ErrorKind::PermissionDenied,
                    "You dont have permission to create the config folder",
                ));
            }
            _ => panic!("failed to create config folder"),
        },
    };

    Ok(config_dir.to_str().unwrap().to_owned())
}

pub fn insert_credential(
    name: &str,
    password: &str,
    metadata: Option<Vec<(String, String)>>,
) -> Result<()> {
    let repo_path = get_repo_path();
    let repository = open_repository(&repo_path)?;
    let file_path = repo_path.join(name);

    create_dir_all(file_path.as_path().parent().unwrap()).map_err(|err| match err.kind() {
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to create a subdirectory",
        ),
        io::ErrorKind::AlreadyExists => Error::new(
            ErrorKind::AlreadyExists,
            "A credential already exists with this name",
        ),
        _ => panic!("Unexpected error while creating credentials directories"),
    })?;

    let pub_key = recover_rsa_pub_key()?;
    let mut file_data = String::new();

    let mut file = File::create_new(&file_path).map_err(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => Error::new(
            ErrorKind::AlreadyExists,
            "A credential already exists with this name",
        ),
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to edit the repository",
        ),
        _ => panic!("Unexpected error while creating credentials file"),
    })?;

    file_data.push_str(password);

    if let Some(data) = metadata {
        data.iter().for_each(|(key, value)| {
            file_data.push_str(format!("\n{key}={value}").as_str());
        });
    }

    file.write_all(pgp::encrypt(file_data, pub_key)?.as_ref())
        .expect("failed to write credentials");

    commit_changes(
        &repository,
        Some(vec![name]),
        None,
        &format!("add {:?}", name),
    )
}

pub fn get_credential(name: &str, password: &str, full: bool) -> Result<String> {
    let private_key = recover_private_key()?;

    let path = get_repo_path().join(name);
    let mut buffer = Vec::new();

    get_credential_file(&path, false)?
        .read_to_end(&mut buffer)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid credential data")
            }
            _ => panic!("unexpected error while reading credential"),
        })?;

    let credentials = decrypt(buffer, password, private_key)?;

    if full {
        Ok(credentials)
    } else {
        Ok(credentials.lines().next().unwrap().to_owned())
    }
}

pub fn edit_credential(
    name: &str,
    gpg_password: &str,
    password: Option<&str>,
    metadata: Option<Vec<(String, Option<String>)>>,
) -> Result<()> {
    let repo_path = get_repo_path();
    let file_path = repo_path.join(name);
    let mut buffer = Vec::new();
    let mut new_credential = String::new();
    let mut file = get_credential_file(&file_path, true)?;

    file.read_to_end(&mut buffer)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::InvalidData => {
                Error::new(ErrorKind::BadConfig, "Invalid credential data")
            }
            _ => panic!("unexpected error while reading credential"),
        })?;

    let pub_key = recover_rsa_pub_key()?;
    let private_key = recover_private_key()?;
    let credential = decrypt(buffer, gpg_password, private_key)?;

    match password {
        Some(pass) => new_credential.push_str(pass),
        None => new_credential.push_str(credential.lines().next().unwrap()),
    };

    let mut data: HashMap<String, String> = credential
        .lines()
        .filter_map(|line| {
            let mut split = line.splitn(2, '=');
            let key = split.next()?.to_owned();
            let value = split.next()?.to_owned();
            Some((key, value))
        })
        .collect();

    if let Some(metadata) = metadata {
        metadata.into_iter().for_each(|item| {
            let (key, value) = item;

            match value {
                Some(inner_value) => {
                    data.insert(key, inner_value);
                }
                None => {
                    data.remove(key.as_str());
                }
            };
        });
    }

    data.iter().for_each(|(key, value)| {
        new_credential.push_str(&format!("\n{}={}", key, value));
    });

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(pgp::encrypt(new_credential, pub_key)?.as_ref())
        .expect("failed to write credentials");

    let repository = open_repository(&repo_path)?;

    commit_changes(
        &repository,
        Some(vec![name]),
        None,
        &format!("update {:?}", name),
    )
}

pub fn remove_credential(name: &str) -> Result<()> {
    let repo_path = get_repo_path();
    let file_path = repo_path.join(name);

    fs::remove_file(file_path).map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => Error::new(ErrorKind::NotFound, "credential not found"),
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to remove this credential",
        ),
        io::ErrorKind::InvalidInput => Error::new(ErrorKind::NotFound, "credential not found"),
        _ => panic!("unexpected error while removing credential"),
    })?;

    let repository = open_repository(&repo_path)?;

    commit_changes(
        &repository,
        None,
        Some(vec![name]),
        &format!("remove {:?}", name),
    )
}

pub fn move_credential(target: &str, destination: &str) -> Result<()> {
    let repo_path = get_repo_path();
    let target_path = repo_path.join(target);
    let destination_path = repo_path.join(destination);

    create_dir_all(destination_path.parent().unwrap()).map_err(|err| match err.kind() {
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to create a subdirectory",
        ),
        io::ErrorKind::AlreadyExists => Error::new(
            ErrorKind::AlreadyExists,
            "A credential already exists with this name",
        ),
        _ => panic!("Unexpected error while creating credentials directories"),
    })?;

    fs::rename(&target_path, &destination_path).map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => Error::new(ErrorKind::NotFound, "credential not found"),
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to move this credential",
        ),
        _ => panic!("unexpected error while moving credential"),
    })?;

    let repository = open_repository(&repo_path)?;

    commit_changes(
        &repository,
        Some(vec![destination]),
        Some(vec![target]),
        &format!("move {} to {}", target, destination),
    )
}
