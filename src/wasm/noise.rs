use super::localstorage;

use crate::noise::{ConfigError, NoiseConfigData};

pub static LOCAL_STORAGE_CONFIG_KEY: &str = "bitbox02Config";

/// Store the noise keys in the browser localstorage if possible.
pub(crate) struct LocalStorageNoiseConfig {}

impl crate::util::Threading for LocalStorageNoiseConfig {}

impl crate::NoiseConfig for LocalStorageNoiseConfig {
    fn read_config(&self) -> Result<NoiseConfigData, ConfigError> {
        localstorage::get(LOCAL_STORAGE_CONFIG_KEY).or(Ok(NoiseConfigData::default()))
    }

    fn store_config(&self, conf: &NoiseConfigData) -> Result<(), ConfigError> {
        localstorage::set(LOCAL_STORAGE_CONFIG_KEY, conf)
            .map_err(|_| ConfigError("could not write to localstorage".into()))
    }
}
