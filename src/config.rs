use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(feature = "dirs")]
use dirs::{config_dir, home_dir};

use super::{Error, ErrorKind, Result};

static CONFIG_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static HOME_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn get_home_dir() -> Option<PathBuf> {
    #[cfg(feature = "dirs")]
    return HOME_DIR.get_or_init(|| home_dir()).clone();

    #[cfg(not(feature = "dirs"))]
    HOME_DIR.get().cloned().flatten()
}

pub fn get_config_dir() -> Option<PathBuf> {
    #[cfg(feature = "dirs")]
    return CONFIG_DIR.get_or_init(|| config_dir()).clone();

    #[cfg(not(feature = "dirs"))]
    CONFIG_DIR.get().cloned().flatten()
}

pub fn set_home_dir(path: PathBuf) -> Result<()> {
    HOME_DIR
        .set(Some(path))
        .map_err(|_| Error::new(ErrorKind::ConfigAlreadySet, "home_dir already set"))?;

    Ok(())
}

pub fn set_config_dir(path: PathBuf) -> Result<()> {
    CONFIG_DIR
        .set(Some(path))
        .map_err(|_| Error::new(ErrorKind::ConfigAlreadySet, "config_dir already set"))?;

    Ok(())
}
