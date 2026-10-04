//! Partner portal SDK hooks (Poki / CrazyGames). No-op when JS helpers are absent.

use crate::platform_identity::PlatformIdentity;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

#[cfg(target_arch = "wasm32")]
fn get_window_value(name: &str) -> Option<wasm_bindgen::JsValue> {
    let w = window()?;
    js_sys::Reflect::get(&w, &wasm_bindgen::JsValue::from_str(name)).ok()
}

#[cfg(target_arch = "wasm32")]
fn call_window_hook(name: &str) {
    let Some(val) = get_window_value(name) else {
        return;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return;
        };
        let _ = func.call0(&wasm_bindgen::JsValue::NULL);
    }
}

#[cfg(target_arch = "wasm32")]
fn call_window_hook_str(name: &str, arg: &str) {
    let Some(val) = get_window_value(name) else {
        return;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return;
        };
        let _ = func.call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_str(arg),
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn take_window_u64(name: &str) -> Option<u64> {
    let val = get_window_value(name)?;
    if val.is_null() || val.is_undefined() {
        return None;
    }
    if let Some(s) = val.as_string() {
        return s.parse().ok();
    }
    val.as_f64().map(|n| n as u64)
}

#[cfg(target_arch = "wasm32")]
fn take_window_bool(name: &str) -> bool {
    get_window_value(name)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn gameplay_start() {
    call_window_hook_str("SOW_portalUpdateRoom", &json);
}

pub fn left_room() {
    call_window_hook("SOW_portalLeftRoom");
}

pub fn happytime() {
    call_window_hook("SOW_portalHappytime");
}
pub fn show_auth_prompt() {
    call_window_hook("SOW_portalShowAuthPrompt");
}

pub fn sign_out() {
    call_window_hook("SOW_portalSignOut");
}

pub fn poll_auth_changed() -> bool {
    let changed = take_window_bool("SOW_AUTH_CHANGED");
    if changed {
        call_window_hook("SOW_portalClearAuthChanged");
    }
    changed
}

pub fn is_signed_in_crazygames() -> bool {
    let identity = load_identity("Player");
    identity.provider == "crazygames"
        && identity
            .auth_token
            .as_ref()
            .is_some_and(|token| !token.is_empty())
}

// Leaderboard scores are submitted server-side on match finalize (sow-data
// posts to leaderboard.crazygames.com with the deployment API key). The old
// client-side submitScore path used a placeholder encryption key and never
// worked — removed.
