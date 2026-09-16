//! Canonical anonymous account identifier for the browser/native client.
//!
//! The value stored here is the server's canonical `account_id`.

pub const ACCOUNT_ID_STORAGE_KEY: &str = "sow_account_id";
pub const ACCOUNT_SECRET_STORAGE_KEY: &str = "sow_account_secret";
pub const PENDING_DISPLAY_NAME_STORAGE_KEY: &str = "poki_ignore_sow_pending_display_name";

pub fn load_account_id() -> Option<String> {
    load_storage(ACCOUNT_ID_STORAGE_KEY)
}

pub fn save_account_id(account_id: &str) {
    save_storage(ACCOUNT_ID_STORAGE_KEY, account_id);
}

pub fn clear_account_id() {
    clear_storage(ACCOUNT_ID_STORAGE_KEY);
}

/// One-time ownership secret minted by sow-data on first profile fetch.
/// Presented (id + secret) on JoinWithAuth to bind stats and reconnects.
pub fn load_account_secret() -> Option<String> {
    load_storage(ACCOUNT_SECRET_STORAGE_KEY)
}

pub fn save_account_secret(secret: &str) {
    save_storage(ACCOUNT_SECRET_STORAGE_KEY, secret);
}

pub fn load_pending_display_name() -> Option<(Option<String>, String)> {
    let raw = load_storage(PENDING_DISPLAY_NAME_STORAGE_KEY)?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?
        .to_string();
    let account_id = value
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(str::to_string);
    Some((account_id, name))
}

pub fn save_pending_display_name(account_id: Option<&str>, name: &str) {
    let value = serde_json::json!({
        "account_id": account_id.unwrap_or_default(),
        "name": name,
    });
    if let Ok(raw) = serde_json::to_string(&value) {
        save_storage(PENDING_DISPLAY_NAME_STORAGE_KEY, &raw);
    }
}

pub fn clear_pending_display_name() {
    clear_storage(PENDING_DISPLAY_NAME_STORAGE_KEY);
}

#[cfg(target_arch = "wasm32")]
fn load_storage(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(key)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_storage(key: &str) -> Option<String> {
    std::fs::read_to_string(crate::paths::native_data_dir().join(key))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
fn save_storage(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_storage(key: &str, value: &str) {
    let path = crate::paths::native_data_dir().join(key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, value);
}

#[cfg(target_arch = "wasm32")]
fn clear_storage(key: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.remove_item(key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_storage(key: &str) {
    let _ = std::fs::remove_file(crate::paths::native_data_dir().join(key));
}
