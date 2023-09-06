use thiserror::Error;

#[derive(Error, Debug)]
#[error("{0}")]
pub struct ConfigError(pub String);

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
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

pub trait NoiseConfig {
    fn read_config(&self) -> Result<NoiseConfigData, ConfigError> {
        Ok(NoiseConfigData::default())
    }
    fn store_config(&self, _conf: &NoiseConfigData) -> Result<(), ConfigError> {
        Ok(())
    }
}

pub struct NoiseConfigNoCache;
impl NoiseConfig for NoiseConfigNoCache {}

pub struct PersistedNoiseConfig {
    config_dir: String,
}

impl PersistedNoiseConfig {
    /// Creates a new persisting noise config, which stores the pairing information in "bitbox.json"
    /// in the provided directory.
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

        let mut file = std::fs::File::open(config_path).map_err(|e| ConfigError(e.to_string()))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| ConfigError(e.to_string()))?;

        serde_json::from_str::<NoiseConfigData>(&contents).map_err(|e| ConfigError(e.to_string()))
    }

    fn store_config(&self, conf: &NoiseConfigData) -> Result<(), ConfigError> {
        use std::io::Write;

        let config_path = std::path::Path::new(&self.config_dir).join("bitbox.json");

        let mut file =
            std::fs::File::create(config_path).map_err(|e| ConfigError(e.to_string()))?;

        let data = serde_json::to_string(conf).map_err(|e| ConfigError(e.to_string()))?;

        file.write_all(data.as_bytes())
            .map_err(|e| ConfigError(e.to_string()))
    }
}
