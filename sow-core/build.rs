//! Shared by native workers and WASM clients. Deployment identity belongs to
//! the server; simulation sources and shared simulation settings identify rules.
use std::{fs, path::Path};

fn generate_game_speed_config(root: &Path) -> Vec<u8> {
    use std::fmt::Write as _;

    let path = root.join("sow-dist/config/game-speed.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let source = fs::read(&path).expect("read shared game speed config");
    let config: serde_json::Value =
        serde_json::from_slice(&source).expect("parse shared game speed config");
    let normal = config["normal_game_speed"]
        .as_f64()
        .filter(|value| value.is_finite() && *value > 0.0)
        .expect("normal_game_speed must be a positive finite number");
    let presets = config["presets"]
        .as_array()
        .expect("presets must be an array");
    let expected = ["slowest", "slow", "normal", "fast", "fastest", "turbo"];
    assert_eq!(
        presets.len(),
        expected.len(),
        "expected six game speed presets"
    );

    let mut generated = format!(
        "/// Local fallback when a match does not choose another preset.\n\
         /// Its shared JSON value is also enforced for standard server matches.\n\
         pub const NORMAL_GAME_SPEED: f64 = {normal:?};\n"
    );
    generated.push_str("pub const GAME_SPEED_PRESETS: [(&str, f64); 6] = [\n");
    let mut previous_ratio = 0.0;
    for (preset, expected_key) in presets.iter().zip(expected) {
        let key = preset["key"].as_str().expect("preset key must be a string");
        let ratio = preset["ratio"]
            .as_f64()
            .filter(|value| value.is_finite() && *value > 0.0)
            .expect("preset ratio must be a positive finite number");
        assert_eq!(key, expected_key, "game speed preset order/name changed");
        assert!(ratio > previous_ratio, "game speed ratios must increase");
        if key == "normal" {
            assert_eq!(ratio, 1.0, "normal preset ratio must be 1.0");
        }
        previous_ratio = ratio;
        writeln!(
            &mut generated,
            "    ({key:?}, NORMAL_GAME_SPEED * {ratio:?}),"
        )
        .unwrap();
    }
    generated.push_str("];\n");
    let output = std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR");
    fs::write(Path::new(&output).join("game_speed.rs"), generated)
        .expect("write generated game speed config");
    source
}

fn source(root: &Path, path: &Path, hash: &mut blake3::Hasher) {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read simulation sources")
            .map(|entry| entry.expect("simulation source entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            source(root, &entry, hash);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension == "rs" || extension == "toml")
        // `*tests.rs` modules are #[cfg(test)] only and never reach a player.
        && !path.to_string_lossy().ends_with("tests.rs")
    {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .strip_prefix(root)
            .expect("simulation source is inside workspace")
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read(path).expect("read simulation source");
        hash.update(&(name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update(&(contents.len() as u64).to_le_bytes());
        hash.update(&contents);
    }
}

fn main() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("crate directory");
    let root = Path::new(&manifest).parent().expect("workspace directory");
    let game_speed_config = generate_game_speed_config(root);
    let mut hash = blake3::Hasher::new();
    hash.update(b"sow-simulation-rules-v1");
    source(root, &root.join("sow-core/src"), &mut hash);
    hash.update(b"sow-game-speed-config-v1");
    hash.update(&game_speed_config);
    source(root, &root.join("sow-core/Cargo.toml"), &mut hash);
    source(root, &root.join("sow-server/src/replay.rs"), &mut hash);
    // Only the sow-data the simulation reads. Prices, rewards and campaign progress are
    // server-authoritative: changing them must not force players onto a new client.
    for file in ["leaders.rs", "colors", "geo_entities"] {
        source(root, &root.join("sow-data/src").join(file), &mut hash);
    }
    // Which characters bots become, and where they live, is simulation input.
    // Economy and link fields are dropped by name, so a new field counts as a rule
    // until it is listed here: forgetting one forces an update, never a desync.
    let catalog = root.join("assets/geo_entities.json");
    println!("cargo:rerun-if-changed={}", catalog.display());
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog).expect("read character catalog"))
            .expect("parse character catalog");
    for entity in catalog["entities"].as_array_mut().expect("catalog entities") {
        let entity = entity.as_object_mut().expect("catalog entity");
        entity.remove("wikipedia");
        if let Some(hero) = entity.get_mut("hero").and_then(|hero| hero.as_object_mut()) {
            for field in ["rarity", "price_crowns", "free", "card"] {
                hero.remove(field);
            }
        }
    }
    hash.update(&serde_json::to_vec(&catalog).expect("encode character catalog"));
    let lock_path = root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    let lock = fs::read_to_string(lock_path).expect("read simulation dependency versions");
    for package in lock.split("[[package]]").skip(1) {
        let name = package
            .lines()
            .find_map(|line| {
                line.strip_prefix("name = \"")
                    .and_then(|value| value.strip_suffix('"'))
            })
            .expect("dependency name");
        if matches!(
            name,
            "serde"
                | "serde_core"
                | "serde_derive"
                | "serde_json"
                | "wyrand"
                | "libm"
                | "brotli"
                | "brotli-decompressor"
                | "bitfield"
        ) {
            hash.update(package.as_bytes());
        }
    }
    println!(
        "cargo:rustc-env=SOW_SIMULATION_RULES_ID={}",
        hash.finalize().to_hex()
    );
}
