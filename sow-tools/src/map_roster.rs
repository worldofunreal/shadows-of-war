use serde::Deserialize;
use sow_core::map_file::{self, MapRosterPreset};
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RosterSave {
    default_roster: Option<String>,
    rosters: Vec<MapRosterPreset>,
}

pub struct SaveMapRostersArgs {
    pub map: String,
    pub maps_root: PathBuf,
}

pub fn run(args: SaveMapRostersArgs) -> Result<(), Box<dyn Error>> {
    let key = sow_core::maps::map_key(&args.map);
    if key != args.map || key.is_empty() {
        return Err("invalid map key".into());
    }
    let mut body = Vec::new();
    std::io::stdin().read_to_end(&mut body)?;
    save_payload(&key, &args.maps_root, &body)
}

fn save_payload(key: &str, maps_root: &Path, body: &[u8]) -> Result<(), Box<dyn Error>> {
    let dir = maps_root.join(key);
    let bin_path = dir.join("map.bin");
    let current = fs::read(&bin_path)?;
    let mut map = map_file::parse(&current)?;
    let roster: RosterSave = serde_json::from_slice(body)?;
    map.default_roster = roster.default_roster;
    map.rosters = roster.rosters;
    map_file::validate_rosters(&map, true).map_err(|message| format!("{message}"))?;

    let encoded = map_file::encode(&map);
    let compressed = brotli_compress(&encoded)?;
    let old_compressed = fs::read(dir.join("map.bin.br")).ok();
    write_atomic(&bin_path, &encoded)?;
    if let Err(error) = write_atomic(&dir.join("map.bin.br"), &compressed) {
        write_atomic(&bin_path, &current)?;
        return Err(error.into());
    }
    if let Err(error) = super::map_source_import::refresh_catalog(maps_root) {
        write_atomic(&bin_path, &current)?;
        if let Some(old) = old_compressed {
            write_atomic(&dir.join("map.bin.br"), &old)?;
        } else {
            let _ = fs::remove_file(dir.join("map.bin.br"));
        }
        return Err(error);
    }
    Ok(())
}

fn brotli_compress(input: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 11, 22);
    writer.write_all(input)?;
    writer.flush()?;
    drop(writer);
    Ok(output)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("map");
    let temp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    fs::write(&temp, bytes)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::map_file::{MapFile, MapSpawn};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn saving_updates_raw_compressed_and_catalog_without_touching_anchors() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let maps_root =
            std::env::temp_dir().join(format!("sow-map-roster-{}-{nonce}", std::process::id()));
        let map_dir = maps_root.join("testmap");
        fs::create_dir_all(&map_dir).unwrap();

        let map = MapFile {
            display_name: "Test map".into(),
            width: 4,
            height: 2,
            num_land_tiles: 8,
            spawns: vec![MapSpawn {
                name: "Unlinked legacy anchor".into(),
                flag: String::new(),
                x: 0,
                y: 0,
            }],
            geo_bounds: None,
            default_roster: None,
            rosters: Vec::new(),
            terrain: vec![0x80; 8],
        };
        let anchors = map.spawns.clone();
        let original = map_file::encode(&map);
        fs::write(map_dir.join("map.bin"), &original).unwrap();
        fs::write(
            map_dir.join("map.bin.br"),
            brotli_compress(&original).unwrap(),
        )
        .unwrap();

        let request = br#"{"default_roster":"historical","rosters":[{"id":"historical","name":"Historical","entries":[{"entity_id":"iceland","role":"nation","x":2,"y":1,"legacy_anchor":0}]}]}"#;
        save_payload("testmap", &maps_root, request).unwrap();

        let raw = fs::read(map_dir.join("map.bin")).unwrap();
        let compressed = fs::read(map_dir.join("map.bin.br")).unwrap();
        let from_raw = map_file::parse(&raw).unwrap();
        let from_compressed =
            map_file::parse(&map_file::decompress_map_payload(&compressed).unwrap()).unwrap();
        assert_eq!(from_raw.rosters, from_compressed.rosters);
        assert_eq!(from_raw.default_roster.as_deref(), Some("historical"));
        assert_eq!(from_raw.spawns, anchors);
        assert_eq!(from_raw.rosters[0].entries[0].x, 2);
        assert_eq!(from_raw.rosters[0].entries[0].y, 1);
        let catalog =
            map_file::parse_catalog(&fs::read(maps_root.join("catalog.bin")).unwrap()).unwrap();
        assert_eq!(
            catalog.entries[0].default_roster.as_deref(),
            Some("historical")
        );
        assert!(!map_dir.join("roster.json").exists());

        fs::remove_dir_all(maps_root).unwrap();
    }
}
