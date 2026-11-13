//! Text, building and nameplate-status atlases. The web loader (`loader.ts`) downloads them as
//! cached files after the engine and decodes them in the browser, so the engine binary does not
//! carry them and a deploy does not download them again.

use wasm_bindgen::{JsCast, JsValue};

/// The decoded atlases, or `None` while the loader is still fetching them. Asking again while
/// they are missing (re)starts the download; the loader dedupes and spaces out retries. The request
/// does not wait for the first loader to finish: a match can be what keeps that loader open.
pub(crate) fn get() -> Option<super::gpu::TextAtlasImages> {
    let window: JsValue = web_sys::window()?.into();
    let atlases = js_sys::Reflect::get(&window, &"SOW_TEXT_ATLASES".into()).ok()?;
    if atlases.is_undefined() || atlases.is_null() {
        if let Ok(load) = js_sys::Reflect::get(&window, &"SOW_loadTextAtlases".into())
            && let Some(load) = load.dyn_ref::<js_sys::Function>()
        {
            let _ = load.call0(&window);
        }
        return None;
    }
    let bitmap = |key: &str| {
        js_sys::Reflect::get(&atlases, &key.into())
            .ok()?
            .dyn_into::<web_sys::ImageBitmap>()
            .ok()
    };
    Some(super::gpu::TextAtlasImages {
        text: bitmap("text")?,
        buildings: bitmap("buildings")?,
        badges: bitmap("badges")?,
    })
}

/// Install the Arabic shaping font once a name has asked for it. The loader fetches it on demand
/// (`SOW_loadArabicFont`), so players who never see Arabic text never download it.
pub(crate) fn poll_arabic_font() {
    // A file that does not parse is not fetched again this session.
    static UNUSABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !super::gpu::arabic_font_wanted() || UNUSABLE.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let window: JsValue = window.into();
    let key: JsValue = "SOW_ARABIC_FONT".into();
    match js_sys::Reflect::get(&window, &key) {
        Ok(buffer) if buffer.is_instance_of::<js_sys::ArrayBuffer>() => {
            let _ = js_sys::Reflect::delete_property(window.unchecked_ref::<js_sys::Object>(), &key);
            if !super::gpu::set_arabic_font(js_sys::Uint8Array::new(&buffer).to_vec()) {
                UNUSABLE.store(true, std::sync::atomic::Ordering::Relaxed);
                log::error!("Arabic font download is not a usable font");
            }
        }
        _ => {
            if let Ok(load) = js_sys::Reflect::get(&window, &"SOW_loadArabicFont".into())
                && let Some(load) = load.dyn_ref::<js_sys::Function>()
            {
                let _ = load.call0(&window);
            }
        }
    }
}
