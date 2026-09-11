use egui::{Context, Rect, TextureHandle, TextureOptions};
use sow_data::emoji::{ATLAS_HEIGHT, ATLAS_WIDTH, lookup};

#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/", $path))
    };
}

pub static EMOJI_ATLAS_BYTES: &[u8] = repo_asset_bytes!("gameplay/emoji/atlas.webp");

const ATLAS_TEX_ID: &str = "sow_emoji_atlas";
const SHOP_TEX_ID: &str = "sow_shop_icon";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CurrencyIcon {
    Crown,
    Gem,
    Gold,
}

impl CurrencyIcon {
    fn texture_id(self) -> &'static str {
        match self {
            Self::Crown => "sow_currency_crown",
            Self::Gem => "sow_currency_gem",
            Self::Gold => "sow_currency_gold",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Crown => repo_asset_bytes!("gameplay/currency/crown.webp"),
            Self::Gem => repo_asset_bytes!("gameplay/currency/gem.webp"),
            Self::Gold => repo_asset_bytes!("gameplay/currency/gold.webp"),
        }
    }
}

pub fn currency_texture(ctx: &Context, icon: CurrencyIcon) -> Option<TextureHandle> {
    static_texture(ctx, icon.texture_id(), icon.bytes())
}

pub fn paint_currency_icon(
    painter: &egui::Painter,
    icon: CurrencyIcon,
    rect: Rect,
    tint: egui::Color32,
) -> bool {
    let Some(texture) = currency_texture(painter.ctx(), icon) else {
        return false;
    };
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
    true
}

pub fn shop_texture(ctx: &Context) -> Option<TextureHandle> {
    static_texture(
        ctx,
        SHOP_TEX_ID,
        repo_asset_bytes!("shell/mobile-nav/store.webp"),
    )
}

pub fn gem_bundle_texture(ctx: &Context, product_id: &str) -> Option<TextureHandle> {
    let (texture_id, bytes): (&'static str, &'static [u8]) = match product_id {
        "sow_gems_500" => (
            "sow_gem_bundle_500",
            repo_asset_bytes!("gameplay/store/gem_bundles/sow_gems_500.webp"),
        ),
        "sow_gems_1200" => (
            "sow_gem_bundle_1200",
            repo_asset_bytes!("gameplay/store/gem_bundles/sow_gems_1200.webp"),
        ),
        "sow_gems_2600" => (
            "sow_gem_bundle_2600",
            repo_asset_bytes!("gameplay/store/gem_bundles/sow_gems_2600.webp"),
        ),
        _ => return None,
    };
    static_texture(ctx, texture_id, bytes)
}

fn static_texture(
    ctx: &Context,
    texture_id: &'static str,
    bytes: &'static [u8],
) -> Option<TextureHandle> {
    let id = egui::Id::new(texture_id);
    if let Some(tex) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [rgba.width() as _, rgba.height() as _];
    let pixels = rgba.as_flat_samples();
    let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    let handle = ctx.load_texture(texture_id, color, texture_options());
    ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
    Some(handle)
}

pub fn atlas_texture(ctx: &Context) -> Option<TextureHandle> {
    let id = egui::Id::new(ATLAS_TEX_ID);
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let rgba = image::load_from_memory(EMOJI_ATLAS_BYTES).ok()?.to_rgba8();
    let size = [rgba.width() as _, rgba.height() as _];
    let pixels = rgba.as_flat_samples();
    let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    let handle = ctx.load_texture(ATLAS_TEX_ID, color, texture_options());
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

pub fn register_emoji_atlas(ctx: &Context) {
    let _ = atlas_texture(ctx);
}

pub fn register_game_assets(ctx: &Context) {
    register_emoji_atlas(ctx);
}

pub fn atlas_uv(emoji: &str) -> Option<Rect> {
    let rect = lookup(emoji).or_else(|| {
        let stripped = strip_fe0f(emoji);
        if stripped == emoji {
            None
        } else {
            lookup(stripped)
        }
    })?;
    Some(Rect::from_min_max(
        egui::pos2(
            rect.x as f32 / ATLAS_WIDTH as f32,
            rect.y as f32 / ATLAS_HEIGHT as f32,
        ),
        egui::pos2(
            (rect.x + rect.w) as f32 / ATLAS_WIDTH as f32,
            (rect.y + rect.h) as f32 / ATLAS_HEIGHT as f32,
        ),
    ))
}

fn strip_fe0f(emoji: &str) -> &str {
    if emoji.ends_with('\u{fe0f}') {
        &emoji[..emoji.len() - '\u{fe0f}'.len_utf8()]
    } else {
        emoji
    }
}

pub fn texture_options() -> TextureOptions {
    TextureOptions::LINEAR
}
