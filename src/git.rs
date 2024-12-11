use std::path::PathBuf;

use super::{Error, ErrorKind, Result};
use dirs::home_dir;
use git2::{Index, Repository, Signature};

pub fn get_repo_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(if cfg!(target_os = "linux") {
            ".local/share/rspass"
        } else {
            "rspass"
        })
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

pub(crate) fn open_repository(path: &PathBuf) -> Result<Repository> {
    Repository::open(path).map_err(|_err| {
        Error::new(
            ErrorKind::NotInitialized,
            "failed to access repository. Make sure to initialize a valid repository",
        )
    })
}

pub(crate) fn get_repo_index(repo: &Repository) -> Result<Index> {
    repo.index().map_err(|_err| {
        Error::new(
            ErrorKind::InsertionError,
            "Failed to obtain repository index",
        )
    })
}

pub fn commit_changes(
    repo: &Repository,
    additions: Option<Vec<&str>>,
    removals: Option<Vec<&str>>,
    message: &str,
) -> Result<()> {
    let mut index = get_repo_index(repo)?;

    if let Some(entries) = additions {
        entries.iter().for_each(|name| {
            index
                .add_path(&PathBuf::from(name))
                .expect("failed to add file");
        });
    }

    if let Some(entries) = removals {
        entries.iter().for_each(|name| {
            index
                .remove_path(&PathBuf::from(name))
                .expect("failed to add file");
        });
    }

    index.write().unwrap();

    let oid = index.write_tree().unwrap();
    let signature = Signature::now("rspass", "rspass@rspass").unwrap();
    let tree = repo.find_tree(oid).unwrap();

    let parent_commit = match repo.head() {
        Ok(head) => head.peel_to_commit().ok(),
        Err(_) => None,
    };

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        parent_commit.iter().collect::<Vec<_>>().as_slice(),
    )
    .unwrap();

    Ok(())
}

pub fn add_remote(uri: &str) -> Result<()> {
    let repo = open_repository(&get_repo_path())?;

    repo.remote("origin", uri).map_err(|_| {
        Error::new(
            ErrorKind::RemoteError,
            "failed to add remote, verify the params",
        )
    })?;

    Ok(())
}

pub fn fetch_from_remote() -> Result<()> {
    todo!()
}

pub fn push_to_remote() -> Result<()> {
    todo!()
}
