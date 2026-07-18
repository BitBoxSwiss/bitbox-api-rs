// SPDX-License-Identifier: Apache-2.0

use crate::util::Threading;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("{0}")]
pub struct ConfigError(pub String);

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NoiseConfigData {
    pub app_static_privkey: Option<[u8; 32]>,
    pub device_static_pubkeys: Vec<Vec<u8>>,
}

impl NoiseConfigData {
    pub(crate) fn contains_device_static_pubkey(&self, pubkey: &[u8]) -> bool {
        self.device_static_pubkeys
            .iter()
            .any(|config_pubkey| config_pubkey.as_slice() == pubkey)
    }

    pub(crate) fn add_device_static_pubkey(&mut self, pubkey: &[u8]) {
        if !self.contains_device_static_pubkey(pubkey) {
            self.device_static_pubkeys.push(pubkey.to_vec());
        }
    }

    pub(crate) fn get_app_static_privkey(&self) -> Option<zeroize::Zeroizing<[u8; 32]>> {
        // This zeroize is just to make the types work. Ideally we'd zerioze the struct field too,
        // but that is not compatible with serde.
        self.app_static_privkey.map(zeroize::Zeroizing::new)
    }

    pub(crate) fn set_app_static_privkey(&mut self, privkey: &[u8]) -> Result<(), ConfigError> {
        self.app_static_privkey = Some(
            privkey
                .try_into()
                .map_err(|e: std::array::TryFromSliceError| ConfigError(e.to_string()))?,
        );
        Ok(())
    }
}

pub trait NoiseConfig: Threading {
    fn read_config(&self) -> Result<NoiseConfigData, ConfigError> {
        Ok(NoiseConfigData::default())
    }
    fn store_config(&self, _conf: &NoiseConfigData) -> Result<(), ConfigError> {
        Ok(())
    }
}

pub struct NoiseConfigNoCache;
impl NoiseConfig for NoiseConfigNoCache {}
impl Threading for NoiseConfigNoCache {}

#[cfg(unix)]
fn ensure_private_file_permissions(path: &std::path::Path) -> Result<(), ConfigError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| ConfigError(e.to_string()))?;

    // A symlink can be part of an intentional setup. Do not change its target's permissions.
    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| ConfigError(e.to_string()))
}

pub struct PersistedNoiseConfig {
    config_dir: String,
}

impl Threading for PersistedNoiseConfig {}

impl PersistedNoiseConfig {
    /// Creates a new persisting noise config, which stores the pairing information in "bitbox.json"
    /// in the provided directory. The directory must already exist and should be created with
    /// `0700` permissions on Unix.
    pub fn new(config_dir: &str) -> PersistedNoiseConfig {
        PersistedNoiseConfig {
            config_dir: config_dir.into(),
        }
    }
}

impl NoiseConfig for PersistedNoiseConfig {
    fn read_config(&self) -> Result<NoiseConfigData, ConfigError> {
        use std::io::Read;

        let config_path = std::path::Path::new(&self.config_dir).join("bitbox.json");

        if !config_path.exists() {
            return Ok(NoiseConfigData::default());
        }

        let mut file = std::fs::File::open(&config_path).map_err(|e| ConfigError(e.to_string()))?;

        #[cfg(unix)]
        ensure_private_file_permissions(&config_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| ConfigError(e.to_string()))?;

        serde_json::from_str::<NoiseConfigData>(&contents).map_err(|e| ConfigError(e.to_string()))
    }

    fn store_config(&self, conf: &NoiseConfigData) -> Result<(), ConfigError> {
        use std::io::Write;

        let config_path = std::path::Path::new(&self.config_dir).join("bitbox.json");
        let data = serde_json::to_string(conf).map_err(|e| ConfigError(e.to_string()))?;

        let mut options = std::fs::File::options();
        options.write(true).create(true);

        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(&config_path)
            .map_err(|e| ConfigError(e.to_string()))?;

        #[cfg(unix)]
        ensure_private_file_permissions(&config_path)?;

        file.set_len(0).map_err(|e| ConfigError(e.to_string()))?;

        file.write_all(data.as_bytes())
            .map_err(|e| ConfigError(e.to_string()))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let unique = format!(
                "bitbox-api-noise-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            Self(std::env::temp_dir().join(unique))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn store_config_uses_private_file_permissions() {
        let dir = TempDir::new();
        std::fs::create_dir(&dir.0).unwrap();
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = PersistedNoiseConfig::new(dir.0.to_str().unwrap());

        config.store_config(&NoiseConfigData::default()).unwrap();

        assert_eq!(mode(&dir.0), 0o755);
        assert_eq!(mode(&dir.0.join("bitbox.json")), 0o600);
    }

    #[test]
    fn store_config_does_not_create_config_directory() {
        let dir = TempDir::new();
        let config = PersistedNoiseConfig::new(dir.0.to_str().unwrap());

        assert!(config.store_config(&NoiseConfigData::default()).is_err());
        assert!(!dir.0.exists());
    }

    #[test]
    fn store_config_truncates_existing_file() {
        let dir = TempDir::new();
        std::fs::create_dir(&dir.0).unwrap();
        let config_path = dir.0.join("bitbox.json");
        std::fs::write(&config_path, vec![b'x'; 1024]).unwrap();
        let config = PersistedNoiseConfig::new(dir.0.to_str().unwrap());
        let data = NoiseConfigData::default();

        config.store_config(&data).unwrap();

        assert_eq!(
            std::fs::read_to_string(config_path).unwrap(),
            serde_json::to_string(&data).unwrap()
        );
    }

    #[test]
    fn read_config_repairs_permissive_permissions() {
        let dir = TempDir::new();
        std::fs::create_dir(&dir.0).unwrap();
        std::fs::write(
            dir.0.join("bitbox.json"),
            r#"{"app_static_privkey":null,"device_static_pubkeys":[]}"#,
        )
        .unwrap();
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(
            dir.0.join("bitbox.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let config = PersistedNoiseConfig::new(dir.0.to_str().unwrap());

        config.read_config().unwrap();

        assert_eq!(mode(&dir.0), 0o755);
        assert_eq!(mode(&dir.0.join("bitbox.json")), 0o600);
    }

    #[test]
    fn store_config_does_not_change_file_symlink_target_permissions() {
        let dir = TempDir::new();
        std::fs::create_dir(&dir.0).unwrap();
        let target = dir.0.join("target.json");
        std::fs::write(&target, "{}").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        let config_dir = dir.0.join("config");
        std::fs::create_dir(&config_dir).unwrap();
        std::os::unix::fs::symlink(&target, config_dir.join("bitbox.json")).unwrap();
        let config = PersistedNoiseConfig::new(config_dir.to_str().unwrap());

        config.store_config(&NoiseConfigData::default()).unwrap();

        assert!(std::fs::symlink_metadata(config_dir.join("bitbox.json"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(mode(&target), 0o644);
    }
}
