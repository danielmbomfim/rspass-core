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

pub fn fetch_from_remote(username: &str, token: &str) -> Result<()> {
    let repo = open_repository(&get_repo_path())?;

    let mut remote = repo
        .find_remote("origin")
        .map_err(|_| Error::new(ErrorKind::RemoteError, "failed to find remote"))?;

    let mut callbacks = git2::RemoteCallbacks::new();

    callbacks.credentials(|_, _, _| git2::Cred::userpass_plaintext(username, token));

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    remote
        .fetch(&["master"], Some(&mut fetch_options), None)
        .map_err(|_| Error::new(ErrorKind::FetchError, "failed to fetch master from origin"))?;

    let local_branch = repo.find_branch("master", git2::BranchType::Local).unwrap();
    let local_oid = local_branch.get().target().unwrap();

    let remote_branch_ref = format!("refs/remotes/origin/{}", "master");
    let remote_branch = repo
        .find_reference(&remote_branch_ref)
        .map_err(|_| Error::new(ErrorKind::FetchError, "failed to fetch master from origin"))?;
    let remote_oid = remote_branch.target().unwrap();

    if local_oid != remote_oid {
        let annotated_commit = repo
            .reference_to_annotated_commit(&remote_branch)
            .map_err(|_| Error::new(ErrorKind::FetchError, "failed to fetch master from origin"))?;
        let (analysis, _) = repo
            .merge_analysis(&[&annotated_commit])
            .map_err(|_| Error::new(ErrorKind::FetchError, "failed to fetch master from origin"))?;

        if analysis.is_fast_forward() {
            let mut reference = repo
                .find_reference(&format!("refs/heads/{}", "master"))
                .map_err(|_| {
                    Error::new(ErrorKind::FetchError, "failed to fetch master from origin")
                })?;
            reference
                .set_target(remote_oid, "Fast-forward")
                .map_err(|_| {
                    Error::new(ErrorKind::FetchError, "failed to fetch master from origin")
                })?;
            repo.set_head(&format!("refs/heads/{}", "master"))
                .map_err(|_| {
                    Error::new(ErrorKind::FetchError, "failed to fetch master from origin")
                })?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|_| {
                    Error::new(ErrorKind::FetchError, "failed to fetch master from origin")
                })?;
        } else if analysis.is_normal() {
            repo.merge(&[&annotated_commit], None, None).map_err(|_| {
                Error::new(ErrorKind::FetchError, "failed to fetch master from origin")
            })?;
        } else {
            println!("No merge necessary");
        }
    }
    Ok(())
}

pub fn push_to_remote(username: &str, token: &str) -> Result<()> {
    let repo = open_repository(&get_repo_path())?;

    let mut remote = repo
        .find_remote("origin")
        .map_err(|_| Error::new(ErrorKind::RemoteError, "failed to find remote"))?;

    let mut callbacks = git2::RemoteCallbacks::new();

    callbacks.credentials(|_, _, _| git2::Cred::userpass_plaintext(username, token));

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);

    remote
        .push(
            &["refs/heads/master:refs/heads/master"],
            Some(&mut push_options),
        )
        .map_err(|_| Error::new(ErrorKind::PushError, "failed to push to remote"))?;

    Ok(())
}
