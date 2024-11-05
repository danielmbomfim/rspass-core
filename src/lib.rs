use dirs::{config_dir, home_dir};
use git2::{Repository, Signature};
use pgp::{recover_rsa_pub_key, Keys};
use rand::distributions::Alphanumeric;
use rand::prelude::SliceRandom;
use rand::seq::IteratorRandom;
use rand::Rng;
use std::fs::{create_dir, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::{fs::File, io};

mod pgp;

#[derive(Debug)]
pub enum ErrorKind {
    InitializationError,
    PermissionDenied,
    NotInitialized,
    BadConfig,
    InsertionError,
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

fn get_repo_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(if cfg!(target_os = "linux") {
            ".local/share/rspass"
        } else {
            "rspass"
        })
}

fn get_config_path() -> PathBuf {
    config_dir().unwrap().join("rspass")
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

pub fn initialize_repository() -> Result<String> {
    let folder = get_repo_path();

    Repository::init(&folder).map_err(|_err| {
        Error::new(
            ErrorKind::InitializationError,
            "failed to initialize repository",
        )
    })?;

    Ok(folder.to_str().unwrap().to_owned())
}

pub fn insert_credential(
    name: &str,
    password: &str,
    metadata: Option<Vec<(String, String)>>,
) -> Result<()> {
    let repo_path = get_repo_path();

    let repository = Repository::open(&repo_path).map_err(|_err| {
        Error::new(
            ErrorKind::NotInitialized,
            "failed to access repository. Make sure to initialize a valid repository",
        )
    })?;

    let mut index = repository.index().map_err(|_err| {
        Error::new(
            ErrorKind::InsertionError,
            "Failed to obtain repository index",
        )
    })?;

    let file_path = repo_path.join(name);

    create_dir_all(file_path.as_path().parent().unwrap()).map_err(|err| match err.kind() {
        io::ErrorKind::PermissionDenied => Error::new(
            ErrorKind::PermissionDenied,
            "You dont have permission to create a subdirectory",
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

    index
        .add_path(&PathBuf::from(name))
        .expect("failed to add file");
    index.write().unwrap();

    let oid = index.write_tree().unwrap();
    let signature = Signature::now("rspass", "rspass@rspass").unwrap();
    let tree = repository.find_tree(oid).unwrap();

    let parent_commit = match repository.head() {
        Ok(head) => head.peel_to_commit().ok(),
        Err(_) => None,
    };

    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("add {:?}", name),
            &tree,
            parent_commit.iter().collect::<Vec<_>>().as_slice(),
        )
        .unwrap();

    Ok(())
}
