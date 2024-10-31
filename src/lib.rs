use std::path::PathBuf;

use dirs::home_dir;
use git2::Repository;

pub fn initialize_repository() {
    let mut folder = home_dir().unwrap_or_else(|| PathBuf::from("."));

    folder = if cfg!(target_os = "linux") {
        folder.join(".local/share/rspass")
    } else {
        folder.join("rspass")
    };

    Repository::init(&folder).expect("Failed to initialize git repository");

    println!("Git repository initialized at {}", folder.to_str().unwrap());
}
