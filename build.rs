use std::io::Result;
fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    #[cfg(feature = "wasm")]
    let config = config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    #[cfg(feature = "wasm")]
    let config = config.type_attribute(".", "#[serde(rename_all = \"camelCase\")]");
    config.compile_protos(&["src/messages/hww.proto"], &["src/messages/"])?;
    Ok(())
}
