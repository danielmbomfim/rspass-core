use rand::distributions::Alphanumeric;
use rand::prelude::SliceRandom;
use rand::Rng;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use dirs::home_dir;
use git2::{Repository, Signature};
use rand::seq::IteratorRandom;

fn get_repo_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(if cfg!(target_os = "linux") {
            ".local/share/rspass"
        } else {
            "rspass"
        })
}

pub fn initialize_repository() {
    let folder = get_repo_path();

    Repository::init(&folder).expect("Failed to initialize git repository");

    println!("Git repository initialized at {}", folder.to_str().unwrap());
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

    println!("random password with {length} characters generated");

    password
}

pub fn insert_credential(name: &str, password: &str, metadata: Option<Vec<(String, String)>>) {
    let repo_path = get_repo_path();

    let repository = Repository::open(&repo_path)
        .expect("failed to open repository, make sure you have called \"init\".");

    let mut index = repository
        .index()
        .expect("failed to obtain repository index");

    let file_path = repo_path.join(name);
    let mut file = File::create(&file_path).expect("failed to create credential");

    file.write(password.as_bytes())
        .expect("failed to write credentials");

    if let Some(data) = metadata {
        data.iter().for_each(|(key, value)| {
            file.write(format!("\n{key}={value}").as_bytes())
                .expect("failed to write metadata");
        });
    }

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
}
