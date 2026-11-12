//! Run against an operator snapshot, never a live database. The original file
//! is copied; the check proves identities can change without changing progress.
use redb::ReadableTable;
use sow_data::{PlayerDb, auth::IdentityImport, redb_store::PLAYERS_TABLE};
use std::{collections::BTreeMap, error::Error, sync::Arc};

fn accounts(db: &redb::Database) -> Result<BTreeMap<String, serde_json::Value>, Box<dyn Error>> {
    let read = db.begin_read()?;
    let table = read.open_table(PLAYERS_TABLE)?;
    let mut result = BTreeMap::new();
    for entry in table.iter()? {
        let (key, raw) = entry?;
        let parsed: sow_data::PlayerAccount = serde_json::from_slice(raw.value())?;
        let mut account = serde_json::to_value(parsed)?;
        account
            .as_object_mut()
            .ok_or("invalid account")?
            .remove("linked_identities");
        result.insert(key.value().to_owned(), account);
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: check_identity_import SNAPSHOT.redb IMPORT.json".into());
    }
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("accounts.redb");
    std::fs::copy(&args[0], &path)?;
    let database = Arc::new(sow_data::init_database(path).map_err(|e| e.to_string())?);
    let before = accounts(&database)?;
    let db = PlayerDb::new("redis://127.0.0.1/", None, Some(database.clone()));
    let bundle: IdentityImport = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let imported = db
        .import_identity_bundle(&bundle)
        .map_err(|e| e.to_string())?;
    db.assert_local_identity_authority()
        .map_err(|e| e.to_string())?;
    let after = accounts(&database)?;
    if after != before {
        let mut fields = BTreeMap::<String, usize>::new();
        for (id, old) in &before {
            let new = after.get(id).ok_or("identity import deleted an account")?;
            for key in old.as_object().ok_or("invalid account")?.keys() {
                if old[key] == new[key] {
                    continue;
                }
                if let (Some(a), Some(b)) = (old[key].as_object(), new[key].as_object()) {
                    for field in a.keys().chain(b.keys()) {
                        if a.get(field) != b.get(field) {
                            *fields.entry(format!("{key}.{field}")).or_default() += 1;
                        }
                    }
                } else {
                    *fields.entry(key.to_owned()).or_default() += 1;
                }
            }
        }
        eprintln!("Changed field names and counts only: {fields:?}");
        return Err("identity import changed account data".into());
    }
    db.import_identity_bundle(&bundle)
        .map_err(|e| e.to_string())?;
    if accounts(&database)? != before {
        return Err("repeated import changed account data".into());
    }
    println!(
        "Import passed twice: {imported} imported, {} game accounts preserved",
        before.len()
    );
    Ok(())
}
