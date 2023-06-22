pub fn get<T: serde::de::DeserializeOwned>(key: &str) -> Result<T, ()> {
    let window = web_sys::window().ok_or(())?;
    let local_storage = match window.local_storage() {
        Ok(Some(ls)) => ls,
        _ => return Err(()),
    };

    match local_storage.get_item(key) {
        Ok(Some(config_str)) => serde_json::from_str(&config_str).or(Err(())),
        _ => Err(()),
    }
}

pub fn set<T: serde::Serialize>(key: &str, value: &T) -> Result<(), ()> {
    let window = web_sys::window().ok_or(())?;
    let local_storage = match window.local_storage() {
        Ok(Some(ls)) => ls,
        _ => return Err(()),
    };

    let value_str = serde_json::to_string(value).or(Err(()))?;

    local_storage.set_item(key, &value_str).or(Err(()))
}
