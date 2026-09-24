//! Canonical anonymous account identifier for the browser client.
//!
//! The value stored here is the server's canonical `account_id`.

pub const ACCOUNT_ID_STORAGE_KEY: &str = "sow_account_id";
pub const ACCOUNT_SECRET_STORAGE_KEY: &str = "sow_account_secret";
pub const PENDING_DISPLAY_NAME_STORAGE_KEY: &str = "poki_ignore_sow_pending_display_name";
const PENDING_REWARD_RECEIPTS_STORAGE_PREFIX: &str = "sow_pending_reward_receipts_v1:";

fn pending_reward_receipts_storage_key(account_id: &str) -> Option<String> {
    let account_id = account_id.trim();
    (!account_id.is_empty() && account_id.len() <= 128)
        .then(|| format!("{PENDING_REWARD_RECEIPTS_STORAGE_PREFIX}{account_id}"))
}

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
pub fn load_pending_reward_receipt_ids(account_id: &str) -> std::collections::BTreeSet<String> {
    let Some(key) = pending_reward_receipts_storage_key(account_id) else {
        return Default::default();
    };
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return Default::default();
    };
    match storage.get_item(&key) {
        Ok(Some(raw)) => match serde_json::from_str(&raw) {
            Ok(receipt_ids) => receipt_ids,
            Err(error) => {
                log::warn!("[rewards] saved pending receipt list is invalid: {error}");
                Default::default()
            }
        },
        Ok(None) => Default::default(),
        Err(error) => {
            log::warn!("[rewards] saved pending receipt list could not be read: {error:?}");
            Default::default()
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_pending_reward_receipt_ids(
    account_id: &str,
    receipt_ids: &std::collections::BTreeSet<String>,
) {
    let Some(key) = pending_reward_receipts_storage_key(account_id) else {
        log::warn!("[rewards] pending receipts were not saved: invalid account id");
        return;
    };
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        log::debug!("[rewards] local storage unavailable; retry is limited to this session");
        return;
    };
    let result = if receipt_ids.is_empty() {
        storage.remove_item(&key)
    } else {
        match serde_json::to_string(receipt_ids) {
            Ok(raw) => storage.set_item(&key, &raw),
            Err(error) => {
                log::warn!("[rewards] pending receipt list could not be encoded: {error}");
                return;
            }
        }
    };
    if let Err(error) = result {
        log::warn!("[rewards] pending receipt list could not be saved: {error:?}");
    }
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

#[cfg(target_arch = "wasm32")]
fn save_storage(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(target_arch = "wasm32")]
fn clear_storage(key: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.remove_item(key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_pending_reward_receipt_ids(_account_id: &str) -> std::collections::BTreeSet<String> {
    Default::default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_pending_reward_receipt_ids(
    _account_id: &str,
    _receipt_ids: &std::collections::BTreeSet<String>,
) {
}
