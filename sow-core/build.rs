//! Shared by native workers and WASM clients. Deployment identity belongs to
//! the server; only simulation sources and their dependencies identify rules.
use std::{fs, path::Path};

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
    let mut hash = blake3::Hasher::new();
    hash.update(b"sow-simulation-rules-v1");
    source(root, &root.join("sow-core/src"), &mut hash);
    source(root, &root.join("sow-core/Cargo.toml"), &mut hash);
    source(root, &root.join("sow-server/src/replay.rs"), &mut hash);
    for file in [
        "leaders.rs",
        "commerce.rs",
        "rewards.rs",
        "colors",
        "geo_entities",
        "campaign.rs",
    ] {
        source(root, &root.join("sow-data/src").join(file), &mut hash);
    }
    // Which characters bots become, and where they live, is simulation input.
    let catalog = root.join("assets/geo_entities.json");
    println!("cargo:rerun-if-changed={}", catalog.display());
    hash.update(&fs::read(&catalog).expect("read character catalog"));
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
