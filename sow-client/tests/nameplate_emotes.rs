#![allow(dead_code)]

mod map_cache {
    pub fn load(_: &str) -> Option<Vec<u8>> {
        None
    }
}

#[path = "../src/render/motion.rs"]
mod motion;

#[path = "../src/ui/asset_loader/mod.rs"]
mod asset_loader;
