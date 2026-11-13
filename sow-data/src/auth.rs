//! First-party identity on the existing account database. Provider SDKs keep
//! their own proof formats; website credentials are opaque, rotating tokens.
use crate::db::{AccountKind, PlayerAccount, PlayerDb};
use crate::redb_store::PLAYERS_TABLE;
use rand::RngCore;
use redb::{ReadableTable, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod http;
mod oauth;
pub use http::router;
pub use oauth::public_host;

pub const SESSIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("auth_sessions");
const ACCESS_SECONDS: u64 = 15 * 60;
const REFRESH_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone)]
pub struct AuthService {
    pub(crate) db: PlayerDb,
    pub(crate) secret: String,
    pub(crate) http: reqwest::Client,
}

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    account_id: String,
    access_key: String,
    refresh_key: String,
    access_expires: u64,
    refresh_expires: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Tokens {
    pub session_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub status: &'static str,
    pub account: serde_json::Value,
    #[serde(flatten)]
    pub tokens: Tokens,
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub(crate) fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn storage_error(e: impl std::fmt::Display) -> String {
    format!("Identity storage unavailable: {e}")
}

pub fn account_card(account: &PlayerAccount) -> serde_json::Value {
    serde_json::json!({
        "account_id": account.id,
        "display_name": account.display_name,
        "linked_identities": account.linked_identities.iter()
            .map(|identity| serde_json::json!({"provider": identity.provider}))
            .collect::<Vec<_>>(),
    })
}

pub fn verify_session(database: &redb::Database, token: &str) -> Result<String, String> {
    if !token.starts_with("sow_a_") || token.len() != 70 {
        return Err("Invalid session".into());
    }
    let read = database.begin_read().map_err(storage_error)?;
    let sessions = read.open_table(SESSIONS).map_err(storage_error)?;
    let key = digest(token);
    let value = sessions
        .get(key.as_str())
        .map_err(storage_error)?
        .ok_or("Session expired or revoked")?;
    let session: Session = serde_json::from_slice(value.value()).map_err(storage_error)?;
    if session.access_key != key || session.access_expires <= now() {
        return Err("Session expired or revoked".into());
    }
    // Deleting an account invalidates its proof even if an old row survives
    // an interrupted cleanup. Account IDs are never accepted as credentials.
    if read
        .open_table(PLAYERS_TABLE)
        .map_err(storage_error)?
        .get(session.account_id.as_str())
        .map_err(storage_error)?
        .is_none()
    {
        return Err("Session account no longer exists".into());
    }
    Ok(session.account_id)
}

fn write_session(txn: &WriteTransaction, account_id: &str) -> Result<Tokens, String> {
    write_session_credentials(
        txn,
        account_id,
        format!("sow_a_{}", random_token()),
        format!("sow_r_{}", random_token()),
    )
}

fn write_session_credentials(
    txn: &WriteTransaction,
    account_id: &str,
    access: String,
    refresh: String,
) -> Result<Tokens, String> {
    let account = txn
        .open_table(PLAYERS_TABLE)
        .map_err(storage_error)?
        .get(account_id)
        .map_err(storage_error)?
        .map(|value| crate::db::parse_account(value.value()))
        .transpose()
        .map_err(storage_error)?
        .ok_or("Account does not exist")?;
    if account.kind != AccountKind::Human {
        return Err("A bot cannot obtain a player session".into());
    }
    let session = Session {
        account_id: account_id.to_owned(),
        access_key: digest(&access),
        refresh_key: digest(&refresh),
        access_expires: now() + ACCESS_SECONDS,
        refresh_expires: now() + REFRESH_SECONDS,
    };
    let raw = serde_json::to_vec(&session).map_err(storage_error)?;
    let mut sessions = txn.open_table(SESSIONS).map_err(storage_error)?;
    sessions
        .insert(session.access_key.as_str(), raw.as_slice())
        .map_err(storage_error)?;
    sessions
        .insert(session.refresh_key.as_str(), raw.as_slice())
        .map_err(storage_error)?;
    Ok(Tokens {
        session_token: access,
        refresh_token: refresh,
        expires_at: session.access_expires,
    })
}

pub(crate) fn delete_account_sessions(
    txn: &WriteTransaction,
    account_id: &str,
) -> Result<u32, String> {
    let mut sessions = txn.open_table(SESSIONS).map_err(storage_error)?;
    let keys = sessions
        .iter()
        .map_err(storage_error)?
        .map(|entry| {
            let (key, value) = entry.map_err(storage_error)?;
            let session: Session = serde_json::from_slice(value.value()).map_err(storage_error)?;
            Ok((key.value().to_owned(), session.account_id == account_id))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut removed = 0;
    for (key, belongs) in keys {
        if belongs {
            sessions.remove(key.as_str()).map_err(storage_error)?;
            removed += 1;
        }
    }
    Ok(removed)
}

impl AuthService {
    pub fn new(db: PlayerDb, secret: String) -> Result<Self, String> {
        if secret.len() < 32 {
            return Err("The identity signing key is not configured".into());
        }
        log::info!("[auth] ensuring sessions table");
        let database = db.redb_db.as_ref().ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(|e| format!("begin_write failed for sessions table: {e}"))?;
        txn.open_table(SESSIONS).map_err(|e| format!("open_table(SESSIONS) failed: {e}"))?;
        txn.commit().map_err(|e| format!("commit failed for sessions table: {e}"))?;
        log::info!("[auth] initializing auth HTTP client");
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("build auth HTTP client failed: {e}"))?;
        log::info!("[auth] auth initialized");
        Ok(Self {
            db,
            secret,
            http,
        })
    }

    pub fn verify_token(&self, token: &str) -> Result<String, String> {
        verify_session(
            self.db
                .redb_db
                .as_ref()
                .ok_or("Account database unavailable")?,
            token,
        )
    }

    pub fn issue_tokens(&self, account_id: &str) -> Result<Tokens, String> {
        let database = self
            .db
            .redb_db
            .as_ref()
            .ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(storage_error)?;
        let tokens = write_session(&txn, account_id)?;
        txn.commit().map_err(storage_error)?;
        Ok(tokens)
    }

    pub fn refresh(&self, token: &str) -> Result<(String, Tokens), String> {
        if !token.starts_with("sow_r_") || token.len() != 70 {
            return Err("Invalid refresh credential".into());
        }
        let database = self
            .db
            .redb_db
            .as_ref()
            .ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(storage_error)?;
        let key = digest(token);
        let session: Session = {
            let mut sessions = txn.open_table(SESSIONS).map_err(storage_error)?;
            let raw = sessions
                .get(key.as_str())
                .map_err(storage_error)?
                .map(|value| value.value().to_vec())
                .ok_or("Refresh credential expired or revoked")?;
            let session: Session = serde_json::from_slice(&raw).map_err(storage_error)?;
            if session.refresh_key != key || session.refresh_expires <= now() {
                return Err("Refresh credential expired or revoked".into());
            }
            sessions
                .remove(session.access_key.as_str())
                .map_err(storage_error)?;
            sessions
                .remove(session.refresh_key.as_str())
                .map_err(storage_error)?;
            session
        };
        let tokens = write_session(&txn, &session.account_id)?;
        txn.commit().map_err(storage_error)?;
        Ok((session.account_id, tokens))
    }

    pub fn logout(&self, token: &str) -> Result<(), String> {
        let database = self
            .db
            .redb_db
            .as_ref()
            .ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(storage_error)?;
        {
            let mut sessions = txn.open_table(SESSIONS).map_err(storage_error)?;
            let raw = sessions
                .get(digest(token).as_str())
                .map_err(storage_error)?
                .map(|value| value.value().to_vec());
            if let Some(raw) = raw {
                let session: Session = serde_json::from_slice(&raw).map_err(storage_error)?;
                sessions
                    .remove(session.access_key.as_str())
                    .map_err(storage_error)?;
                sessions
                    .remove(session.refresh_key.as_str())
                    .map_err(storage_error)?;
            }
        }
        txn.commit().map_err(storage_error)?;
        Ok(())
    }

    pub fn prune(&self) -> Result<usize, String> {
        let database = self
            .db
            .redb_db
            .as_ref()
            .ok_or("Account database unavailable")?;
        let txn = database.begin_write().map_err(storage_error)?;
        let removed = {
            let mut sessions = txn.open_table(SESSIONS).map_err(storage_error)?;
            let expired = sessions
                .iter()
                .map_err(storage_error)?
                .map(|entry| {
                    let (key, value) = entry.map_err(storage_error)?;
                    let session: Session =
                        serde_json::from_slice(value.value()).map_err(storage_error)?;
                    Ok((key.value().to_owned(), session.refresh_expires <= now()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let mut count = 0;
            for (key, expired) in expired {
                if expired {
                    sessions.remove(key.as_str()).map_err(storage_error)?;
                    count += 1;
                }
            }
            count
        };
        txn.commit().map_err(storage_error)?;
        Ok(removed)
    }

    pub async fn response(&self, account_id: &str, tokens: Tokens) -> Result<AuthResponse, String> {
        let account = self
            .db
            .account_by_id(account_id)
            .await
            .map_err(storage_error)?;
        Ok(AuthResponse {
            status: "success",
            account: account_card(&account),
            tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn service() -> (tempfile::TempDir, AuthService) {
        let temp = tempfile::tempdir().unwrap();
        let database = std::sync::Arc::new(
            crate::redb_store::init_database(temp.path().join("accounts.redb")).unwrap(),
        );
        let db = PlayerDb::new("redis://127.0.0.1/", None, Some(database));
        (
            temp,
            AuthService::new(db, "test-only-auth-secret-never-use-in-production".into()).unwrap(),
        )
    }

    #[tokio::test]
    async fn session_rotation_revocation_and_account_deletion() {
        let (_temp, auth) = service();
        let account = auth
            .db
            .get_or_create_anonymous(None, None, None)
            .await
            .unwrap();
        let original = auth.issue_tokens(&account.id).unwrap();
        assert_eq!(
            auth.verify_token(&original.session_token).unwrap(),
            account.id
        );
        assert!(auth.verify_token(&original.refresh_token).is_err());
        let (id, rotated) = auth.refresh(&original.refresh_token).unwrap();
        assert_eq!(id, account.id);
        assert!(auth.verify_token(&original.session_token).is_err());
        assert!(auth.refresh(&original.refresh_token).is_err());
        assert_eq!(
            auth.verify_token(&rotated.session_token).unwrap(),
            account.id
        );
        let database = auth.db.redb_db.as_ref().unwrap();
        let read = database.begin_read().unwrap();
        let sessions = read.open_table(SESSIONS).unwrap();
        assert!(
            sessions
                .get(rotated.session_token.as_str())
                .unwrap()
                .is_none()
        );
        drop(sessions);
        drop(read);
        auth.logout(&rotated.session_token).unwrap();
        assert!(auth.verify_token(&rotated.session_token).is_err());
        assert!(auth.refresh(&rotated.refresh_token).is_err());
        let last = auth.issue_tokens(&account.id).unwrap();
        let txn = database.begin_write().unwrap();
        assert_eq!(delete_account_sessions(&txn, &account.id).unwrap(), 2);
        txn.open_table(PLAYERS_TABLE)
            .unwrap()
            .remove(account.id.as_str())
            .unwrap();
        txn.commit().unwrap();
        assert!(auth.verify_token(&last.session_token).is_err());
        assert!(auth.issue_tokens(&account.id).is_err());
    }

    #[tokio::test]
    async fn verified_links_preserve_progress_and_reject_conflicting_accounts_atomically() {
        let (_temp, auth) = service();
        let guest = auth
            .db
            .get_or_create_anonymous(None, None, None)
            .await
            .unwrap();
        let profile = serde_json::to_value(&guest.profile).unwrap();
        let google = crate::db::LinkedIdentity {
            provider: "google".into(),
            environment: "production".into(),
            external_id: "provider-subject".into(),
        };
        let email = crate::db::LinkedIdentity {
            provider: "email".into(),
            environment: "production".into(),
            external_id: "account@example.test".into(),
        };
        let linked = auth
            .db
            .resolve_verified_identities(&[google.clone(), email.clone()], Some(&guest.id))
            .await
            .unwrap();
        assert_eq!(linked.id, guest.id);
        assert_eq!(serde_json::to_value(&linked.profile).unwrap(), profile);
        let restored = auth
            .db
            .resolve_verified_identities(&[google.clone()], None)
            .await
            .unwrap();
        assert_eq!(restored.id, guest.id);
        let other = auth
            .db
            .resolve_verified_identity("discord", "production", "another-subject", None)
            .await
            .unwrap();
        let conflict = crate::db::LinkedIdentity {
            provider: "discord".into(),
            environment: "production".into(),
            external_id: "another-subject".into(),
        };
        assert!(
            auth.db
                .resolve_verified_identities(&[email, conflict], None)
                .await
                .is_err()
        );
        assert_eq!(
            auth.db
                .resolve_verified_identities(&[google], None)
                .await
                .unwrap()
                .id,
            guest.id
        );
        assert_eq!(
            auth.db
                .account_by_id(&other.id)
                .await
                .unwrap()
                .linked_identities
                .len(),
            1
        );
    }

}
