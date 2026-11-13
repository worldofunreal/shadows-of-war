use super::oauth;
use super::{AuthResponse, AuthService, account_card, digest, random_token};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type Failure = (StatusCode, Json<serde_json::Value>);
type Result<T> = std::result::Result<T, Failure>;
fn failure(status: StatusCode, message: impl AsRef<str>) -> Failure {
    (status, Json(serde_json::json!({"error":message.as_ref()})))
}
fn unavailable(_: impl std::fmt::Display) -> Failure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "Identity service is temporarily unavailable",
    )
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
fn binding(
    auth: &AuthService,
    headers: &HeaderMap,
    requested: Option<&str>,
) -> Result<Option<String>> {
    let Some(token) = bearer(headers) else {
        return Ok(None);
    };
    let owner = auth
        .verify_token(token)
        .map_err(|_| failure(StatusCode::UNAUTHORIZED, "Session expired or revoked"))?;
    if requested.is_some_and(|id| id != owner) {
        return Err(failure(
            StatusCode::UNAUTHORIZED,
            "Account does not match this session",
        ));
    }
    Ok(Some(owner))
}
fn email(value: &str) -> Result<String> {
    let email = value.trim().to_lowercase();
    if email.len() > 254 || email.parse::<lettre::Address>().is_err() {
        return Err(failure(StatusCode::BAD_REQUEST, "Invalid email address"));
    }
    Ok(email)
}

impl AuthService {
    async fn throttle(&self, key: &str, limit: u32, seconds: u32) -> Result<()> {
        let mut connection = self.db.get_connection().await.map_err(unavailable)?;
        let count: u32 = redis::Script::new("local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]) end; return n")
            .key(format!("auth:rate:{key}")).arg(seconds).invoke_async(&mut connection).await.map_err(unavailable)?;
        if count > limit {
            return Err(failure(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many sign-in attempts; try again later",
            ));
        }
        Ok(())
    }
    async fn ip_limit(
        &self,
        headers: &HeaderMap,
        operation: &str,
        limit: u32,
        seconds: u32,
    ) -> Result<()> {
        let ip = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<std::net::IpAddr>().ok())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "local".into());
        self.throttle(&format!("{operation}:{}", digest(&ip)), limit, seconds)
            .await
    }
    async fn cache(&self, key: &str, value: &str, seconds: u32) -> Result<()> {
        let mut connection = self.db.get_connection().await.map_err(unavailable)?;
        redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(seconds)
            .query_async::<()>(&mut connection)
            .await
            .map_err(unavailable)
    }
    async fn take(&self, key: &str) -> Result<Option<String>> {
        let mut connection = self.db.get_connection().await.map_err(unavailable)?;
        redis::cmd("GETDEL")
            .arg(key)
            .query_async(&mut connection)
            .await
            .map_err(unavailable)
    }
    fn otp_mac(&self, email: &str, code: &str) -> Hmac<Sha256> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(b"sow-auth-email-otp\0");
        mac.update(email.as_bytes());
        mac.update(b"\0");
        mac.update(code.as_bytes());
        mac
    }
    async fn login(
        &self,
        provider: &str,
        subject: &str,
        verified_email: Option<&str>,
        owner: Option<&str>,
    ) -> Result<AuthResponse> {
        let mut identities = vec![crate::db::LinkedIdentity {
            provider: provider.to_owned(),
            environment: "production".into(),
            external_id: subject.to_owned(),
        }];
        if let Some(address) = verified_email {
            let address = email(address)?;
            if provider != "email" {
                identities.push(crate::db::LinkedIdentity {
                    provider: "email".into(),
                    environment: "production".into(),
                    external_id: address,
                });
            }
        }
        let account = self
            .db
            .resolve_verified_identities(&identities, owner)
            .await
            .map_err(|_| {
                failure(
                    StatusCode::CONFLICT,
                    "These verified identities cannot be linked to the same account",
                )
            })?;
        let tokens = self.issue_tokens(&account.id).map_err(unavailable)?;
        Ok(AuthResponse {
            status: "success",
            account: account_card(&account),
            tokens,
        })
    }
}

#[derive(Deserialize)]
struct AnonymousRequest {
    display_name: Option<String>,
    account_id: Option<String>,
    auth_secret: Option<String>,
}
async fn anonymous(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(payload): Json<AnonymousRequest>,
) -> Result<Json<AuthResponse>> {
    auth.ip_limit(&headers, "anonymous", 120, 3600).await?;
    let account = if let Some(id) = payload.account_id.as_deref() {
        let proof = payload
            .auth_secret
            .as_deref()
            .ok_or_else(|| failure(StatusCode::UNAUTHORIZED, "Account ownership proof required"))?;
        let id = auth
            .db
            .verify_anonymous_secret(id, proof)
            .await
            .map_err(|_| failure(StatusCode::UNAUTHORIZED, "Account ownership proof failed"))?;
        auth.db.account_by_id(&id).await.map_err(unavailable)?
    } else {
        auth.db
            .get_or_create_anonymous(None, payload.display_name.as_deref(), None)
            .await
            .map_err(unavailable)?
    };
    let tokens = auth.issue_tokens(&account.id).map_err(unavailable)?;
    Ok(Json(AuthResponse {
        status: "success",
        account: account_card(&account),
        tokens,
    }))
}
async fn me(
    State(auth): State<AuthService>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let id = binding(&auth, &headers, None)?
        .ok_or_else(|| failure(StatusCode::UNAUTHORIZED, "Sign-in required"))?;
    let account = auth.db.account_by_id(&id).await.map_err(unavailable)?;
    Ok(Json(account_card(&account)))
}
#[derive(Deserialize, Default)]
struct RefreshRequest {
    refresh_token: Option<String>,
}
async fn refresh(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>> {
    auth.ip_limit(&headers, "refresh", 300, 60).await?;
    let token = payload
        .refresh_token
        .as_deref()
        .ok_or_else(|| failure(StatusCode::UNAUTHORIZED, "Refresh credential required"))?;
    let (id, tokens) = auth.refresh(token).map_err(|_| {
        failure(
            StatusCode::UNAUTHORIZED,
            "Refresh credential expired or revoked",
        )
    })?;
    Ok(Json(auth.response(&id, tokens).await.map_err(unavailable)?))
}
async fn logout(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> Result<StatusCode> {
    if let Some(token) = bearer(&headers).or(payload.refresh_token.as_deref()) {
        auth.logout(token).map_err(unavailable)?;
    }
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
struct OtpRequest {
    email: String,
    account_id: Option<String>,
    code: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct PendingOtp {
    mac: String,
    account_id: Option<String>,
}
async fn request_otp(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(payload): Json<OtpRequest>,
) -> Result<Json<serde_json::Value>> {
    let email = email(&payload.email)?;
    let owner = binding(&auth, &headers, payload.account_id.as_deref())?;
    auth.ip_limit(&headers, "otp-send", 60, 3600).await?;
    auth.throttle("otp-global", 300, 60).await?;
    auth.throttle(&format!("otp-email:{}", digest(&email)), 3, 900)
        .await?;
    let code = format!("{:06}", rand::rngs::OsRng.gen_range(0..1_000_000));
    let record = PendingOtp {
        mac: hex::encode(auth.otp_mac(&email, &code).finalize().into_bytes()),
        account_id: owner,
    };
    let key = format!("auth:otp:{}", digest(&email));
    auth.cache(
        &key,
        &serde_json::to_string(&record).map_err(unavailable)?,
        600,
    )
    .await?;
    let body = format!(
        "Your Shadows of War sign-in code is:\n\n{code}\n\nIt expires in 10 minutes. If you did not request it, ignore this email. Never share this code."
    );
    if crate::mail::send_message(&email, "Your Shadows of War sign-in code", body)
        .await
        .is_err()
    {
        auth.take(&key).await?;
        return Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Email delivery is temporarily unavailable",
        ));
    }
    Ok(Json(
        serde_json::json!({"status":"success","expires_in_seconds":600}),
    ))
}
async fn verify_otp(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Json(payload): Json<OtpRequest>,
) -> Result<Json<AuthResponse>> {
    let email = email(&payload.email)?;
    let code = payload.code.as_deref().unwrap_or("");
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(failure(StatusCode::BAD_REQUEST, "Enter the six-digit code"));
    }
    auth.ip_limit(&headers, "otp-check", 60, 60).await?;
    let key = format!("auth:otp:{}", digest(&email));
    let mut connection = auth.db.get_connection().await.map_err(unavailable)?;
    let raw:Option<String>=redis::Script::new("local v=redis.call('GET',KEYS[1]); if not v then return nil end; local n=redis.call('INCR',KEYS[2]); if n==1 then redis.call('EXPIRE',KEYS[2],600) end; if n>5 then redis.call('DEL',KEYS[1]); return nil end; return v")
        .key(&key).key(format!("{key}:attempts")).invoke_async(&mut connection).await.map_err(unavailable)?;
    let raw = raw.ok_or_else(|| failure(StatusCode::UNAUTHORIZED, "Code expired or invalid"))?;
    let pending: PendingOtp = serde_json::from_str(&raw).map_err(unavailable)?;
    let expected = hex::decode(&pending.mac).map_err(unavailable)?;
    if auth.otp_mac(&email, code).verify_slice(&expected).is_err() {
        return Err(failure(StatusCode::UNAUTHORIZED, "Code expired or invalid"));
    }
    let owner = binding(&auth, &headers, payload.account_id.as_deref())?;
    if pending.account_id.is_some() && pending.account_id != owner {
        return Err(failure(
            StatusCode::UNAUTHORIZED,
            "Sign-in session changed; request another code",
        ));
    }
    if auth.take(&key).await?.as_deref() != Some(raw.as_str()) {
        return Err(failure(StatusCode::UNAUTHORIZED, "Code was already used"));
    }
    Ok(Json(
        auth.login("email", &email, Some(&email), owner.as_deref())
            .await?,
    ))
}

#[derive(Deserialize)]
struct OAuthQuery {
    redirect_uri: String,
    state: String,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct OAuthState {
    provider: String,
    redirect: String,
    challenge: Option<String>,
    browser_digest: String,
}
#[derive(Deserialize)]
struct OAuthCallback {
    code: String,
    redirect_uri: String,
    state: String,
    code_verifier: Option<String>,
    account_id: Option<String>,
}
fn cookie_name(state: &str) -> String {
    format!("sow_oauth_{}", &digest(state)[..24])
}
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|item| {
            let (key, value) = item.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}
fn validate_callback(
    record: &OAuthState,
    provider: &str,
    payload: &OAuthCallback,
    browser: &str,
) -> Result<()> {
    if record.provider != provider
        || record.redirect != payload.redirect_uri
        || record.browser_digest != digest(browser)
    {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "OAuth transaction does not match this browser",
        ));
    }
    if let Some(challenge) = record.challenge.as_deref() {
        let verifier = payload.code_verifier.as_deref().unwrap_or("");
        if !(43..=128).contains(&verifier.len())
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
            || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != challenge
        {
            return Err(failure(StatusCode::BAD_REQUEST, "Invalid OAuth PKCE proof"));
        }
    }
    Ok(())
}
async fn oauth_config() -> Json<serde_json::Value> {
    let callback = format!("https://{}/auth/callback", oauth::public_host());
    Json(serde_json::json!({"hub_callback": callback.clone(), "game_callbacks": [callback]}))
}
async fn start_oauth(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Query(query): Query<OAuthQuery>,
) -> Result<Response> {
    if !oauth::allowed_redirect(&query.redirect_uri)
        || query.state.is_empty()
        || query.state.len() > 4096
    {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "Invalid OAuth callback or state",
        ));
    }
    auth.ip_limit(&headers, "oauth-start", 60, 60).await?;
    let (client_id, _) = oauth::credentials(&provider).map_err(|_| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Social provider is not configured",
        )
    })?;
    if provider == "twitter"
        && (query.code_challenge.as_ref().is_none_or(|v| v.len() != 43)
            || query.code_challenge_method.as_deref() != Some("S256"))
    {
        return Err(failure(StatusCode::BAD_REQUEST, "X requires S256 PKCE"));
    }
    let browser = random_token();
    let record = OAuthState {
        provider: provider.clone(),
        redirect: query.redirect_uri.clone(),
        challenge: query.code_challenge.clone(),
        browser_digest: digest(&browser),
    };
    auth.cache(
        &format!("auth:oauth:{}", digest(&query.state)),
        &serde_json::to_string(&record).map_err(unavailable)?,
        600,
    )
    .await?;
    let target = oauth::authorization_url(
        &provider,
        &client_id,
        &query.redirect_uri,
        &query.state,
        query.code_challenge.as_deref(),
    )
    .map_err(|e| failure(StatusCode::BAD_REQUEST, e))?;
    let mut response = Redirect::temporary(&target).into_response();
    let secure = if query.redirect_uri.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(header::SET_COOKIE,HeaderValue::from_str(&format!("{}={browser}; Path=/api/auth/oauth/callback; Max-Age=600; HttpOnly; SameSite=Lax{secure}",cookie_name(&query.state))).map_err(unavailable)?);
    Ok(response)
}
async fn finish_oauth(
    State(auth): State<AuthService>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Json(payload): Json<OAuthCallback>,
) -> Result<Response> {
    if payload.state.is_empty()
        || payload.state.len() > 4096
        || payload.code.is_empty()
        || payload.code.len() > 8192
        || !oauth::allowed_redirect(&payload.redirect_uri)
    {
        return Err(failure(StatusCode::BAD_REQUEST, "Invalid OAuth callback"));
    }
    auth.ip_limit(&headers, "oauth-callback", 60, 60).await?;
    let name = cookie_name(&payload.state);
    let browser = cookie(&headers, &name)
        .ok_or_else(|| failure(StatusCode::BAD_REQUEST, "OAuth browser proof is missing"))?;
    let key = format!("auth:oauth:{}", digest(&payload.state));
    let mut connection = auth.db.get_connection().await.map_err(unavailable)?;
    let raw: Option<String> = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut connection)
        .await
        .map_err(unavailable)?;
    let raw = raw.ok_or_else(|| {
        failure(
            StatusCode::BAD_REQUEST,
            "OAuth state expired or was already used",
        )
    })?;
    let record: OAuthState = serde_json::from_str(&raw).map_err(unavailable)?;
    validate_callback(&record, &provider, &payload, browser)?;
    let owner = binding(&auth, &headers, payload.account_id.as_deref())?;
    if auth.take(&key).await?.as_deref() != Some(raw.as_str()) {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "OAuth state was already used",
        ));
    }
    let identity = auth
        .exchange_social(
            &provider,
            &payload.code,
            &payload.redirect_uri,
            payload.code_verifier.as_deref(),
        )
        .await
        .map_err(|_| {
            failure(
                StatusCode::UNAUTHORIZED,
                "Social sign-in could not be verified",
            )
        })?;
    let mut response = Json(
        auth.login(
            &provider,
            &identity.subject,
            identity.verified_email.as_deref(),
            owner.as_deref(),
        )
        .await?,
    )
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{name}=; Path=/api/auth/oauth/callback; Max-Age=0; HttpOnly; SameSite=Lax"
        ))
        .map_err(unavailable)?,
    );
    Ok(response)
}

pub fn router<S: Clone + Send + Sync + 'static>(auth: AuthService) -> Router<S> {
    Router::new()
        .route("/auth/anonymous", post(anonymous))
        .route("/auth/me", get(me))
        .route("/auth/session/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/otp/request", post(request_otp))
        .route("/auth/otp/verify", post(verify_otp))
        .route("/auth/oauth/config", get(oauth_config))
        .route("/auth/oauth/login/{provider}", get(start_oauth))
        .route("/auth/oauth/callback/{provider}", post(finish_oauth))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(axum::middleware::map_response(
            |mut response: Response| async move {
                response
                    .headers_mut()
                    .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                response
            },
        ))
        .with_state(auth)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_is_bound_to_browser_provider_uri_and_pkce() {
        let verifier = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let record = OAuthState {
            provider: "twitter".into(),
            redirect: "https://shadowsofwar.io/auth/callback".into(),
            challenge: Some(URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))),
            browser_digest: digest("browser-nonce"),
        };
        let mut payload = OAuthCallback {
            code: "code".into(),
            redirect_uri: record.redirect.clone(),
            state: "state".into(),
            code_verifier: Some(verifier.into()),
            account_id: None,
        };
        assert!(validate_callback(&record, "twitter", &payload, "browser-nonce").is_ok());
        assert!(validate_callback(&record, "twitter", &payload, "other-browser").is_err());
        assert!(validate_callback(&record, "google", &payload, "browser-nonce").is_err());
        payload.code_verifier = Some("x".repeat(43));
        assert!(validate_callback(&record, "twitter", &payload, "browser-nonce").is_err());
        payload.code_verifier = Some(verifier.into());
        payload.redirect_uri = "https://evil.test/auth/callback".into();
        assert!(validate_callback(&record, "twitter", &payload, "browser-nonce").is_err());
        assert!(email("Name <address@example.test>").is_err());
    }
}
