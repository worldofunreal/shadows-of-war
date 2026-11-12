//! Temporary, offline credential handover. This accepts proofs minted before
//! the operator snapshot, only for game accounts present in that snapshot.
use super::*;
use crate::redb_store::ACCOUNT_STORE_MIGRATIONS_TABLE;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize)]
pub struct ImportedRefresh {
    pub account_id: String,
    pub expires_at: u64,
}

#[derive(Serialize, Deserialize)]
pub struct SessionHandover {
    pub snapshot_at: u64,
    pub expires_at: u64,
    pub signing_key: String,
    pub accounts: BTreeMap<String, String>,
    pub refresh_hashes: BTreeMap<String, ImportedRefresh>,
    pub revoked_ids: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
struct PreviousClaims {
    sub: String,
    context: String,
    iat: u64,
    exp: u64,
    #[serde(default)]
    jti: String,
}

impl SessionHandover {
    pub fn validate(&self) -> Result<(), String> {
        if self.signing_key.len() < 32
            || self.snapshot_at > now() + 60
            || self.expires_at <= self.snapshot_at
            || self.expires_at - self.snapshot_at > 30 * 24 * 60 * 60
            || self
                .accounts
                .iter()
                .any(|(old, id)| old.is_empty() || !crate::profile::is_valid_account_id(id))
        {
            return Err("Invalid session handover configuration".into());
        }
        Ok(())
    }
}

impl AuthService {
    pub fn import_session(
        &self,
        access: Option<&str>,
        refresh: Option<&str>,
        import_id: &str,
    ) -> Result<(String, Tokens), String> {
        if import_id.len() != 64 || !import_id.bytes().all(|v| v.is_ascii_hexdigit()) {
            return Err("A saved migration nonce is required".into());
        }
        let config = self
            .handover
            .as_ref()
            .ok_or("Session migration is unavailable; sign in to restore this account")?;
        if config.expires_at <= now() {
            return Err("Session migration expired; sign in to restore this account".into());
        }
        let old = refresh
            .and_then(|proof| config.refresh_hashes.get(&digest(proof)))
            .filter(|row| row.expires_at > now())
            .map(|row| row.account_id.clone());
        let old = match old {
            Some(id) => id,
            None => {
                let token = access.ok_or("An existing session proof is required")?;
                let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
                validation.leeway = 0;
                let claims = jsonwebtoken::decode::<PreviousClaims>(
                    token,
                    &jsonwebtoken::DecodingKey::from_secret(config.signing_key.as_bytes()),
                    &validation,
                )
                .map_err(|_| "Invalid previous session proof")?
                .claims;
                if claims.context != "shadows_of_war"
                    || claims.iat > config.snapshot_at
                    || claims.exp <= now()
                    || config.revoked_ids.contains(&claims.jti)
                {
                    return Err("Previous session cannot be migrated".into());
                }
                claims.sub
            }
        };
        let id = config
            .accounts
            .get(&old)
            .ok_or("Previous session has no existing game account")?;
        use hmac::Mac;
        let derive = |domain: &str| {
            let mut mac = hmac::Hmac::<Sha256>::new_from_slice(self.secret.as_bytes())
                .expect("HMAC accepts any key length");
            mac.update(domain.as_bytes());
            mac.update(b"\0");
            mac.update(id.as_bytes());
            mac.update(b"\0");
            mac.update(import_id.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        let new_access = format!("sow_a_{}", derive("sow-handover-access"));
        let new_refresh = format!("sow_r_{}", derive("sow-handover-refresh"));
        let nonce_hash = digest(import_id);
        let database = self
            .db
            .redb_db
            .as_ref()
            .ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(storage_error)?;
        let proofs: Vec<_> = access
            .into_iter()
            .chain(refresh)
            .filter(|p| !p.is_empty())
            .map(|p| format!("session_handover:{}", digest(p)))
            .collect();
        {
            let mut used = txn
                .open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)
                .map_err(storage_error)?;
            let mut retry = false;
            for proof in &proofs {
                if let Some(previous) = used.get(proof.as_str()).map_err(storage_error)? {
                    if previous.value() != nonce_hash.as_bytes() {
                        return Err("Previous session was already migrated".into());
                    }
                    retry = true;
                }
            }
            if retry {
                // A lost HTTP response must not strand a guest. The same saved
                // nonce can recover the original session while it still exists.
                let table = txn.open_table(SESSIONS).map_err(storage_error)?;
                let session = table
                    .get(digest(&new_refresh).as_str())
                    .map_err(storage_error)?
                    .map(|row| serde_json::from_slice::<Session>(row.value()))
                    .transpose()
                    .map_err(storage_error)?
                    .ok_or("Migrated session was revoked or rotated")?;
                if session.account_id != *id || session.refresh_expires <= now() {
                    return Err("Migrated session expired".into());
                }
                return Ok((
                    id.clone(),
                    Tokens {
                        session_token: new_access,
                        refresh_token: new_refresh,
                        expires_at: session.access_expires,
                    },
                ));
            }
            for proof in &proofs {
                used.insert(proof.as_str(), nonce_hash.as_bytes())
                    .map_err(storage_error)?;
            }
        }
        let tokens = write_session_credentials(&txn, id, new_access, new_refresh)?;
        txn.commit().map_err(storage_error)?;
        Ok((id.clone(), tokens))
    }
}
