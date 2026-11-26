mod match_exit;

use sow_data::crazygames;
use sow_data::db::PlayerDb;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use hmac::Mac;
use log::{error, info, warn};
use rand::RngCore;
use redb::ReadableTableMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tower_http::cors::{Any, CorsLayer};

const MAX_INTERNAL_REQUEST_BYTES: usize = 32 * 1024 * 1024;
const MAX_ANALYTICS_REQUEST_BYTES: usize = 256 * 1024;
const MAX_ANALYTICS_BATCH_EVENTS: usize = 100;
const PUBLIC_LEADERBOARD_CACHE_TTL: Duration = Duration::from_secs(15);
const PUBLIC_PROFILE_CACHE_TTL: Duration = Duration::from_secs(10);
const PUBLIC_PROFILE_CACHE_MAX_ENTRIES: usize = 512;
const PUBLIC_READ_RATE_LIMIT_PER_MIN: usize = 120;
const PUBLIC_SEARCH_RATE_LIMIT_PER_MIN: usize = 25;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyticsBatch {
    events: Vec<sow_data::events::AnalyticsEvent>,
    #[serde(default)]
    visitor_id: Option<String>,
}

struct AppState {
    db: PlayerDb,
    auth: sow_data::auth::AuthService,
    secret_token: String,
    relay_settlement_secret: String,
    revenuecat_webhook_secret: Option<String>,
    redb_path: String,
    legacy_events: std::sync::Mutex<sow_data::events::EventSink>,
    playgames_handoffs: std::sync::Mutex<HashMap<String, PlayGamesHandoff>>,
    playgames_sessions: std::sync::Mutex<HashMap<String, PlayGamesSession>>,
    playgames_access_tokens: std::sync::Mutex<HashMap<String, PlayGamesAccessToken>>,
    replay_verification_tx: tokio::sync::mpsc::Sender<String>,
    replay_verification_client: reqwest::Client,
    match_artifact_lock: tokio::sync::Mutex<()>,
    match_results: tokio::sync::broadcast::Sender<String>,
    leader_rate_limits: std::sync::Mutex<HashMap<String, LeaderRateLimitState>>,
    leaderboard_cache: tokio::sync::RwLock<HashMap<String, CachedLeaderboard>>,
    public_profile_cache: std::sync::Mutex<HashMap<String, CachedPublicProfile>>,
    public_api_rate_limits: std::sync::Mutex<HashMap<String, PublicRateLimitState>>,
}

#[derive(Clone)]
struct CachedLeaderboard {
    epoch: u64,
    built_at: Instant,
    updated_at: u64,
    ranked: Option<Arc<Vec<sow_data::PublicLeaderboardEntry>>>,
    victories: Option<Arc<Vec<sow_data::PublicVictoryLeaderboardEntry>>>,
}

#[derive(Clone)]
struct CachedPublicProfile {
    epoch: u64,
    built_at: Instant,
    body: Bytes,
}

#[derive(Debug, Clone, Default)]
struct PublicRateLimitState {
    timestamps: Vec<Instant>,
}

#[derive(Debug, Clone, Default)]
struct LeaderRateLimitState {
    request_timestamps: Vec<Instant>,
    block_expires_at: Option<Instant>,
    tier2_offenses: Vec<Instant>,
    suspension_expires_at: Option<Instant>,
}

const PLAYGAMES_HANDOFF_TTL: Duration = Duration::from_secs(60);
const PLAYGAMES_SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

struct PlayGamesHandoff {
    expires_at: Instant,
    external_id: String,
    display_name: String,
    avatar_url: Option<String>,
}

struct PlayGamesSession {
    expires_at: Instant,
    external_id: String,
}

struct PlayGamesAccessToken {
    access_token: String,
    expires_at: Instant,
}

struct VerifiedPlayGamesIdentity {
    external_id: String,
}

#[derive(Deserialize)]
struct ProfileQuery {
    provider: String,
    external_id: String,
}

#[derive(Deserialize)]
struct AnonymousProfileRequest {
    account_id: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Deserialize)]
struct DisplayNameRequest {
    #[serde(default)]
    account_id: Option<String>,
    display_name: String,
    #[serde(default)]
    auth_secret: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    external_id: Option<String>,
}

#[derive(Deserialize)]
struct TutorialCompleteRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Deserialize)]
struct CampaignCompleteRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    episode_id: String,
}

#[derive(Deserialize)]
struct LeaderSelectionRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    #[serde(default)]
    preferred_leader: Option<String>,
    #[serde(default)]
    prefer_random_leader: bool,
}

#[derive(Deserialize)]
struct MenuGuideProgressRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    guide_id: String,
    #[serde(default)]
    completed_steps: std::collections::BTreeSet<String>,
    #[serde(default)]
    current_step: Option<String>,
    #[serde(default)]
    status: sow_data::profile::MenuGuideStatus,
    #[serde(default)]
    selected_leader: Option<String>,
    #[serde(default)]
    menu_tips_disabled: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct MatchStartRequest {
    match_id: String,
    player_ids: Vec<String>,
    metadata: serde_json::Value,
}

#[derive(Deserialize)]
struct BotPoolSeedRequest {
    external_ids: Vec<String>,
    display_names: Vec<String>,
}

#[derive(Deserialize, Default)]
struct PublicSearchQuery {
    q: Option<String>,
    cursor: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize, Default)]
struct PublicHistoryQuery {
    cursor: Option<usize>,
    limit: Option<usize>,
    queue: Option<String>,
    mode: Option<String>,
}

#[derive(Deserialize, Default)]
struct PublicLeaderboardQuery {
    queue: Option<String>,
    mode: Option<String>,
    kind: Option<String>,
    tier: Option<String>,
    leader: Option<String>,
    q: Option<String>,
    min_games: Option<u32>,
    sort: Option<String>,
    cursor: Option<usize>,
    limit: Option<usize>,
}

/// POST /internal/verify — resolve an identity proof to a canonical account
/// id. Callers are internal services (sow-server, sow-backfill) authenticated
/// by the deployment bearer secret; the proof itself is what establishes the
/// player's identity.
#[derive(Deserialize)]
struct VerifyRequest {
    provider: String,
    #[serde(default)]
    account_id: Option<String>,
    token: String,
    #[serde(default)]
    requested_leader: Option<String>,
    #[serde(default)]
    random_leader: bool,
}

#[derive(Serialize)]
struct VerifyResponse {
    account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leader: Option<String>,
    #[serde(default)]
    skin_style: u8,
}

#[derive(Deserialize)]
struct VerifiedIdentityRequest {
    provider: String,
    environment: String,
    external_subject: String,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    requested_leader: Option<String>,
    #[serde(default)]
    random_leader: bool,
}

#[derive(Serialize)]
struct VerifiedIdentityResponse {
    account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leader: Option<String>,
    skin_style: u8,
    account: sow_data::db::PlayerAccount,
}

#[derive(Deserialize)]
struct PlayGamesExchangeRequest {
    server_auth_code: String,
    package_name: String,
}

#[derive(Serialize)]
struct PlayGamesExchangeResponse {
    handoff_token: String,
}

#[derive(Deserialize)]
struct PlayGamesConsumeRequest {
    handoff_token: String,
}

#[derive(Serialize)]
struct PlayGamesIdentityResponse {
    provider: &'static str,
    external_id: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    token: String,
}

/// POST /internal/profile/delete — operator-only account erasure for privacy
/// deletion requests (GDPR/CCPA). Same bearer gate as every /internal route
/// and loopback-bound by default; never exposed publicly. Players request
/// deletion via hello@shadowsofwar.io (see Privacy Policy); the operator
/// runs this endpoint and confirms the report.
#[derive(Deserialize)]
struct ProfileDeleteRequest {
    account_id: String,
}

#[derive(Serialize)]
struct ProfileDeleteResponse {
    account_id: String,
    found: bool,
    keys_removed: u64,
    redb_rows_removed: u32,
    analytics_sets_scrubbed: u64,
}

#[derive(Deserialize)]
struct ManualGemGrantRequest {
    account_id: String,
    gems: u64,
    request_id: String,
}

#[derive(Serialize)]
struct ManualGemGrantResponse {
    account_id: String,
    requested_gems: u64,
    balance: u64,
    request_id: String,
    granted: bool,
}

#[derive(Deserialize)]
struct ResetHumanAccountsRequest {
    confirmation: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

fn default_dry_run() -> bool {
    true
}

#[derive(Deserialize)]
struct UnlockLeaderRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    leader_id: String,
    #[serde(default)]
    currency: String,
    /// Live daily/weekly offer the player confirmed; absent means list price.
    #[serde(default)]
    offer_id: Option<String>,
}

#[derive(Deserialize)]
struct UnlockSkinRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    skin_id: String,
    #[serde(default)]
    currency: String,
    #[serde(default)]
    offer_id: Option<String>,
}

#[derive(Deserialize)]
struct UnlockBundleRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    bundle_id: String,
    #[serde(default)]
    currency: String,
}

#[derive(Deserialize)]
struct EquipSkinRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    #[serde(default)]
    skin_id: Option<String>,
}

#[derive(Deserialize)]
struct ClaimCampaignHeroRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    leader_id: String,
}

#[derive(Deserialize)]
struct RevenueCatWebhookRequest {
    event: RevenueCatWebhookEvent,
}

#[derive(Deserialize)]
struct RevenueCatWebhookEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    app_user_id: String,
    #[serde(default)]
    product_id: Option<String>,
    #[serde(default)]
    transaction_id: Option<String>,
    #[serde(default = "default_purchase_environment")]
    environment: String,
}

fn default_purchase_environment() -> String {
    "PRODUCTION".to_string()
}

#[derive(Deserialize)]
struct StorePurchasesRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Deserialize)]
struct DoubleMatchRewardRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    match_id: String,
}

#[derive(Deserialize)]
struct RewardReceiptsAckRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    receipt_ids: Vec<String>,
}

async fn handle_store_catalog(
    State(state): State<Arc<AppState>>,
) -> Result<Json<sow_data::commerce::StoreCatalog>, StatusCode> {
    let period = sow_data::commerce::current_rotation_period();
    let rotation = state
        .db
        .free_hero_rotation_for_period(period)
        .map_err(|error| {
            error!("hero rotation unavailable: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let mut catalog = sow_data::commerce::catalog_for_profile(
        &Default::default(),
        &Default::default(),
        0,
        0,
        period,
        &rotation,
    );
    let now = sow_data::commerce::unix_now();
    catalog.offers = sow_data::commerce::store_offers(now);
    catalog.bundles = sow_data::commerce::store_bundles(now);
    Ok(Json(catalog))
}

async fn store_account_for_request(
    state: &AppState,
    headers: &HeaderMap,
    account_reference: &str,
    auth_secret: Option<&str>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let platform_provider = headers
        .get("x-platform-provider")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default();
    let token = platform_auth_token(headers);
    let requested_account_id = state
        .db
        .account_id_from_reference(account_reference)
        .await
        .map_err(|error| {
            error!("store account lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "store identity unavailable".to_string(),
                }),
            )
        })?;

    if let Some(token) = token {
        let account_id = match platform_provider {
            "" | "sow" => state
                .auth
                .verify_token(&token)
                .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?,
            "playgames" => {
                let external_id = state
                    .verify_playgames_session(None, &token)
                    .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?
                    .external_id;
                let account = state
                    .db
                    .get_existing_identity("playgames".to_string(), external_id.clone())
                    .await
                    .map_err(|error| {
                        error!("Play Games store identity lookup failed: {error}");
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(ErrorResponse {
                                error: "store identity unavailable".to_string(),
                            }),
                        )
                    })?;
                let account = account.ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: "Play Games account is not linked".to_string(),
                        }),
                    )
                })?;
                account.id
            }
            "crazygames" => {
                let external_id = resolve_external_id(state, "crazygames", "", Some(&token))
                    .await
                    .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
                state
                    .db
                    .get_existing_identity("crazygames".to_string(), external_id)
                    .await
                    .map_err(|error| {
                        error!("CrazyGames store identity lookup failed: {error}");
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(ErrorResponse {
                                error: "store identity unavailable".to_string(),
                            }),
                        )
                    })?
                    .ok_or_else(|| {
                        (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: "portal account is not linked".to_string(),
                            }),
                        )
                    })?
                    .id
            }
            _ => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "unsupported platform identity".to_string(),
                    }),
                ));
            }
        };
        if requested_account_id
            .as_deref()
            .is_some_and(|id| id != account_id)
        {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "store account mismatch".to_string(),
                }),
            ));
        }
        return Ok(account_id);
    }

    let Some(account_id) = requested_account_id else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "account not found".to_string(),
            }),
        ));
    };
    if let Err(error) = state
        .db
        .verify_anonymous_secret(&account_id, auth_secret.unwrap_or_default())
        .await
    {
        return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })));
    }
    Ok(account_id)
}

async fn handle_store_purchases(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<StorePurchasesRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state.db.purchase_history_for_account(&account_id).await {
        Ok(purchases) => (StatusCode::OK, Json(purchases)).into_response(),
        Err(error) => {
            error!("purchase history lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "purchase history unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_store_purchase(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(purchase_id): Path<String>,
    Json(payload): Json<StorePurchasesRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .purchase_record_for_account(&account_id, purchase_id.trim())
        .await
    {
        Ok(Some(purchase)) => (StatusCode::OK, Json(purchase)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "purchase not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("purchase lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "purchase history unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_reward_receipts_ack(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RewardReceiptsAckRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .acknowledge_reward_receipts(&account_id, &payload.receipt_ids)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /profile/reward-receipts/double — pay a verified match's crowns and
/// gems once more after the client reports a finished rewarded video.
/// Portals give the server no proof of the view, so the daily cap in
/// `double_match_reward` bounds what a forged call can earn.
async fn handle_double_match_reward(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DoubleMatchRewardRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .double_match_reward(&account_id, &payload.match_id)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /store/leaders/unlock — spend authoritative crowns or gems on a leader,
/// at list price or at a live offer's price.
async fn handle_unlock_leader(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UnlockLeaderRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .unlock_leader(
            &account_id,
            &payload.leader_id,
            &payload.currency,
            payload.offer_id.as_deref(),
        )
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => {
            error!(
                "leader unlock rejected account={} leader={} currency={}: {error}",
                account_id, payload.leader_id, payload.currency
            );
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_unlock_skin(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UnlockSkinRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .unlock_skin(
            &account_id,
            &payload.skin_id,
            &payload.currency,
            payload.offer_id.as_deref(),
        )
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_unlock_bundle(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UnlockBundleRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .unlock_bundle(&account_id, &payload.bundle_id, &payload.currency)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_equip_skin(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EquipSkinRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .equip_skin(&account_id, payload.skin_id.as_deref())
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_revenuecat_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RevenueCatWebhookRequest>,
) -> impl IntoResponse {
    let Some(webhook_secret) = state.revenuecat_webhook_secret.as_deref() else {
        error!("RevenueCat webhook rejected: webhook secret is not configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "RevenueCat webhook is not configured".to_string(),
            }),
        )
            .into_response();
    };
    let authorized = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == webhook_secret);
    if !authorized {
        warn!("Unauthorized RevenueCat webhook request");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    let event = payload.event;
    // Grants flow one way; reversals flow the other. Refund/void/chargeback
    // notifications revoke the bundle amount (floored at zero, deduplicated
    // by event id) so the no-refund economy in the Terms stays real instead
    // of aspirational. Suspension for abuse stays a manual operator call.
    let refund = matches!(
        event.event_type.as_str(),
        "CANCELLATION" | "REFUND" | "VOIDED_PURCHASE"
    );
    if event.event_type != "NON_RENEWING_PURCHASE" && !refund {
        info!(
            "RevenueCat event ignored type={} event={}",
            event.event_type, event.id
        );
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ignored" })),
        )
            .into_response();
    }
    let Some(raw_product_id) = event.product_id.as_deref() else {
        warn!("RevenueCat purchase event {} has no product_id", event.id);
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "RevenueCat event missing product_id".to_string(),
            }),
        )
            .into_response();
    };
    let product_id = raw_product_id.to_string();
    if !sow_data::commerce::is_store_product(&product_id) {
        warn!("RevenueCat event {} has an unknown product_id", event.id);
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unknown store product".to_string(),
            }),
        )
            .into_response();
    }
    if refund {
        return match state
            .db
            .revoke_revenuecat_product(
                &event.id,
                &event.app_user_id,
                &product_id,
                event.transaction_id.as_deref(),
                &event.environment,
            )
            .await
        {
            Ok((account, true)) => {
                info!(
                    "RevenueCat purchase revoked account={} product={} gems={} type={}",
                    account_hint(Some(&account.id)),
                    product_id,
                    account.profile.gems,
                    event.event_type
                );
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "status": "revoked" })),
                )
                    .into_response()
            }
            Ok((_, false)) => (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "duplicate" })),
            )
                .into_response(),
            Err(error) => {
                error!("RevenueCat purchase revocation failed: {error}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "purchase revocation unavailable".to_string(),
                    }),
                )
                    .into_response()
            }
        };
    }
    match state
        .db
        .grant_revenuecat_product(
            &event.id,
            &event.app_user_id,
            &product_id,
            event.transaction_id.as_deref(),
            &event.environment,
        )
        .await
    {
        Ok((account, true)) => {
            info!(
                "RevenueCat gems granted account={} product={} gems={}",
                account_hint(Some(&account.id)),
                product_id,
                account.profile.gems
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "granted" })),
            )
                .into_response()
        }
        Ok((_, false)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "duplicate" })),
        )
            .into_response(),
        Err(error) => {
            error!("RevenueCat gem grant failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "purchase grant unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous/report — file a conduct report against another
/// player. Ownership-proofed (reporter secret), rate-limited, and always
/// activates a block. The moderation mailbox receives the report server-side;
/// its address is never exposed to clients.
#[derive(Deserialize)]
struct ReportPlayerRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    reported_account_id: String,
    #[serde(default)]
    match_id: Option<String>,
    reason: String,
    #[serde(default)]
    details: Option<String>,
}

#[derive(Serialize)]
struct ReportPlayerResponse {
    report_id: String,
    blocked: bool,
}

async fn handle_report_player(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ReportPlayerRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if store_account_for_request(&state, &headers, account_id, payload.auth_secret.as_deref())
        .await
        .is_err()
    {
        warn!("[moderation] report rejected ownership proof");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    let reported_account_id = payload.reported_account_id.trim().to_string();
    let reported_account_id = match state
        .db
        .account_id_from_reference(&reported_account_id)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "reported player not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("[moderation] report lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "report service unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    match sow_data::moderation::submit_report(
        &state.db,
        sow_data::moderation::ReportInput {
            reporter_account_id: account_id.to_string(),
            reported_account_id,
            match_id: payload.match_id,
            reason: payload.reason.trim().to_string(),
            details: payload.details,
        },
    )
    .await
    {
        Ok(outcome) => {
            info!(
                "[moderation] report {} filed by {} email_sent={}",
                outcome.report_id,
                account_hint(Some(account_id)),
                outcome.email_sent
            );
            (
                StatusCode::OK,
                Json(ReportPlayerResponse {
                    report_id: outcome.report_id,
                    blocked: outcome.blocked,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("rate limit") {
                StatusCode::TOO_MANY_REQUESTS
            } else if message.contains("unknown report reason")
                || message.contains("details are required")
                || message.contains("own account")
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            warn!("[moderation] report failed: {message}");
            (
                status,
                Json(ErrorResponse {
                    error: "report could not be filed".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous/blocks — owner-only read of the accounts this
/// account blocked. Clients fetch it at boot to filter chat and presence.
#[derive(Deserialize)]
struct BlocksRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Serialize)]
struct BlocksResponse {
    blocked_ids: Vec<String>,
}

async fn handle_blocks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<BlocksRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if store_account_for_request(&state, &headers, account_id, payload.auth_secret.as_deref())
        .await
        .is_err()
    {
        warn!("[moderation] blocks read rejected ownership proof");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    match sow_data::moderation::blocked_ids(&state.db, account_id).await {
        Ok(blocked_ids) => (StatusCode::OK, Json(BlocksResponse { blocked_ids })).into_response(),
        Err(error) => {
            error!("[moderation] blocks read failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "block list unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous/delete — self-service erasure. Same ownership
/// proof as renames and store unlocks, same erasure engine as the operator
/// endpoint. The client wipes local data on success and mints a fresh
/// anonymous account on next boot.
#[derive(Deserialize)]
struct SelfDeleteRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Serialize)]
struct SelfDeleteResponse {
    deleted: bool,
    keys_removed: u64,
}

fn analytics_subject_id(secret: &str, portal: &str, account_id: &str) -> String {
    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts every key length");
    mac.update(b"sow-product-analytics-v1\0");
    mac.update(portal.as_bytes());
    mac.update(b"\0");
    mac.update(account_id.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn analytics_browser_subject_id(secret: &str, visitor_id: &str) -> String {
    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts every key length");
    mac.update(b"sow-product-analytics-browser-v1\0");
    mac.update(visitor_id.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn analytics_subject_ids(state: &AppState, account_id: &str) -> Vec<String> {
    ["site", "poki", "crazygames", "jest", "android"]
        .into_iter()
        .map(|portal| analytics_subject_id(&state.secret_token, portal, account_id))
        .collect()
}

fn erase_legacy_events(state: &AppState, account_id: &str) -> std::io::Result<u64> {
    let subject_ids = analytics_subject_ids(state, account_id);
    let mut events = state
        .legacy_events
        .lock()
        .map_err(|_| std::io::Error::other("legacy event retention lock poisoned"))?;
    events.erase_account_and_subjects(account_id, &subject_ids)
}

async fn handle_self_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<SelfDeleteRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if store_account_for_request(&state, &headers, account_id, payload.auth_secret.as_deref())
        .await
        .is_err()
    {
        warn!("[privacy] self-delete rejected ownership proof");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = erase_legacy_events(&state, account_id) {
        error!("[privacy] self-delete legacy event scrub failed: {error}");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "account deletion unavailable".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = state
        .db
        .erase_analytics_subjects(&analytics_subject_ids(&state, account_id))
        .await
    {
        error!("[privacy] self-delete analytics membership scrub failed: {error}");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "account deletion unavailable".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.delete_account(account_id).await {
        Ok(report) => {
            info!(
                "[privacy] self-delete ok account={} keys={} sets={}",
                account_hint(Some(&report.account_id)),
                report.keys_removed,
                report.analytics_sets_scrubbed
            );
            (
                StatusCode::OK,
                Json(SelfDeleteResponse {
                    deleted: true,
                    keys_removed: report.keys_removed,
                }),
            )
                .into_response()
        }
        Err(error) => {
            error!("[privacy] self-delete failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "account deletion unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// /profile/anonymous response: the account plus (only when just minted) the
/// one-time ownership secret. Flatten keeps the shape identical to a bare
/// `PlayerAccount` for existing clients — serde ignores the extra field.
#[derive(Serialize)]
struct AnonymousProfileResponse {
    #[serde(flatten)]
    account: sow_data::db::PlayerAccount,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_secret: Option<String>,
}

/// POST /internal/verify
async fn resolve_identity_leader(
    db: &sow_data::db::PlayerDb,
    account_id: &str,
    requested_leader: Option<&str>,
    random_leader: bool,
) -> Result<Option<String>, String> {
    if random_leader {
        return db
            .draw_leader_for_account(account_id)
            .await
            .map(|leader| Some(sow_data::commerce::leader_wire_id(leader).to_string()))
            .map_err(|error| error.to_string());
    }
    let Some(requested) = requested_leader else {
        return Ok(None);
    };
    db.resolve_leader_for_account(account_id, Some(requested))
        .await
        .map(|resolution| Some(sow_data::commerce::leader_wire_id(resolution.resolved).to_string()))
        .map_err(|error| error.to_string())
}

async fn handle_internal_verify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifyRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/verify");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    let provider = payload.provider.trim();
    let result: Result<String, String> = match provider {
        "anonymous" => match (payload.account_id.as_deref(), payload.token.as_str()) {
            (Some(account_id), token) if !account_id.trim().is_empty() => {
                state.db.verify_anonymous_secret(account_id, token).await
            }
            _ => Err("anonymous verification requires account_id and token".to_string()),
        },
        "sow" => state.auth.verify_token(&payload.token).and_then(|id| {
            if payload
                .account_id
                .as_deref()
                .is_some_and(|requested| requested != id)
            {
                Err("account does not match session".into())
            } else {
                Ok(id)
            }
        }),
        "crazygames" | "playgames" => {
            Err("external identities must be verified by sow-server".to_string())
        }
        other => Err(format!("unsupported provider: {other}")),
    };
    match result {
        Ok(account_id) => {
            let leader = match resolve_identity_leader(
                &state.db,
                &account_id,
                payload.requested_leader.as_deref(),
                payload.random_leader,
            )
            .await
            {
                Ok(leader) => leader,
                Err(error) => {
                    error!(
                        "[identity] leader resolution failed account={} error={error}",
                        account_hint(Some(&account_id))
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "leader resolution unavailable".to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            let skin_style = match state.db.selected_skin_style_for_account(&account_id).await {
                Ok(style) => style,
                Err(error) => {
                    error!(
                        "[identity] skin resolution failed account={} error={error}",
                        account_hint(Some(&account_id))
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "skin resolution unavailable".to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            info!(
                "[identity] verify ok provider={provider} account={}",
                account_hint(Some(&account_id))
            );
            (
                StatusCode::OK,
                Json(VerifyResponse {
                    account_id,
                    leader,
                    skin_style,
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("[identity] verify failed provider={provider}: {e}");
            (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e })).into_response()
        }
    }
}

/// POST /internal/identity/resolve — persist only an identity already
/// verified by sow-server. Provider tokens never enter this service.
async fn handle_internal_identity_resolve(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifiedIdentityRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/identity/resolve");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    if payload.provider.trim().is_empty()
        || payload.environment.trim().is_empty()
        || payload.external_subject.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "verified identity fields are incomplete".to_string(),
            }),
        )
            .into_response();
    }
    let provider = payload.provider.trim();
    let environment = payload.environment.trim();
    let external_subject = payload.external_subject.trim();
    let account_result = if provider == "bot" {
        state
            .db
            .get_or_create_bot_account(environment.to_string(), external_subject.to_string())
            .await
    } else if let Some(account_id) = payload.account_id.as_deref() {
        state
            .db
            .get_or_create_for_account_id(
                account_id,
                provider.to_string(),
                environment.to_string(),
                external_subject.to_string(),
            )
            .await
    } else {
        state
            .db
            .resolve_verified_identity(provider, environment, external_subject, None)
            .await
    };
    let account = match account_result {
        Ok(account) => account,
        Err(error) => {
            error!("verified identity persistence failed: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "identity persistence unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    let leader = match resolve_identity_leader(
        &state.db,
        &account.id,
        payload.requested_leader.as_deref(),
        payload.random_leader,
    )
    .await
    {
        Ok(leader) => leader,
        Err(error) => {
            error!("verified identity leader resolution failed: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "leader resolution unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    let skin_style = sow_data::commerce::skin_style_for_profile(
        &account.profile.owned_skins,
        account.profile.selected_skin.as_deref(),
    );
    // These are deliberately accepted at the boundary for future profile
    // enrichment. PlayerAccount currently owns only its display name.
    let _ = (payload.display_name, payload.avatar_url);
    (
        StatusCode::OK,
        Json(VerifiedIdentityResponse {
            account_id: account.id.clone(),
            leader,
            skin_style,
            account: account.without_auth_secret(),
        }),
    )
        .into_response()
}

async fn handle_internal_profile_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ProfileDeleteRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/profile/delete");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    let account_id = payload.account_id.trim().to_string();
    if let Err(error) = erase_legacy_events(&state, &account_id) {
        error!("[privacy] account-delete legacy event scrub failed: {error}");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "account deletion unavailable".to_string(),
            }),
        )
            .into_response();
    }
    if let Err(error) = state
        .db
        .erase_analytics_subjects(&analytics_subject_ids(&state, &account_id))
        .await
    {
        error!("[privacy] account-delete analytics membership scrub failed: {error}");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "account deletion unavailable".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.delete_account(&account_id).await {
        Ok(report) => {
            info!(
                "[privacy] account erased account={} keys={} redb={} sets={}",
                account_hint(Some(&report.account_id)),
                report.keys_removed,
                report.redb_rows_removed,
                report.analytics_sets_scrubbed
            );
            (
                StatusCode::OK,
                Json(ProfileDeleteResponse {
                    account_id: report.account_id,
                    found: report.found,
                    keys_removed: report.keys_removed,
                    redb_rows_removed: report.redb_rows_removed,
                    analytics_sets_scrubbed: report.analytics_sets_scrubbed,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid account_id") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            warn!("[privacy] account erase failed: {message}");
            (
                status,
                Json(ErrorResponse {
                    error: "account erasure unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_internal_profile_grant_gems(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ManualGemGrantRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/profile/grant-gems");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    match state
        .db
        .grant_manual_gems(&payload.account_id, payload.gems, &payload.request_id)
        .await
    {
        Ok((account, granted)) => {
            info!(
                "[operator] gem grant account={} gems={} granted={}",
                account_hint(Some(&account.id)),
                payload.gems,
                granted
            );
            (
                StatusCode::OK,
                Json(ManualGemGrantResponse {
                    account_id: account.id,
                    requested_gems: payload.gems,
                    balance: account.profile.gems,
                    request_id: payload.request_id,
                    granted,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = match message.as_str() {
                "invalid account_id"
                | "gem amount must be greater than zero"
                | "invalid request_id" => StatusCode::BAD_REQUEST,
                "Account not found" => StatusCode::NOT_FOUND,
                "account is not a human player"
                | "request_id already used for a different grant"
                | "gem balance overflow" => StatusCode::CONFLICT,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            warn!("[operator] gem grant failed: {message}");
            (
                status,
                Json(ErrorResponse {
                    error: if status == StatusCode::INTERNAL_SERVER_ERROR {
                        "gem grant failed".to_string()
                    } else {
                        message
                    },
                }),
            )
                .into_response()
        }
    }
}

async fn handle_internal_profile_reset_test_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ResetHumanAccountsRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/profile/reset-test-data");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    if payload.confirmation != "RESET_HUMAN_TEST_DATA" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "exact confirmation is required".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.reset_human_accounts(payload.dry_run).await {
        Ok(report) => {
            info!(
                "[reset] human_accounts={} purchases={} dry_run={}",
                report.human_accounts, report.purchase_records, report.dry_run
            );
            (StatusCode::OK, Json(report)).into_response()
        }
        Err(error) => {
            warn!("[reset] human test data reset refused: {error}");
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
    }
}

#[derive(Serialize)]
struct BotPoolSeedResponse {
    account_ids: Vec<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn account_hint(account_id: Option<&str>) -> String {
    account_id
        .map(|id| id.chars().take(8).collect::<String>())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn identity_request_hint(headers: &HeaderMap) -> String {
    headers
        .get("x-sow-identity-request")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or("missing")
        .to_string()
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    info!("Starting SOW-DATABASE microservice with Redb accounts and Valkey analytics...");

    // Read environment variables
    let port: u16 = std::env::var("SOW_DB_PORT")
        .unwrap_or_else(|_| "25585".to_string())
        .parse()
        .unwrap_or(25585);

    let valkey_url = std::env::var("SOW_VALKEY_URL")
        .or_else(|_| std::env::var("SOW_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let secret_token = std::env::var("SOW_DB_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .expect("SOW_DB_SECRET must be set; refusing insecure default");
    let relay_settlement_secret = std::env::var("SOW_RELAY_SETTLEMENT_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .expect("SOW_RELAY_SETTLEMENT_SECRET must be set; refusing shared settlement auth");

    let revenuecat_webhook_secret = std::env::var("SOW_REVENUECAT_WEBHOOK_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());

    let crazygames_api_key = std::env::var("CRAZYGAMES_API_KEY").ok();

    let sanitized_valkey = if let Some(pos) = valkey_url.find('@') {
        if let Some(scheme_pos) = valkey_url.find("://") {
            format!(
                "{}***@{}",
                &valkey_url[..scheme_pos + 3],
                &valkey_url[pos + 1..]
            )
        } else {
            format!("***@{}", &valkey_url[pos + 1..])
        }
    } else {
        valkey_url.clone()
    };

    info!(
        "Config - Port: {}, Valkey: {}, Secret: [REDACTED], CG API Key Configured: {}, RevenueCat Webhook Configured: {}",
        port,
        sanitized_valkey,
        crazygames_api_key.is_some(),
        revenuecat_webhook_secret.is_some()
    );

    // Open REDB persistent database
    let redb_path =
        std::env::var("SOW_REDB_PATH").unwrap_or_else(|_| "sow_metadata.redb".to_string());
    info!("Opening persistent REDB database at {}", redb_path);
    if let Some(dir) = std::path::Path::new(&redb_path).parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).expect("Failed to create REDB data directory");
    }
    let redb_db = sow_data::init_database(&redb_path).expect("Failed to initialize Redb database");
    let redb_db_arc = Arc::new(redb_db);

    // Initialize database connector
    let player_db = PlayerDb::new(
        &valkey_url,
        crazygames_api_key,
        Some(Arc::clone(&redb_db_arc)),
    );
    // Establish Redb account storage and Valkey analytics before accepting requests.
    player_db
        .warm_connection()
        .await
        .expect("Failed to initialize the Redb account authority");
    let initial_rotation = player_db
        .current_free_hero_rotation()
        .expect("Failed to initialize the durable hero rotation");
    info!(
        "Initialized durable free hero rotation with {} heroes",
        initial_rotation.len()
    );
    player_db
        .ensure_current_season()
        .expect("Failed to initialize current profile season");

    let default_analytics_dir = std::path::Path::new(&redb_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join("analytics").to_string_lossy().into_owned())
        .unwrap_or_else(|| "analytics".to_string());

    let analytics_dir = std::env::var("SOW_ANALYTICS_DIR").unwrap_or(default_analytics_dir);
    let legacy_events = sow_data::events::EventSink::new(&analytics_dir).unwrap_or_else(|error| {
        panic!("Failed to initialize legacy event retention at {analytics_dir}: {error}");
    });

    let (replay_verification_tx, replay_verification_rx) = tokio::sync::mpsc::channel(16);
    let replay_verification_client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .build()
        .expect("failed to create replay verification client");

    let auth = sow_data::auth::AuthService::new(player_db.clone(), secret_token.clone())
        .expect("Failed to initialize first-party sessions");
    let state = Arc::new(AppState {
        auth,
        db: player_db,
        secret_token,
        relay_settlement_secret,
        revenuecat_webhook_secret,
        redb_path: redb_path.clone(),
        legacy_events: std::sync::Mutex::new(legacy_events),
        playgames_handoffs: std::sync::Mutex::new(HashMap::new()),
        playgames_sessions: std::sync::Mutex::new(HashMap::new()),
        playgames_access_tokens: std::sync::Mutex::new(HashMap::new()),
        replay_verification_tx,
        replay_verification_client,
        match_artifact_lock: tokio::sync::Mutex::new(()),
        match_results: tokio::sync::broadcast::channel(256).0,
        leader_rate_limits: std::sync::Mutex::new(HashMap::new()),
        leaderboard_cache: tokio::sync::RwLock::new(HashMap::new()),
        public_profile_cache: std::sync::Mutex::new(HashMap::new()),
        public_api_rate_limits: std::sync::Mutex::new(HashMap::new()),
    });

    tokio::spawn(match_exit::worker(
        Arc::clone(&state),
        replay_verification_rx,
    ));

    let sync_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            sync_state.retry_active_playgames_sync().await;
            if let Err(error) = sync_state.db.retry_crazygames_scores().await {
                warn!("CrazyGames pending score retry failed: {error}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    let event_retention_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        loop {
            interval.tick().await;
            if let Err(error) = event_retention_state.auth.prune() {
                error!("identity session cleanup failed: {error}");
            }
            match event_retention_state.legacy_events.lock() {
                Ok(events) => {
                    if let Err(error) = events.prune_old() {
                        error!("legacy event retention cleanup failed: {error}");
                    }
                }
                Err(error) => error!("legacy event retention lock failed: {error}"),
            }
        }
    });

    // Configure CORS for web portal compatibility
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("x-platform-auth"),
            header::HeaderName::from_static("x-platform-provider"),
            header::HeaderName::from_static("x-sow-identity-request"),
        ]);

    // Define router
    let app = Router::new()
        .merge(sow_data::auth::router(state.auth.clone()))
        .route("/healthz", get(handle_healthz))
        .route(
            "/event",
            post(handle_event).layer(DefaultBodyLimit::max(MAX_ANALYTICS_REQUEST_BYTES)),
        )
        .route("/internal/match-id-seed", get(handle_match_id_seed))
        .route(
            "/internal/release-work/{release_id}",
            get(handle_release_work),
        )
        .route("/profile", get(handle_get_profile))
        .route("/auth/playgames/exchange", post(handle_playgames_exchange))
        .route("/auth/playgames/consume", post(handle_playgames_consume))
        .route("/store/catalog", get(handle_store_catalog))
        .route("/store/purchases", post(handle_store_purchases))
        .route(
            "/profile/reward-receipts/ack",
            post(handle_reward_receipts_ack),
        )
        .route(
            "/profile/reward-receipts/double",
            post(handle_double_match_reward),
        )
        .route(
            "/store/purchases/{purchase_id}",
            post(handle_store_purchase),
        )
        .route("/store/leaders/unlock", post(handle_unlock_leader))
        .route("/store/skins/unlock", post(handle_unlock_skin))
        .route("/store/bundles/unlock", post(handle_unlock_bundle))
        .route("/store/skins/equip", post(handle_equip_skin))
        .route(
            "/internal/revenuecat/webhook",
            post(handle_revenuecat_webhook),
        )
        .route("/profiles/search", get(handle_public_profile_search))
        .route("/profiles/{account_id}", get(handle_public_profile))
        .route(
            "/profiles/{account_id}/matches",
            get(handle_public_match_history),
        )
        .route(
            "/profiles/{account_id}/seasons",
            get(handle_public_profile_seasons),
        )
        .route("/matches/{match_id}", get(handle_public_match_detail))
        .route("/seasons/current", get(handle_current_season))
        .route(
            "/seasons/current/leaderboard",
            get(handle_current_season_leaderboard),
        )
        .route(
            "/seasons/{season_id}/leaderboard",
            get(handle_public_leaderboard),
        )
        .route("/profile/anonymous", post(handle_anonymous_profile))
        .route("/profile/name", post(handle_display_name))
        .route(
            "/profile/anonymous/tutorial-complete",
            post(handle_tutorial_complete),
        )
        .route("/profile/tutorial-complete", post(handle_tutorial_complete))
        .route("/profile/match-result", post(match_exit::handle_result))
        .route("/profile/campaign-complete", post(handle_campaign_complete))
        .route(
            "/profile/campaign/claim-hero",
            post(handle_campaign_hero_claim),
        )
        .route("/profile/leader-selection", post(handle_leader_selection))
        .route(
            "/profile/menu-guide-progress",
            post(handle_menu_guide_progress),
        )
        .route("/profile/missions/claim", post(handle_claim_mission))
        .route("/profile/missions/swap", post(handle_swap_mission))
        .route("/profile/anonymous/report", post(handle_report_player))
        .route("/profile/anonymous/blocks", post(handle_blocks))
        .route("/profile/anonymous/delete", post(handle_self_delete))
        .route("/match/start", post(handle_match_start))
        .route(
            "/internal/match-start/{match_id}",
            get(handle_internal_match_start),
        )
        .route(
            "/internal/match-turns",
            post(match_exit::handle_relay_progress)
                .layer(DefaultBodyLimit::max(sow_data::MAX_MATCH_CHUNK_BYTES * 5)),
        )
        .route("/internal/stats", get(handle_internal_stats))
        .route("/internal/verify", post(handle_internal_verify))
        .route(
            "/internal/identity/resolve",
            post(handle_internal_identity_resolve),
        )
        .route(
            "/internal/profile/delete",
            post(handle_internal_profile_delete),
        )
        .route(
            "/internal/profile/grant-gems",
            post(handle_internal_profile_grant_gems),
        )
        .route(
            "/internal/profile/reset-test-data",
            post(handle_internal_profile_reset_test_data),
        )
        .route("/internal/bot-pool/seed", post(handle_bot_pool_seed))
        .route("/internal/ranked-bot", post(handle_ranked_bot))
        .route(
            "/internal/ranked-bots/available",
            post(handle_available_ranked_bots),
        )
        .route("/internal/ranked-rating", post(handle_ranked_rating))
        .layer(DefaultBodyLimit::max(MAX_INTERNAL_REQUEST_BYTES))
        .layer(cors)
        .with_state(state);

    // Bind to loopback by default. Public exposure must be an explicit
    // deployment decision made with SOW_DB_LISTEN.
    let addr: SocketAddr = std::env::var("SOW_DB_LISTEN")
        .unwrap_or_else(|_| format!("127.0.0.1:{port}"))
        .parse()
        .expect("SOW_DB_LISTEN must be a valid socket address");
    info!("SOW-DATABASE serving on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn public_client_ip(headers: &HeaderMap) -> String {
    for name in ["cf-connecting-ip", "x-real-ip"] {
        if let Some(value) = headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<std::net::IpAddr>().ok())
        {
            return value.to_string();
        }
    }
    "direct".to_string()
}

fn eval_public_rate_limit(
    limits: &mut HashMap<String, PublicRateLimitState>,
    bucket_key: &str,
    max_per_minute: usize,
    now: Instant,
) -> Result<(), u64> {
    if limits.len() > 10_000 {
        limits.retain(|_, state| {
            state
                .timestamps
                .last()
                .is_some_and(|t| now.duration_since(*t) <= Duration::from_secs(60))
        });
    }
    let state = limits.entry(bucket_key.to_string()).or_default();
    state
        .timestamps
        .retain(|t| now.duration_since(*t) <= Duration::from_secs(60));
    if state.timestamps.len() >= max_per_minute {
        let retry_after = state
            .timestamps
            .first()
            .map(|oldest| 60_u64.saturating_sub(now.duration_since(*oldest).as_secs()))
            .unwrap_or(10)
            .max(1);
        return Err(retry_after);
    }
    state.timestamps.push(now);
    Ok(())
}

fn check_public_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    scope: &str,
    max_per_minute: usize,
) -> Result<(), axum::response::Response> {
    let ip = public_client_ip(headers);
    let key = format!("{scope}:{ip}");
    let res = {
        let mut limits = state.public_api_rate_limits.lock().unwrap();
        eval_public_rate_limit(&mut limits, &key, max_per_minute, Instant::now())
    };
    if let Err(retry_after) = res {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, retry_after.to_string())],
            Json(ErrorResponse {
                error: "rate limit exceeded".to_string(),
            }),
        )
            .into_response());
    }
    Ok(())
}

fn bad_public_request(message: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn is_valid_query_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

async fn handle_public_profile_search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<PublicSearchQuery>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "search", PUBLIC_SEARCH_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    let query_text = query.q.as_deref().unwrap_or("").trim();
    if query_text.len() > 48 {
        return bad_public_request("search query too long");
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 20);
    let cursor = query.cursor.unwrap_or(0).min(200);
    if query_text.chars().count() < 2 {
        return (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "public, max-age=5")],
            Json(serde_json::json!({
                "items": [],
                "next_cursor": serde_json::Value::Null
            })),
        )
            .into_response();
    }
    match state
        .db
        .search_public_profiles(query_text, cursor.saturating_add(limit).saturating_add(1))
        .await
    {
        Ok(mut profiles) => {
            if cursor < profiles.len() {
                profiles.drain(..cursor);
            } else {
                profiles.clear();
            }
            let has_next = profiles.len() > limit;
            profiles.truncate(limit);
            let next_cursor = has_next.then_some(cursor.saturating_add(limit));
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, "public, max-age=10, stale-while-revalidate=15")],
                Json(serde_json::json!({
                    "items": profiles,
                    "next_cursor": next_cursor
                })),
            )
                .into_response()
        }
        Err(error) => {
            error!("public profile search failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profiles unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "read", PUBLIC_READ_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    let account_id = account_id.trim().to_string();
    if !sow_data::profile::is_valid_account_id(&account_id) {
        return bad_public_request("invalid account id");
    }
    let epoch = state.db.leaderboard_epoch();
    {
        let cache = state.public_profile_cache.lock().unwrap();
        if let Some(cached) = cache.get(&account_id)
            && cached.epoch == epoch
            && cached.built_at.elapsed() <= PUBLIC_PROFILE_CACHE_TTL
        {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CACHE_CONTROL, "public, max-age=10, stale-while-revalidate=20"),
                ],
                cached.body.clone(),
            )
                .into_response();
        }
    }
    match state.db.public_profile(&account_id).await {
        Ok(Some(profile)) => {
            let body = match serde_json::to_vec(&profile) {
                Ok(vec) => Bytes::from(vec),
                Err(error) => {
                    error!("public profile serialization failed: {error}");
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ErrorResponse {
                            error: "profiles unavailable".to_string(),
                        }),
                    )
                        .into_response();
                }
            };
            {
                let mut cache = state.public_profile_cache.lock().unwrap();
                if cache.len() >= PUBLIC_PROFILE_CACHE_MAX_ENTRIES {
                    cache.retain(|_, item| {
                        item.epoch == epoch && item.built_at.elapsed() <= PUBLIC_PROFILE_CACHE_TTL
                    });
                    if cache.len() >= PUBLIC_PROFILE_CACHE_MAX_ENTRIES {
                        cache.clear();
                    }
                }
                cache.insert(
                    account_id,
                    CachedPublicProfile {
                        epoch,
                        built_at: Instant::now(),
                        body: body.clone(),
                    },
                );
            }
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CACHE_CONTROL, "public, max-age=10, stale-while-revalidate=20"),
                ],
                body,
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profiles unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_match_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
    Query(query): Query<PublicHistoryQuery>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "read", PUBLIC_READ_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    let account_id = account_id.trim();
    if !sow_data::profile::is_valid_account_id(account_id) {
        return bad_public_request("invalid account id");
    }
    let queue = query.queue.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let mode = query.mode.as_deref().map(str::trim).filter(|v| !v.is_empty());
    if queue.is_some_and(|v| !is_valid_query_token(v, 32))
        || mode.is_some_and(|v| !is_valid_query_token(v, 32))
    {
        return bad_public_request("invalid history filter");
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let cursor = query.cursor.unwrap_or(0).min(10_000);
    match state
        .db
        .public_match_history(account_id, cursor, limit, queue, mode)
        .await
    {
        Ok(Some(history)) => {
            let has_next = history.len() > limit;
            let next_cursor = if has_next {
                Some(cursor.saturating_add(limit))
            } else {
                None
            };
            let items = history.into_iter().take(limit).collect::<Vec<_>>();
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, "public, max-age=10, stale-while-revalidate=20")],
                Json(serde_json::json!({
                    "items": items,
                    "next_cursor": next_cursor,
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public match history failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match history unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_match_detail(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(match_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "read", PUBLIC_READ_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    let match_id = match_id.trim();
    if !sow_data::profile::is_valid_match_id(match_id) {
        return bad_public_request("invalid match id");
    }
    match state.db.public_match_detail(match_id) {
        Ok(Some(detail)) => {
            let cache_control = if detail.verified {
                "public, max-age=300, immutable"
            } else {
                "public, max-age=15"
            };
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, cache_control)],
                Json(detail),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "match not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public match detail failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_profile_seasons(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "read", PUBLIC_READ_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    let account_id = account_id.trim();
    if !sow_data::profile::is_valid_account_id(account_id) {
        return bad_public_request("invalid account id");
    }
    match state.db.public_ratings(account_id).await {
        Ok(Some(ratings)) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "public, max-age=10, stale-while-revalidate=20")],
            Json(serde_json::json!({ "items": ratings })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile seasons failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "seasons unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_current_season(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.seasons() {
        Ok(seasons) => {
            let current = seasons
                .iter()
                .find(|season| season.id == sow_data::profile::CURRENT_SEASON_ID)
                .cloned();
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, "public, max-age=60")],
                Json(serde_json::json!({
                    "season": current,
                    "items": seasons,
                })),
            )
                .into_response()
        }
        Err(error) => {
            error!("current season lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "seasons unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

const RANKED_TIER_KEYS: [&str; 10] = [
    "nomad",
    "chieftain",
    "commander",
    "governor",
    "king",
    "emperor",
    "sovereign",
    "overlord",
    "immortal",
    "deity",
];

fn empty_tier_counts() -> BTreeMap<String, usize> {
    RANKED_TIER_KEYS
        .iter()
        .map(|key| ((*key).to_string(), 0))
        .collect()
}

fn entry_matches_leader_filter(
    preferred_leader: Option<&str>,
    wanted: sow_data::Leader,
) -> bool {
    preferred_leader
        .and_then(sow_data::commerce::leader_from_any_id)
        .is_some_and(|leader| leader == wanted)
}

async fn load_cached_leaderboard_snapshot(
    state: &Arc<AppState>,
    season_id: u32,
    victories: bool,
    queue: &str,
    mode: &str,
) -> Result<CachedLeaderboard, Box<dyn std::error::Error + Send + Sync>> {
    let epoch = state.db.leaderboard_epoch();
    let cache_key = if victories {
        format!("{season_id}:victories")
    } else {
        format!("{season_id}:ranked:{queue}:{mode}")
    };
    {
        let cache = state.leaderboard_cache.read().await;
        if let Some(entry) = cache.get(&cache_key)
            && entry.epoch == epoch
            && entry.built_at.elapsed() <= PUBLIC_LEADERBOARD_CACHE_TTL
        {
            return Ok(entry.clone());
        }
    }
    let db = state.db.clone();
    let queue_owned = queue.to_string();
    let mode_owned = mode.to_string();
    let snapshot = tokio::task::spawn_blocking(move || {
        if victories {
            let entries = db.public_victory_leaderboard_snapshot()?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((None, Some(Arc::new(entries))))
        } else {
            let entries =
                db.public_ranked_leaderboard_snapshot(season_id, &queue_owned, &mode_owned)?;
            Ok((Some(Arc::new(entries)), None))
        }
    })
    .await
    .map_err(|error| Box::<dyn std::error::Error + Send + Sync>::from(error.to_string()))??;
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let built = CachedLeaderboard {
        epoch,
        built_at: Instant::now(),
        updated_at,
        ranked: snapshot.0,
        victories: snapshot.1,
    };
    {
        let mut cache = state.leaderboard_cache.write().await;
        if cache.len() > 32 {
            cache.retain(|_, v| v.built_at.elapsed() <= PUBLIC_LEADERBOARD_CACHE_TTL);
        }
        cache.insert(cache_key, built.clone());
    }
    Ok(built)
}

fn leaderboard_etag(epoch: u64, fingerprint_parts: &str) -> String {
    let hash = blake3::hash(format!("{epoch}:{fingerprint_parts}").as_bytes()).to_hex();
    format!("W/\"lb-{epoch}-{}\"", &hash[..16])
}

fn request_matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    let Some(header_value) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let strong = etag.strip_prefix("W/").unwrap_or(etag);
    header_value.split(',').map(str::trim).any(|candidate| {
        let normalized = candidate.strip_prefix("W/").unwrap_or(candidate);
        candidate == "*" || normalized == strong
    })
}

async fn handle_current_season_leaderboard(
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<PublicLeaderboardQuery>,
) -> impl IntoResponse {
    handle_public_leaderboard(
        state,
        headers,
        Path(sow_data::profile::CURRENT_SEASON_ID),
        query,
    )
    .await
}

async fn handle_public_leaderboard(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(season_id): Path<u32>,
    Query(query): Query<PublicLeaderboardQuery>,
) -> impl IntoResponse {
    if let Err(resp) =
        check_public_rate_limit(&state, &headers, "read", PUBLIC_READ_RATE_LIMIT_PER_MIN)
    {
        return resp;
    }
    if season_id != sow_data::profile::CURRENT_SEASON_ID {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "season not found".to_string(),
            }),
        )
            .into_response();
    }
    let kind_raw = query.kind.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let is_victories = match kind_raw {
        None | Some("ranked") => false,
        Some("victories") => true,
        _ => return bad_public_request("invalid leaderboard kind"),
    };
    let leader_raw = query
        .leader
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"));
    let wanted_leader = match leader_raw {
        None => None,
        Some(raw) => {
            if raw.len() > 48 {
                return bad_public_request("invalid leader filter");
            }
            match sow_data::commerce::leader_from_any_id(raw) {
                Some(leader) => Some(leader),
                None => return bad_public_request("unknown leader filter"),
            }
        }
    };
    let search_q = query.q.as_deref().unwrap_or("").trim();
    if search_q.chars().count() > 48 {
        return bad_public_request("search query too long");
    }
    let needle = search_q.to_lowercase();
    let min_games = query.min_games.unwrap_or(0).min(500);
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let cursor = query.cursor.unwrap_or(0).min(2_000);

    if is_victories {
        let sort = query
            .sort
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("wins");
        if !matches!(sort, "wins" | "win_rate" | "matches" | "level") {
            return bad_public_request("invalid victories sort");
        }
        if query
            .tier
            .as_deref()
            .map(str::trim)
            .is_some_and(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"))
        {
            return bad_public_request("tier filter is not supported for victories leaderboard");
        }
        let snapshot =
            match load_cached_leaderboard_snapshot(&state, season_id, true, "", "").await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    error!("victories leaderboard lookup failed: {error}");
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ErrorResponse {
                            error: "leaderboard unavailable".to_string(),
                        }),
                    )
                        .into_response();
                }
            };
        let all = snapshot.victories.as_deref().map_or(&[][..], |v| v.as_slice());
        let etag = leaderboard_etag(
            snapshot.epoch,
            &format!(
                "victories:{}:{min_games}:{needle}:{sort}:{cursor}:{limit}",
                wanted_leader.map_or("", |l| l.id())
            ),
        );
        if request_matches_etag(&headers, &etag) {
            return (
                StatusCode::NOT_MODIFIED,
                [
                    (header::ETAG, etag),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=15, stale-while-revalidate=30".to_string(),
                    ),
                ],
            )
                .into_response();
        }
        let mut filtered: Vec<sow_data::PublicVictoryLeaderboardEntry> = all
            .iter()
            .filter(|entry| {
                entry.matches_played >= min_games
                    && (needle.is_empty()
                        || entry.handle.to_lowercase().contains(&needle)
                        || entry.account_id.to_lowercase().contains(&needle))
                    && wanted_leader.is_none_or(|leader| {
                        entry_matches_leader_filter(entry.preferred_leader.as_deref(), leader)
                    })
            })
            .cloned()
            .collect();
        match sort {
            "win_rate" => filtered.sort_by(|a, b| {
                b.win_rate
                    .total_cmp(&a.win_rate)
                    .then_with(|| a.rank.cmp(&b.rank))
            }),
            "matches" => filtered.sort_by(|a, b| {
                b.matches_played
                    .cmp(&a.matches_played)
                    .then_with(|| a.rank.cmp(&b.rank))
            }),
            "level" => filtered.sort_by(|a, b| {
                b.level.cmp(&a.level).then_with(|| a.rank.cmp(&b.rank))
            }),
            _ => {}
        }
        let total_count = filtered.len();
        let slice_end = cursor.saturating_add(limit).min(total_count);
        let items = if cursor < total_count {
            filtered[cursor..slice_end].to_vec()
        } else {
            Vec::new()
        };
        let next_cursor = (slice_end < total_count).then_some(slice_end);
        return (
            StatusCode::OK,
            [
                (header::ETAG, etag),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=15, stale-while-revalidate=30".to_string(),
                ),
            ],
            Json(serde_json::json!({
                "season_id": season_id,
                "metric": "victories",
                "items": items,
                "next_cursor": next_cursor,
                "total_count": total_count,
                "tier_counts": empty_tier_counts(),
                "updated_at": snapshot.updated_at,
            })),
        )
            .into_response();
    }

    let queue = query
        .queue
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Ranked1v1");
    if !matches!(queue, "Ranked1v1" | "RankedFFA" | "Matchmaking") {
        return bad_public_request("invalid leaderboard queue");
    }
    let mode = query
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("FFA");
    if mode != "FFA" {
        return bad_public_request("invalid leaderboard mode");
    }
    let tier_raw = query
        .tier
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("all"));
    let wanted_tier = match tier_raw {
        None => None,
        Some(raw) => match sow_data::profile::canonical_tier_key(raw) {
            Some(tier) => Some(tier),
            None => return bad_public_request("invalid tier filter"),
        },
    };
    let sort = query
        .sort
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("score");
    if !matches!(sort, "score" | "win_rate" | "wins" | "peak" | "games") {
        return bad_public_request("invalid leaderboard sort");
    }

    let snapshot =
        match load_cached_leaderboard_snapshot(&state, season_id, false, queue, mode).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                error!("public leaderboard lookup failed: {error}");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "leaderboard unavailable".to_string(),
                    }),
                )
                    .into_response();
            }
        };

    let all = snapshot.ranked.as_deref().map_or(&[][..], |v| v.as_slice());
    let etag = leaderboard_etag(
        snapshot.epoch,
        &format!(
            "ranked:{queue}:{mode}:{}:{}:{min_games}:{needle}:{sort}:{cursor}:{limit}",
            wanted_tier.unwrap_or("all"),
            wanted_leader.map_or("", |l| l.id())
        ),
    );
    if request_matches_etag(&headers, &etag) {
        return (
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=15, stale-while-revalidate=30".to_string(),
                ),
            ],
        )
            .into_response();
    }

    let mut tier_counts = empty_tier_counts();
    let mut filtered: Vec<sow_data::PublicLeaderboardEntry> = Vec::new();
    for entry in all {
        if entry.games_played < min_games {
            continue;
        }
        if !needle.is_empty()
            && !entry.handle.to_lowercase().contains(&needle)
            && !entry.account_id.to_lowercase().contains(&needle)
        {
            continue;
        }
        if wanted_leader.is_some_and(|leader| {
            !entry_matches_leader_filter(entry.preferred_leader.as_deref(), leader)
        }) {
            continue;
        }
        let tier_key = entry.tier.to_ascii_lowercase();
        if let Some(count) = tier_counts.get_mut(&tier_key) {
            *count += 1;
        }
        if wanted_tier.is_some_and(|wanted| !entry.tier.eq_ignore_ascii_case(wanted)) {
            continue;
        }
        filtered.push(entry.clone());
    }
    match sort {
        "win_rate" => filtered.sort_by(|a, b| {
            b.win_rate
                .total_cmp(&a.win_rate)
                .then_with(|| a.rank.cmp(&b.rank))
        }),
        "wins" => filtered.sort_by(|a, b| {
            b.wins.cmp(&a.wins).then_with(|| a.rank.cmp(&b.rank))
        }),
        "peak" => filtered.sort_by(|a, b| {
            b.peak_score
                .cmp(&a.peak_score)
                .then_with(|| a.rank.cmp(&b.rank))
        }),
        "games" => filtered.sort_by(|a, b| {
            b.games_played
                .cmp(&a.games_played)
                .then_with(|| a.rank.cmp(&b.rank))
        }),
        _ => {}
    }
    let total_count = filtered.len();
    let slice_end = cursor.saturating_add(limit).min(total_count);
    let items = if cursor < total_count {
        filtered[cursor..slice_end].to_vec()
    } else {
        Vec::new()
    };
    let next_cursor = (slice_end < total_count).then_some(slice_end);
    (
        StatusCode::OK,
        [
            (header::ETAG, etag),
            (
                header::CACHE_CONTROL,
                "public, max-age=15, stale-while-revalidate=30".to_string(),
            ),
        ],
        Json(serde_json::json!({
            "season_id": season_id,
            "queue": queue,
            "mode": mode,
            "metric": "ranked",
            "items": items,
            "next_cursor": next_cursor,
            "total_count": total_count,
            "tier_counts": tier_counts,
            "updated_at": snapshot.updated_at,
        })),
    )
        .into_response()
}

#[cfg(test)]
fn leaderboard_next_cursor(cursor: usize, limit: usize, fetched: usize) -> Option<usize> {
    (fetched > limit).then(|| cursor.saturating_add(limit))
}

#[cfg(test)]
mod leaderboard_pagination_tests {
    use super::leaderboard_next_cursor;

    #[test]
    fn next_cursor_exists_only_when_the_lookahead_finds_another_row() {
        assert_eq!(leaderboard_next_cursor(0, 25, 0), None);
        assert_eq!(leaderboard_next_cursor(0, 25, 25), None);
        assert_eq!(leaderboard_next_cursor(0, 25, 26), Some(25));
        assert_eq!(leaderboard_next_cursor(25, 25, 26), Some(50));
    }
}

/// `host` is this deployment's public host (`sow_data::auth::public_host`), so PTR's pages are first-party too.
fn is_first_party_analytics_origin(headers: &HeaderMap, host: &str) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    origin == format!("https://{host}") || origin == format!("https://www.{host}")
}

#[cfg(test)]
mod analytics_origin_tests {
    use super::{HeaderMap, header, is_first_party_analytics_origin};

    fn origin_header(origin: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, origin.parse().unwrap());
        headers
    }

    #[test]
    fn accepts_only_the_deployment_host_and_its_www_alias() {
        let cases = [
            ("shadowsofwar.io", "https://shadowsofwar.io", true),
            ("shadowsofwar.io", "https://www.shadowsofwar.io", true),
            ("shadowsofwar.io", "https://ptr.shadowsofwar.io", false),
            ("ptr.shadowsofwar.io", "https://ptr.shadowsofwar.io", true),
            ("ptr.shadowsofwar.io", "https://shadowsofwar.io", false),
            ("shadowsofwar.io", "https://attacker.example", false),
            ("shadowsofwar.io", "http://shadowsofwar.io", false),
            ("shadowsofwar.io", "null", false),
        ];
        for (host, origin, accepted) in cases {
            assert_eq!(
                is_first_party_analytics_origin(&origin_header(origin), host),
                accepted,
                "{origin} on {host}"
            );
        }
        assert!(!is_first_party_analytics_origin(
            &HeaderMap::new(),
            "shadowsofwar.io"
        ));
    }
}

fn newly_recorded_analytics_lines(lines: &[String], recorded: &[bool]) -> Option<Vec<String>> {
    if lines.len() != recorded.len() {
        return None;
    }
    Some(
        lines
            .iter()
            .zip(recorded)
            .filter_map(|(line, &is_new)| is_new.then(|| line.clone()))
            .collect(),
    )
}

#[cfg(test)]
mod analytics_archive_tests {
    use super::newly_recorded_analytics_lines;

    #[test]
    fn archive_includes_only_events_newly_counted_by_redis() {
        let lines = vec!["first".to_string(), "retry".to_string(), "next".to_string()];
        assert_eq!(
            newly_recorded_analytics_lines(&lines, &[true, false, true]),
            Some(vec!["first".to_string(), "next".to_string()])
        );
    }

    #[test]
    fn archive_rejects_unmatched_event_outcomes() {
        assert_eq!(
            newly_recorded_analytics_lines(&["event".to_string()], &[]),
            None
        );
    }
}

#[cfg(test)]
mod analytics_identity_tests {
    use super::analytics_subject_id;

    #[test]
    fn legacy_account_erasure_hash_is_channel_scoped() {
        let account_id = "0123456789abcdef0123456789abcdef";
        let site_id = analytics_subject_id("analytics-test-secret", "site", account_id);
        assert_ne!(
            site_id,
            analytics_subject_id("analytics-test-secret", "poki", account_id)
        );
    }
}

/// Liveness probe for the pipeline and service supervisor. It deliberately
/// performs no profile lookup and emits no authentication warning.
async fn handle_healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

/// Public, consent-gated product funnel. Durable events remain session-scoped;
/// an optional current-consent browser ID is HMACed for return cohorts only.
/// Account and player identity fields are rejected at deserialization.
async fn handle_event(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(batch): Json<AnalyticsBatch>,
) -> impl IntoResponse {
    if !is_first_party_analytics_origin(&headers, &sow_data::auth::public_host()) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "first-party origin required" })),
        )
            .into_response();
    }
    if batch.events.is_empty() || batch.events.len() > MAX_ANALYTICS_BATCH_EVENTS {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid analytics batch size" })),
        )
            .into_response();
    }
    let browser_subject = match batch.visitor_id.as_deref() {
        Some(visitor_id)
            if sow_data::events::is_uuid_v4(visitor_id)
                && batch.events.iter().all(|event| {
                    event.consent_version == sow_data::events::CONSENT_POLICY_VERSION
                }) =>
        {
            Some(analytics_browser_subject_id(
                &state.secret_token,
                visitor_id,
            ))
        }
        Some(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid browser consent identity" })),
            )
                .into_response();
        }
        None => None,
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    for event in &batch.events {
        if let Err(reason) = event.validate(now_ms) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": reason })),
            )
                .into_response();
        }
    }

    let accepted_events = batch.events;
    if accepted_events.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({ "accepted": 0 }))).into_response();
    }

    let lines = match accepted_events
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(lines) => lines,
        Err(error) => {
            error!("analytics event serialization failed: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "analytics unavailable" })),
            )
                .into_response();
        }
    };
    match state
        .db
        .record_analytics_batch(&accepted_events, browser_subject.as_deref())
        .await
    {
        Ok(recorded) => {
            let accepted = recorded.iter().filter(|&&is_new| is_new).count();
            match newly_recorded_analytics_lines(&lines, &recorded) {
                Some(lines) if !lines.is_empty() => match state.legacy_events.lock() {
                    Ok(mut events) => {
                        if let Err(error) = events.append_batch(&lines) {
                            error!(
                                "analytics event archive write failed after counters committed: {error}"
                            );
                        }
                    }
                    Err(error) => {
                        error!(
                            "analytics event archive lock failed after counters committed: {error}"
                        );
                    }
                },
                Some(_) => {}
                None => error!(
                    "analytics event archive skipped: outcome count mismatch after counters committed"
                ),
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "accepted": accepted })),
            )
                .into_response()
        }
        Err(error) => {
            error!("analytics batch aggregation failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "analytics temporarily unavailable" })),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous — load the canonical anonymous account and issue
/// one for a new browser profile. The initial display name is stored only when
/// the account is created; later renames use the explicit name endpoint.
async fn handle_anonymous_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AnonymousProfileRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    info!(
        "[identity] profile request id={request_id} account={} requested_name_len={}",
        account_hint(payload.account_id.as_deref()),
        payload
            .display_name
            .as_deref()
            .map(|name| name.chars().count())
            .unwrap_or(0)
    );
    match state
        .db
        .get_or_create_anonymous(
            payload.account_id.as_deref(),
            payload.display_name.as_deref(),
            payload.auth_secret.as_deref(),
        )
        .await
    {
        Ok(account) => {
            // Mint the ownership secret on first sight; the plaintext travels
            // exactly once.
            let revealed_secret = state
                .db
                .ensure_auth_secret(&account.id)
                .await
                .map_err(|e| {
                    warn!(
                        "[identity] secret mint failed for {}: {e}",
                        account_hint(Some(&account.id))
                    );
                })
                .ok()
                .flatten();
            info!(
                "[identity] profile ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (
                StatusCode::OK,
                Json(AnonymousProfileResponse {
                    account: account.without_auth_secret(),
                    auth_secret: revealed_secret,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid secret") {
                StatusCode::UNAUTHORIZED
            } else if message.contains("account_id must") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            warn!(
                "[identity] profile failed id={request_id} account={} status={} error={}",
                account_hint(payload.account_id.as_deref()),
                status,
                message
            );
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// POST /profile/name — persist the SOW display name for any authenticated account.
async fn handle_display_name(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DisplayNameRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    let platform_provider = headers
        .get("x-platform-provider")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    info!(
        "[identity] rename request id={request_id} account={} requested_name_len={}",
        account_hint(payload.account_id.as_deref()),
        payload.display_name.chars().count()
    );
    let account_id = if let (Some(account_id), Some(auth_secret)) = (
        payload.account_id.as_deref(),
        payload.auth_secret.as_deref(),
    ) {
        match state
            .db
            .verify_anonymous_secret(account_id, auth_secret)
            .await
        {
            Ok(account_id) => account_id,
            Err(error) => {
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
            }
        }
    } else {
        let provider = payload
            .provider
            .as_deref()
            .or(platform_provider)
            .unwrap_or_default()
            .trim();
        let external_id = payload.external_id.as_deref().unwrap_or_default().trim();
        if provider.is_empty() || external_id.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "identity proof is incomplete".to_string(),
                }),
            )
                .into_response();
        }
        match resolve_platform_account(
            &state,
            provider,
            external_id,
            platform_auth_token(&headers).as_deref(),
        )
        .await
        {
            Ok(account) => account.id,
            Err(error) => {
                warn!("[identity] rename identity failed id={request_id}: {error}");
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
            }
        }
    };
    match state
        .db
        .update_display_name(&account_id, &payload.display_name)
        .await
    {
        Ok(account) => {
            info!(
                "[identity] rename ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid secret") {
                StatusCode::UNAUTHORIZED
            } else if message.contains("account_id must") || message.contains("display_name") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            warn!(
                "[identity] rename failed id={request_id} account={} status={} error={}",
                account_hint(Some(&account_id)),
                status,
                message
            );
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// POST /profile/tutorial-complete — authenticate the canonical account through
/// its current identity and grant the onboarding reward exactly once.
/// The old anonymous path is an alias to this same handler.
async fn handle_tutorial_complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<TutorialCompleteRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => {
            warn!(
                "[tutorial] completion rejected id={request_id} account={}",
                account_hint(Some(&payload.account_id))
            );
            return response.into_response();
        }
    };

    match state.db.complete_tutorial(&account_id).await {
        Ok(account) => {
            let sync_state = Arc::clone(&state);
            let sync_account_id = account.id.clone();
            tokio::spawn(async move {
                if let Err(error) = sync_state.sync_playgames_profile(&sync_account_id).await {
                    warn!(
                        "Play Games tutorial sync failed for account={}: {error}",
                        account_hint(Some(&sync_account_id))
                    );
                }
            });
            info!(
                "[tutorial] completion accepted id={request_id} account={} completed={}",
                account_hint(Some(&account.id)),
                account.profile.intro_completed
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(error) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_campaign_complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CampaignCompleteRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .complete_campaign_episode(&account_id, payload.episode_id.trim())
        .await
    {
        Ok(account) => {
            let sync_state = Arc::clone(&state);
            let sync_account_id = account.id.clone();
            tokio::spawn(async move {
                if let Err(error) = sync_state.sync_playgames_profile(&sync_account_id).await {
                    warn!(
                        "Play Games campaign sync failed for account={}: {error}",
                        account_hint(Some(&sync_account_id))
                    );
                }
            });
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.starts_with("unknown campaign") {
                StatusCode::BAD_REQUEST
            } else if message.contains("not published") || message.contains("is locked") {
                StatusCode::CONFLICT
            } else if message.contains("account") || message.contains("Account not found") {
                StatusCode::NOT_FOUND
            } else {
                error!("campaign completion failed: {message}");
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

async fn handle_campaign_hero_claim(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ClaimCampaignHeroRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    match state
        .db
        .claim_advanced_tactics_hero(&account_id, payload.leader_id.trim())
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid hero") {
                StatusCode::BAD_REQUEST
            } else if message.contains("first") || message.contains("already claimed") {
                StatusCode::CONFLICT
            } else if message.contains("account") || message.contains("Account not found") {
                StatusCode::NOT_FOUND
            } else {
                error!("campaign hero claim failed: {message}");
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

async fn handle_leader_selection(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<LeaderSelectionRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    if let Err((status, message)) = state.check_leader_rate_limit(&account_id) {
        return (status, Json(ErrorResponse { error: message })).into_response();
    }
    match state
        .db
        .update_leader_selection(
            &account_id,
            payload.preferred_leader.as_deref(),
            payload.prefer_random_leader,
        )
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => {
            let message = error.to_string();
            let status = if message.starts_with("unknown")
                || message.contains("not available")
                || message.contains("required")
            {
                StatusCode::BAD_REQUEST
            } else if message.contains("Account not found") {
                StatusCode::NOT_FOUND
            } else {
                error!("leader selection save failed: {message}");
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// POST /profile/menu-guide-progress — save server-validated menu guide state.
async fn handle_menu_guide_progress(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<MenuGuideProgressRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    let progress = sow_data::profile::MenuGuideProgress {
        completed_steps: payload.completed_steps,
        current_step: payload.current_step,
        status: payload.status,
        selected_leader: payload.selected_leader,
        source_match_id: None,
    };
    match state
        .db
        .save_menu_guide_progress_with_options(
            &account_id,
            payload.guide_id.trim(),
            progress,
            payload.menu_tips_disabled,
        )
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => {
            let message = error.to_string();
            let status = if message.starts_with("unknown ")
                || message.starts_with("completed menu guide")
                || message.starts_with("selected_leader")
                || message.starts_with("menu guide")
            {
                StatusCode::BAD_REQUEST
            } else if message.contains("Account not found") {
                StatusCode::NOT_FOUND
            } else {
                error!("menu guide progress save failed: {message}");
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct SwapMissionRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    mission_id: String,
    #[serde(default)]
    used_ad: bool,
}

/// POST /profile/missions/swap — swap one incomplete active Daily/Weekly mission for another playable mission.
async fn handle_swap_mission(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<SwapMissionRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };

    match state
        .db
        .swap_mission(&account_id, payload.mission_id.trim(), payload.used_ad)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => {
            let message = error.to_string();
            let status = if message.starts_with("unknown mission")
                || message.contains("cannot be swapped")
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::CONFLICT
            };
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClaimMissionRequest {
    account_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    mission_id: String,
    #[serde(default)]
    doubled_by_ad: bool,
}

/// POST /profile/missions/claim — securely claim an authoritative mission reward.
async fn handle_claim_mission(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ClaimMissionRequest>,
) -> impl IntoResponse {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &payload.account_id,
        payload.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };

    match state
        .db
        .claim_mission_reward(
            &account_id,
            payload.mission_id.trim(),
            payload.doubled_by_ad,
        )
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

fn valid_match_participant_settlement_params(match_id: &str, account_id: &str) -> bool {
    match_id.parse::<u64>().is_ok() && sow_data::profile::is_valid_account_id(account_id)
}

fn participant_closure_error_status(message: &str) -> StatusCode {
    if matches!(
        message,
        "account is not registered in this match"
            | "account is not a registered human participant"
            | "only human participants can close a personal receipt"
            | "personal receipt closure conflicts with its saved state"
            | "personal receipt rejection conflicts with its saved state"
    ) {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn reject_participant_settlement(
    db: &PlayerDb,
    match_id: &str,
    account_id: &str,
    closure_turn: u64,
) -> Result<bool, String> {
    db.note_match_participant_closure(match_id, account_id, closure_turn)
        .await
        .map_err(|error| error.to_string())?;
    let account = db
        .reject_match_participant_result(match_id, account_id, closure_turn)
        .await
        .map_err(|error| error.to_string())?;
    Ok(account
        .profile
        .reward_receipts
        .get(match_id)
        .is_some_and(|receipt| {
            receipt.verification_status
                == Some(sow_data::profile::ReplayVerificationStatus::Rejected)
        }))
}

#[cfg(test)]
mod participant_closure_http_tests {
    use super::{
        StatusCode, participant_closure_error_status, valid_match_participant_settlement_params,
    };

    #[test]
    fn settlement_accepts_supported_account_id_formats_only() {
        assert!(valid_match_participant_settlement_params(
            "15381",
            "0123456789abcdef0123456789abcdef"
        ));
        assert!(valid_match_participant_settlement_params(
            "15381",
            "52948534-1c9f-4def-bc8e-80390d3287a6"
        ));
        assert!(!valid_match_participant_settlement_params(
            "15381",
            "guest_0123456789abcdef0123456789abcdef"
        ));
        assert!(!valid_match_participant_settlement_params(
            "not-a-match",
            "0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn only_invalid_closures_are_permanent_http_errors() {
        assert_eq!(
            participant_closure_error_status("account is not registered in this match"),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            participant_closure_error_status(
                "personal receipt rejection conflicts with its saved state"
            ),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            participant_closure_error_status("database unavailable"),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

#[derive(Deserialize)]
struct PlayGamesOAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct PlayGamesPlayerResponse {
    #[serde(rename = "playerId")]
    player_id: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(rename = "avatarImageUrl", default)]
    avatar_url: Option<String>,
}

fn random_playgames_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn handle_playgames_exchange(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlayGamesExchangeRequest>,
) -> impl IntoResponse {
    if payload.package_name != "com.shadowsofwar"
        && payload.package_name != "com.shadowsofwar.debug"
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported Android package".to_string(),
            }),
        )
            .into_response();
    }
    if payload.server_auth_code.trim().is_empty() || payload.server_auth_code.len() > 4096 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid Play Games server auth code".to_string(),
            }),
        )
            .into_response();
    }

    let client_id = match std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("Play Games server client ID is not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Play Games server access is not configured".to_string(),
                }),
            )
                .into_response();
        }
    };
    let client_secret = match std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_SECRET") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("Play Games server client secret is not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Play Games server access is not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let client = reqwest::Client::new();
    let token_response = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", payload.server_auth_code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<PlayGamesOAuthTokenResponse>().await {
                Ok(value) if !value.access_token.is_empty() => value,
                Ok(_) => {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(ErrorResponse {
                            error: "Play Games token response was empty".to_string(),
                        }),
                    )
                        .into_response();
                }
                Err(error) => {
                    warn!("Play Games token response unreadable: {error}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: "Play Games token exchange failed".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Ok(response) => {
            warn!(
                "Play Games token exchange rejected: HTTP {}",
                response.status()
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Play Games authentication was rejected".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Play Games token exchange failed: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "Play Games authentication is temporarily unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    let access_token = token_response.access_token.clone();
    let player = match client
        .get("https://games.googleapis.com/games/v1/players/me")
        .bearer_auth(&access_token)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<PlayGamesPlayerResponse>().await {
                Ok(player) => player,
                Err(error) => {
                    warn!("Play Games player response unreadable: {error}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: "Play Games player verification failed".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Ok(response) => {
            warn!(
                "Play Games player verification rejected: HTTP {}",
                response.status()
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Play Games player verification was rejected".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Play Games player verification failed: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "Play Games player verification is temporarily unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    if player.player_id.trim().is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Play Games response did not include a player ID".to_string(),
            }),
        )
            .into_response();
    }

    let account = match state
        .db
        .get_existing_identity("playgames".to_string(), player.player_id.clone())
        .await
    {
        Ok(Some(account)) => account,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Play Games account is not linked".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("Play Games account lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Play Games account lookup failed".to_string(),
                }),
            )
                .into_response();
        }
    };

    let token_ttl = token_response
        .expires_in
        .unwrap_or(3600)
        .saturating_sub(30)
        .max(60);
    {
        let Ok(mut access_tokens) = state.playgames_access_tokens.lock() else {
            error!("Play Games access-token store is poisoned");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Play Games session unavailable".to_string(),
                }),
            )
                .into_response();
        };
        access_tokens.insert(
            account.id.clone(),
            PlayGamesAccessToken {
                access_token,
                expires_at: Instant::now() + Duration::from_secs(token_ttl),
            },
        );
    }

    if let Err(error) = state.sync_playgames_profile(&account.id).await {
        error!(
            "Play Games profile reconciliation failed for account={}: {error}",
            account_hint(Some(&account.id))
        );
    }

    let handoff_token = random_playgames_token();
    let handoff = PlayGamesHandoff {
        expires_at: Instant::now() + PLAYGAMES_HANDOFF_TTL,
        external_id: player.player_id,
        display_name: player.display_name,
        avatar_url: player.avatar_url,
    };
    let Ok(mut handoffs) = state.playgames_handoffs.lock() else {
        error!("Play Games handoff store is poisoned");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Play Games handoff unavailable".to_string(),
            }),
        )
            .into_response();
    };
    let now = Instant::now();
    handoffs.retain(|_, value| value.expires_at > now);
    handoffs.insert(handoff_token.clone(), handoff);

    (
        StatusCode::OK,
        Json(PlayGamesExchangeResponse { handoff_token }),
    )
        .into_response()
}

async fn handle_playgames_consume(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlayGamesConsumeRequest>,
) -> impl IntoResponse {
    let handoff = {
        let Ok(mut handoffs) = state.playgames_handoffs.lock() else {
            error!("Play Games handoff store is poisoned");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Play Games handoff unavailable".to_string(),
                }),
            )
                .into_response();
        };
        let now = Instant::now();
        handoffs.retain(|_, value| value.expires_at > now);
        handoffs.remove(payload.handoff_token.trim())
    };

    let Some(handoff) = handoff else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Play Games handoff expired or already used".to_string(),
            }),
        )
            .into_response();
    };

    let token = random_playgames_token();
    let Ok(mut sessions) = state.playgames_sessions.lock() else {
        error!("Play Games session store is poisoned");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Play Games session unavailable".to_string(),
            }),
        )
            .into_response();
    };
    let now = Instant::now();
    sessions.retain(|_, value| value.expires_at > now);
    sessions.insert(
        token.clone(),
        PlayGamesSession {
            expires_at: now + PLAYGAMES_SESSION_TTL,
            external_id: handoff.external_id.clone(),
        },
    );

    (
        StatusCode::OK,
        Json(PlayGamesIdentityResponse {
            provider: "playgames",
            external_id: handoff.external_id,
            display_name: handoff.display_name,
            avatar_url: handoff.avatar_url,
            token,
        }),
    )
        .into_response()
}

impl AppState {
    fn check_leader_rate_limit(&self, account_id: &str) -> Result<(), (StatusCode, String)> {
        Self::eval_leader_rate_limit(
            &mut self.leader_rate_limits.lock().unwrap(),
            account_id,
            Instant::now(),
        )
    }

    fn eval_leader_rate_limit(
        limits: &mut HashMap<String, LeaderRateLimitState>,
        account_id: &str,
        now: Instant,
    ) -> Result<(), (StatusCode, String)> {
        if limits.len() > 5000 {
            limits.retain(|_, v| {
                v.block_expires_at.is_some_and(|t| t > now)
                    || v.suspension_expires_at.is_some_and(|t| t > now)
                    || !v.tier2_offenses.is_empty()
                    || !v.request_timestamps.is_empty()
            });
        }
        let entry = limits.entry(account_id.to_string()).or_default();

        if let Some(suspended_until) = entry.suspension_expires_at {
            if now < suspended_until {
                let remaining_secs = (suspended_until - now).as_secs().max(1);
                return Err((
                    StatusCode::FORBIDDEN,
                    format!(
                        "Account temporarily suspended for repeated abuse; try again in {remaining_secs}s"
                    ),
                ));
            } else {
                entry.suspension_expires_at = None;
            }
        }

        entry
            .request_timestamps
            .retain(|t| now.duration_since(*t) <= Duration::from_secs(300));
        entry
            .tier2_offenses
            .retain(|t| now.duration_since(*t) <= Duration::from_secs(3 * 86400));
        entry.request_timestamps.push(now);

        let reqs_last_5m = entry.request_timestamps.len();
        if reqs_last_5m > 60 {
            entry.tier2_offenses.push(now);
            entry.request_timestamps.clear();
            if entry.tier2_offenses.len() >= 3 {
                entry.suspension_expires_at = Some(now + Duration::from_secs(3 * 86400));
                return Err((
                    StatusCode::FORBIDDEN,
                    "Account suspended for 3 days due to repeated rate limit violations"
                        .to_string(),
                ));
            }
            entry.block_expires_at = Some(now + Duration::from_secs(300));
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Too many hero selection requests (>60/5m); locked for 5 minutes".to_string(),
            ));
        }

        if let Some(blocked_until) = entry.block_expires_at {
            if now < blocked_until {
                let remaining_secs = (blocked_until - now).as_secs().max(1);
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    format!("Too many hero selection requests; try again in {remaining_secs}s"),
                ));
            } else {
                entry.block_expires_at = None;
            }
        }

        let reqs_last_1m = entry
            .request_timestamps
            .iter()
            .filter(|t| now.duration_since(**t) <= Duration::from_secs(60))
            .count();
        if reqs_last_1m > 20 {
            entry.block_expires_at = Some(now + Duration::from_secs(5));
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded (max 20/min); cooling down for 5 seconds".to_string(),
            ));
        }

        Ok(())
    }

    async fn sync_playgames_progress(
        &self,
        account_id: &str,
        unlocked_achievements: &[String],
        victories: Option<u32>,
    ) -> Result<bool, String> {
        let env_value = |name: &str| std::env::var(name).unwrap_or_default().trim().to_string();
        let first_victory = env_value("SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID");
        let battle_hardened = env_value("SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID");
        let victory_march = env_value("SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID");
        let laurel_hoard = env_value("SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID");
        let first_command = env_value("SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID");
        let commander_victorious = env_value("SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID");
        let veteran_commander = env_value("SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID");
        let banner_collector = env_value("SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID");
        let leader_path = env_value("SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID");
        let leaderboard_id = env_value("SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID");
        if first_victory.is_empty()
            && battle_hardened.is_empty()
            && victory_march.is_empty()
            && laurel_hoard.is_empty()
            && first_command.is_empty()
            && commander_victorious.is_empty()
            && veteran_commander.is_empty()
            && banner_collector.is_empty()
            && leader_path.is_empty()
            && leaderboard_id.is_empty()
        {
            return Ok(false);
        }

        let access_token = {
            let mut access_tokens = self
                .playgames_access_tokens
                .lock()
                .map_err(|_| "Play Games access-token store is poisoned".to_string())?;
            access_tokens.retain(|_, value| value.expires_at > Instant::now());
            access_tokens
                .get(account_id)
                .map(|value| value.access_token.clone())
        };
        let Some(access_token) = access_token else {
            return Ok(false);
        };

        let client = reqwest::Client::new();
        let account_hint = account_hint(Some(account_id));
        let mut actions = Vec::new();
        let add_unlock =
            |actions: &mut Vec<(String, Option<(&'static str, String)>, &'static str)>,
             id: &str,
             label: &'static str| {
                if !id.is_empty() {
                    actions.push((
                        format!("https://games.googleapis.com/games/v1/achievements/{id}/unlock"),
                        None,
                        label,
                    ));
                }
            };
        for achievement in unlocked_achievements {
            match achievement.as_str() {
                "first_victory" => add_unlock(&mut actions, &first_victory, "First Victory"),
                "battle_hardened" => add_unlock(&mut actions, &battle_hardened, "Battle Hardened"),
                "victory_march" => add_unlock(&mut actions, &victory_march, "Victory March"),
                "laurel_hoard" => add_unlock(&mut actions, &laurel_hoard, "Laurel Hoard"),
                "first_command" => add_unlock(&mut actions, &first_command, "First Command"),
                "commander_victorious" => {
                    add_unlock(&mut actions, &commander_victorious, "Commander Victorious")
                }
                "veteran_commander" => {
                    add_unlock(&mut actions, &veteran_commander, "Veteran Commander")
                }
                "banner_collector" => {
                    add_unlock(&mut actions, &banner_collector, "Banner Collector")
                }
                "leader_path" => add_unlock(&mut actions, &leader_path, "Leader Path"),
                _ => {}
            }
        }
        if let Some(victories) = victories
            && !leaderboard_id.is_empty()
        {
            actions.push((
                format!(
                    "https://games.googleapis.com/games/v1/leaderboards/{leaderboard_id}/scores"
                ),
                Some(("score", victories.to_string())),
                "Victories leaderboard",
            ));
        }
        if actions.is_empty() {
            return Ok(true);
        }

        let mut first_error = None;
        for (url, query, label) in actions {
            let mut request = client
                .post(url)
                .bearer_auth(&access_token)
                .timeout(Duration::from_secs(5));
            if let Some(query) = query {
                request = request.query(&[query]);
            }
            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    info!("Play Games {label} synced for account={account_hint}");
                }
                Ok(response) => {
                    let message = format!(
                        "HTTP {} syncing {label} for account={account_hint}",
                        response.status()
                    );
                    error!("Play Games request rejected: {message}");
                    first_error.get_or_insert(message);
                }
                Err(error) => {
                    let message = format!("{error} syncing {label} for account={account_hint}");
                    error!("Play Games request failed: {message}");
                    first_error.get_or_insert(message);
                }
            }
        }

        first_error.map_or(Ok(true), Err)
    }

    /// Reconcile verified profile progress when a Play Games token is active.
    async fn sync_playgames_profile(&self, account_id: &str) -> Result<(), String> {
        let snapshot = self
            .db
            .playgames_sync_snapshot(account_id)
            .await
            .map_err(|error| format!("load profile for Play Games sync: {error}"))?;
        let Some(snapshot) = snapshot else {
            return Ok(());
        };
        if snapshot.revision <= snapshot.synced_revision {
            return Ok(());
        }
        if self
            .sync_playgames_progress(
                account_id,
                &snapshot.unlocked_achievements,
                Some(snapshot.wins),
            )
            .await?
        {
            self.db
                .mark_playgames_sync(account_id, snapshot.revision)
                .await
                .map_err(|error| format!("mark Play Games sync: {error}"))?;
        }
        Ok(())
    }

    async fn retry_active_playgames_sync(&self) {
        let account_ids = match self.playgames_access_tokens.lock() {
            Ok(mut tokens) => {
                let now = Instant::now();
                tokens.retain(|_, value| value.expires_at > now);
                tokens.keys().cloned().collect::<Vec<_>>()
            }
            Err(_) => {
                error!("Play Games access-token store is poisoned; sync retry skipped");
                return;
            }
        };

        for account_id in account_ids {
            if let Err(error) = self.sync_playgames_profile(&account_id).await {
                warn!(
                    "Play Games pending profile sync failed for account={}: {error}",
                    account_hint(Some(&account_id))
                );
            }
        }
    }

    fn verify_playgames_session(
        &self,
        external_id: Option<&str>,
        token: &str,
    ) -> Result<VerifiedPlayGamesIdentity, String> {
        let Ok(mut sessions) = self.playgames_sessions.lock() else {
            return Err("Play Games session store is poisoned".to_string());
        };
        let now = Instant::now();
        sessions.retain(|_, value| value.expires_at > now);
        let Some(session) = sessions.get(token.trim()) else {
            return Err("Play Games session expired or invalid".to_string());
        };
        if external_id.is_some_and(|value| value != session.external_id) {
            return Err("Play Games player mismatch".to_string());
        }
        Ok(VerifiedPlayGamesIdentity {
            external_id: session.external_id.clone(),
        })
    }
}

fn platform_auth_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-platform-auth")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

async fn resolve_external_id(
    state: &AppState,
    provider: &str,
    external_id: &str,
    auth_token: Option<&str>,
) -> Result<String, String> {
    if provider == "sow" {
        let token = auth_token.ok_or("Sign-in requests require X-Platform-Auth")?;
        let account_id = state.auth.verify_token(token)?;
        if !external_id.is_empty() && external_id != account_id {
            return Err("account does not match session".into());
        }
        return Ok(account_id);
    }
    if provider == "crazygames" {
        let Some(token) = auth_token else {
            return Err("CrazyGames requests require X-Platform-Auth token".into());
        };
        let verified_id = crazygames::verify_user_token(token).await?;
        if !external_id.is_empty() && external_id != verified_id {
            return Err("CrazyGames identity does not match its verified token".into());
        }
        return Ok(verified_id);
    }
    Err("unsupported platform provider".into())
}

async fn resolve_platform_account(
    state: &AppState,
    provider: &str,
    external_id: &str,
    auth_token: Option<&str>,
) -> Result<sow_data::db::PlayerAccount, String> {
    let resolved_external_id = if provider == "playgames" {
        state
            .verify_playgames_session(Some(external_id), auth_token.unwrap_or(""))
            .map(|identity| identity.external_id)
            .map_err(|error| error.to_string())?
    } else {
        resolve_external_id(state, provider, external_id, auth_token).await?
    };

    if provider == "sow" {
        state
            .db
            .account_by_id(&resolved_external_id)
            .await
            .map_err(|error| error.to_string())
    } else if let Some(account) = state
        .db
        .get_existing_identity(provider.to_owned(), resolved_external_id.clone())
        .await
        .map_err(|error| error.to_string())?
    {
        Ok(account)
    } else {
        state
            .db
            .resolve_verified_identity(provider, "production", &resolved_external_id, None)
            .await
            .map_err(|error| error.to_string())
    }
}

/// Verify if the request has the correct Authorization bearer secret
fn verify_internal_auth(headers: &HeaderMap, secret_token: &str) -> bool {
    if let Some(auth_header) = headers.get(header::AUTHORIZATION)
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        return token.trim() == secret_token;
    }
    false
}

/// GET /profile handler (platform token verification for signed-in providers)
async fn handle_get_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ProfileQuery>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    let provider = query.provider.trim();
    let external_id = query.external_id.trim();
    let auth_token = platform_auth_token(&headers);

    info!(
        "[identity] platform profile request id={request_id} provider={provider} external_id_len={}",
        external_id.chars().count()
    );

    if provider.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "provider cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    match resolve_platform_account(&state, provider, external_id, auth_token.as_deref()).await {
        Ok(account) => {
            info!(
                "[identity] platform profile ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(e) => (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e })).into_response(),
    }
}

/// POST /match/start (internal matchmaking registration)
async fn handle_match_id_seed(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"Unauthorized"})),
        )
            .into_response();
    }
    match state.db.reserve_match_id_range() {
        Ok((next_match_id, reserved_through)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "next_match_id": next_match_id,
                "reserved_through": reserved_through
            })),
        )
            .into_response(),
        Err(error) => {
            error!("match ID recovery failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"match IDs unavailable"})),
            )
                .into_response()
        }
    }
}

async fn handle_release_work(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(release_id): Path<String>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"Unauthorized"})),
        )
            .into_response();
    }
    if release_id.is_empty()
        || release_id.len() > 128
        || !release_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid release id"})),
        )
            .into_response();
    }
    match state.db.outstanding_release_matches(&release_id) {
        Ok(outstanding_matches) => (
            StatusCode::OK,
            Json(serde_json::json!({"outstanding_matches": outstanding_matches})),
        )
            .into_response(),
        Err(error) => {
            error!("release work status unavailable for {release_id}: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"release work status unavailable"})),
            )
                .into_response()
        }
    }
}

async fn handle_match_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<MatchStartRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /match/start");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .db
        .register_match_start(&payload.match_id, &payload.player_ids, payload.metadata)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_internal_match_start(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.match_start_record(&match_id) {
        Ok(Some(record)) => (StatusCode::OK, Json(record)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "match registration not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("match {} metadata read failed: {error}", match_id);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match metadata unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn reject_match_verification(
    state: &AppState,
    match_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let start = state
        .db
        .match_start_record(match_id)?
        .ok_or("trusted match registration unavailable")?;
    if start.replay_status == sow_data::profile::ReplayVerificationStatus::Verified {
        state.db.finish_match_verification(
            match_id,
            sow_data::profile::ReplayVerificationStatus::Verified,
        )?;
        return Ok(());
    }
    state
        .db
        .mark_match_reward_verification_status(
            match_id,
            sow_data::profile::ReplayVerificationStatus::Rejected,
        )
        .await?;
    state.db.finish_match_verification(
        match_id,
        sow_data::profile::ReplayVerificationStatus::Rejected,
    )?;
    Ok(())
}

/// POST /internal/bot-pool/seed — resolve or create persistent bot accounts
/// for the given external_ids (provider="bot"). Idempotent: re-calling with
/// the same external_ids returns the same account_ids in the same order.
/// The paired display_names are persisted on those accounts.
async fn handle_bot_pool_seed(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<BotPoolSeedRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/bot-pool/seed");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .db
        .seed_bot_pool(payload.external_ids, payload.display_names)
        .await
    {
        Ok(account_ids) => {
            (StatusCode::OK, Json(BotPoolSeedResponse { account_ids })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct RankedBotSeed {
    external_id: String,
    display_name: String,
    leader: String,
    iq: u32,
}

#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum RankedBotRequest {
    List,
    Create {
        external_id: String,
        display_name: String,
        leader: String,
        iq: u32,
        actor: String,
    },
    Seed {
        bots: Vec<RankedBotSeed>,
    },
    Reserve {
        account_id: String,
        match_id: String,
    },
    Release {
        account_id: String,
        match_id: String,
    },
    Pause {
        account_id: String,
        actor: String,
    },
    Reactivate {
        account_id: String,
        actor: String,
    },
    SetIq {
        account_id: String,
        iq: u32,
        actor: String,
    },
    SetRating {
        account_id: String,
        score: u16,
        actor: String,
    },
    Recover {
        live_match_ids: Vec<String>,
    },
}

#[derive(serde::Deserialize)]
struct RankedRatingRequest {
    account_id: String,
}

async fn handle_ranked_rating(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<RankedRatingRequest>,
) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".into(),
            }),
        )
            .into_response();
    }
    match state.db.ranked_rating(&request.account_id).await {
        Ok(rating) => (StatusCode::OK, Json(rating)).into_response(),
        Err(error) => {
            error!("ranked rating lookup failed: {error}");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "ranked rating unavailable".into(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_available_ranked_bots(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".into(),
            }),
        )
            .into_response();
    }
    match state.db.available_ranked_bots().await {
        Ok(candidates) => {
            let candidates = candidates
                .into_iter()
                .map(|(account, rating)| {
                    serde_json::json!({
                        "account": account.without_auth_secret(),
                        "rating": rating,
                    })
                })
                .collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "candidates": candidates })),
            )
                .into_response()
        }
        Err(error) => {
            error!("ranked bot candidate lookup failed: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "ranked bot candidates unavailable".into(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_ranked_bot(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<RankedBotRequest>,
) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".into(),
            }),
        )
            .into_response();
    }
    let request = match request {
        RankedBotRequest::List => return match state.db.ranked_bots().await {
            Ok(bots) => (StatusCode::OK, Json(serde_json::json!({"bots": bots.into_iter().map(|(account, rating)| serde_json::json!({
                "account": account.without_auth_secret(), "rating": rating,
            })).collect::<Vec<_>>()}))).into_response(),
            Err(error) => {
                error!("ranked bot list failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "ranked bot list unavailable".into() })).into_response()
            }
        },
        RankedBotRequest::Recover { live_match_ids } => return match state.db.recover_ranked_bot_reservations(&live_match_ids).await {
            Ok(released) => (StatusCode::OK, Json(serde_json::json!({"released": released}))).into_response(),
            Err(error) => {
                error!("ranked bot reservation recovery failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "ranked bot recovery failed".into() })).into_response()
            }
        },
        request => request,
    };
    let result = match request {
        RankedBotRequest::Create {
            external_id,
            display_name,
            leader,
            iq,
            actor,
        } => state
            .db
            .get_or_create_ranked_bot(&external_id, &display_name, &leader, iq, Some(&actor))
            .await
            .map(|account| vec![account]),
        RankedBotRequest::Seed { bots } if bots.len() <= 64 => {
            let mut accounts = Vec::with_capacity(bots.len());
            let mut failure = None;
            for bot in bots {
                match state
                    .db
                    .get_or_create_ranked_bot(
                        &bot.external_id,
                        &bot.display_name,
                        &bot.leader,
                        bot.iq,
                        None,
                    )
                    .await
                {
                    Ok(account) => accounts.push(account),
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            match failure {
                Some(error) => Err(error),
                None => Ok(accounts),
            }
        }
        RankedBotRequest::Seed { .. } => Err("ranked bot seed exceeds 64 entries".into()),
        RankedBotRequest::Reserve {
            account_id,
            match_id,
        } => state
            .db
            .reserve_ranked_bot(&account_id, &match_id)
            .await
            .map(|account| account.into_iter().collect()),
        RankedBotRequest::Release {
            account_id,
            match_id,
        } => state
            .db
            .release_ranked_bot(&account_id, &match_id)
            .await
            .map(|()| Vec::new()),
        RankedBotRequest::Pause { account_id, actor } => state
            .db
            .update_ranked_bot(
                &account_id,
                sow_data::db::RankedBotManualUpdate::SetPaused {
                    paused: true,
                    actor,
                },
            )
            .await
            .map(|account| vec![account]),
        RankedBotRequest::Reactivate { account_id, actor } => state
            .db
            .update_ranked_bot(
                &account_id,
                sow_data::db::RankedBotManualUpdate::SetPaused {
                    paused: false,
                    actor,
                },
            )
            .await
            .map(|account| vec![account]),
        RankedBotRequest::SetIq {
            account_id,
            iq,
            actor,
        } => state
            .db
            .update_ranked_bot(
                &account_id,
                sow_data::db::RankedBotManualUpdate::SetIq { iq, actor },
            )
            .await
            .map(|account| vec![account]),
        RankedBotRequest::SetRating {
            account_id,
            score,
            actor,
        } => state
            .db
            .update_ranked_bot(
                &account_id,
                sow_data::db::RankedBotManualUpdate::SetRating { score, actor },
            )
            .await
            .map(|account| vec![account]),
        RankedBotRequest::List | RankedBotRequest::Recover { .. } => unreachable!("handled above"),
    };
    match result {
        Ok(accounts) => (StatusCode::OK, Json(serde_json::json!({
            "accounts": accounts.into_iter().map(|account| account.without_auth_secret()).collect::<Vec<_>>()
        }))).into_response(),
        Err(error) => {
            error!("ranked bot operation failed: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "ranked bot operation failed".into() })).into_response()
        }
    }
}
async fn handle_internal_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let (calculations, payments) = match state.db.pending_reward_calculations() {
        Ok(pending) => pending,
        Err(error) => {
            error!("[rewards] pending calculation inventory failed: {error}");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    let checkpoint = if let Some(id) = query.get("match_id") {
        if id.len() > 20 || id.parse::<u64>().is_err() {
            return StatusCode::BAD_REQUEST.into_response();
        }
        match match_exit::checkpoint_rewards_terminal(&state, id).await {
            Ok(terminal) => Some(serde_json::json!({"match_id":id,"rewards_terminal":terminal})),
            Err(error) => {
                error!("[rewards] checkpoint receipt inventory failed match={id}: {error}");
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
        }
    } else {
        None
    };
    let db_path = std::path::Path::new(&state.redb_path);
    let file_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let season_rating_rows = state.db.redb_db.as_ref().and_then(|db| {
        let read_txn = db.begin_read().ok()?;
        let table = read_txn
            .open_table(sow_data::redb_store::SEASON_RATINGS_TABLE)
            .ok()?;
        table.len().ok()
    });

    Json(serde_json::json!({
        "checkpoint":checkpoint,
        "redb": {
            "path": state.redb_path,
            "file_size_bytes": file_size,
            "season_rating_rows": season_rating_rows,
        },
        "rewards": {
            "pending_calculations": calculations.len(),
            "pending_payments": payments.len(),
            "match_ids": calculations,
            "closing_match_ids": payments,
        }
    }))
    .into_response()
}

#[cfg(test)]
mod leader_rate_limit_tests {
    use super::*;

    #[test]
    fn allows_up_to_20_requests_per_minute() {
        let mut limits = HashMap::new();
        let base = Instant::now();
        for i in 0..20 {
            let t = base + Duration::from_millis(i * 100);
            assert!(AppState::eval_leader_rate_limit(&mut limits, "acc_1", t).is_ok());
        }
        let res =
            AppState::eval_leader_rate_limit(&mut limits, "acc_1", base + Duration::from_secs(2));
        assert_eq!(res.unwrap_err().0, StatusCode::TOO_MANY_REQUESTS);

        let res2 =
            AppState::eval_leader_rate_limit(&mut limits, "acc_1", base + Duration::from_secs(4));
        assert_eq!(res2.unwrap_err().0, StatusCode::TOO_MANY_REQUESTS);

        let res3 =
            AppState::eval_leader_rate_limit(&mut limits, "acc_1", base + Duration::from_secs(65));
        assert!(res3.is_ok());
    }

    #[test]
    fn blocks_for_5_minutes_on_tier2_offense() {
        let mut limits = HashMap::new();
        let base = Instant::now();
        for i in 0..60 {
            let t = base + Duration::from_secs(i * 3);
            let _ = AppState::eval_leader_rate_limit(&mut limits, "acc_2", t);
        }
        let t_61 = base + Duration::from_secs(181);
        let res = AppState::eval_leader_rate_limit(&mut limits, "acc_2", t_61);
        let err = res.unwrap_err();
        assert_eq!(err.0, StatusCode::TOO_MANY_REQUESTS);
        assert!(err.1.contains("locked for 5 minutes"));

        let res_during =
            AppState::eval_leader_rate_limit(&mut limits, "acc_2", t_61 + Duration::from_secs(100));
        assert_eq!(res_during.unwrap_err().0, StatusCode::TOO_MANY_REQUESTS);

        let res_after =
            AppState::eval_leader_rate_limit(&mut limits, "acc_2", t_61 + Duration::from_secs(305));
        assert!(res_after.is_ok());
    }

    #[test]
    fn suspends_for_3_days_on_third_tier2_offense() {
        let mut limits = HashMap::new();
        let base = Instant::now();

        for i in 0..61 {
            let t = base + Duration::from_millis(i * 10);
            let _ = AppState::eval_leader_rate_limit(&mut limits, "acc_3", t);
        }
        assert_eq!(limits.get("acc_3").unwrap().tier2_offenses.len(), 1);

        let t_hour1 = base + Duration::from_secs(3600);
        for i in 0..61 {
            let t = t_hour1 + Duration::from_millis(i * 10);
            let _ = AppState::eval_leader_rate_limit(&mut limits, "acc_3", t);
        }
        assert_eq!(limits.get("acc_3").unwrap().tier2_offenses.len(), 2);

        let t_hour2 = base + Duration::from_secs(7200);
        for i in 0..60 {
            let t = t_hour2 + Duration::from_millis(i * 10);
            let _ = AppState::eval_leader_rate_limit(&mut limits, "acc_3", t);
        }
        let res_tier3 = AppState::eval_leader_rate_limit(
            &mut limits,
            "acc_3",
            t_hour2 + Duration::from_millis(610),
        );
        let err = res_tier3.unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert!(err.1.contains("suspended for 3 days"));

        let res_suspended = AppState::eval_leader_rate_limit(
            &mut limits,
            "acc_3",
            t_hour2 + Duration::from_secs(86400),
        );
        assert_eq!(res_suspended.unwrap_err().0, StatusCode::FORBIDDEN);
    }
}

#[cfg(test)]
mod public_leaderboard_security_tests {
    use super::*;
    use axum::body::to_bytes;

    fn test_state(temp: &tempfile::TempDir) -> Arc<AppState> {
        let redb_path = temp.path().join("public-api-test.redb");
        let redb = Arc::new(sow_data::init_database(&redb_path).unwrap());
        let db = PlayerDb::new("redis://127.0.0.1:1/", None, Some(redb));
        db.ensure_current_season().unwrap();
        let auth = sow_data::auth::AuthService::new(
            db.clone(),
            "test-only-auth-secret-never-use-in-production".into(),
        )
        .unwrap();
        let analytics_dir = temp.path().join("analytics");
        std::fs::create_dir_all(&analytics_dir).unwrap();
        let legacy_events = sow_data::events::EventSink::new(analytics_dir.to_str().unwrap()).unwrap();
        let (replay_verification_tx, _rx) = tokio::sync::mpsc::channel(4);
        let replay_verification_client = reqwest::Client::builder().no_proxy().build().unwrap();
        Arc::new(AppState {
            db,
            auth,
            secret_token: "test-secret".into(),
            relay_settlement_secret: "relay-secret".into(),
            revenuecat_webhook_secret: None,
            redb_path: redb_path.to_string_lossy().into_owned(),
            legacy_events: std::sync::Mutex::new(legacy_events),
            playgames_handoffs: std::sync::Mutex::new(HashMap::new()),
            playgames_sessions: std::sync::Mutex::new(HashMap::new()),
            playgames_access_tokens: std::sync::Mutex::new(HashMap::new()),
            replay_verification_tx,
            replay_verification_client,
            match_artifact_lock: tokio::sync::Mutex::new(()),
            match_results: tokio::sync::broadcast::channel(8).0,
            leader_rate_limits: std::sync::Mutex::new(HashMap::new()),
            leaderboard_cache: tokio::sync::RwLock::new(HashMap::new()),
            public_profile_cache: std::sync::Mutex::new(HashMap::new()),
            public_api_rate_limits: std::sync::Mutex::new(HashMap::new()),
        })
    }

    async fn read_json(response: axum::response::Response) -> (StatusCode, HeaderMap, serde_json::Value) {
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[test]
    fn public_rate_limiter_enforces_limits_per_ip_and_recovers() {
        let mut limits = HashMap::new();
        let base = Instant::now();
        for i in 0..25 {
            assert!(eval_public_rate_limit(&mut limits, "search:1.2.3.4", 25, base + Duration::from_millis(i)).is_ok());
        }
        assert!(eval_public_rate_limit(&mut limits, "search:1.2.3.4", 25, base + Duration::from_secs(5)).is_err());
        assert!(eval_public_rate_limit(&mut limits, "search:5.6.7.8", 25, base + Duration::from_secs(5)).is_ok());
        assert!(eval_public_rate_limit(&mut limits, "search:1.2.3.4", 25, base + Duration::from_secs(62)).is_ok());
    }

    #[tokio::test]
    async fn public_endpoints_validate_inputs_filter_cache_etag_and_hide_private_fields() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_state(&temp);

        let a1 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let a2 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let mut p1_profile = sow_data::PlayerProfile::default();
        p1_profile.crowns = 9999;
        p1_profile.gems = 888;
        p1_profile.level = 12;
        p1_profile.preferred_leader = Some("Caesar".into());
        p1_profile.verified_wins = 15;
        p1_profile.wins = 15;
        p1_profile.matches_played = 20;
        let p1 = sow_data::PlayerAccount {
            id: a1.into(),
            display_name: "Diogenes".into(),
            profile: p1_profile,
            linked_identities: vec![sow_data::LinkedIdentity {
                provider: "email".into(),
                external_id: "private@example.com".into(),
                environment: "production".into(),
            }],
            kind: sow_data::db::AccountKind::Human,
            auth_secret_hash: Some("top-secret-token-hash".into()),
            created_at: 100,
            updated_at: 100,
        };

        let mut p2_profile = sow_data::PlayerProfile::default();
        p2_profile.level = 7;
        p2_profile.preferred_leader = Some("Alexander".into());
        p2_profile.verified_wins = 4;
        p2_profile.wins = 4;
        p2_profile.matches_played = 12;
        let p2 = sow_data::PlayerAccount {
            id: a2.into(),
            display_name: "Alexander".into(),
            profile: p2_profile,
            linked_identities: Vec::new(),
            kind: sow_data::db::AccountKind::Human,
            auth_secret_hash: None,
            created_at: 100,
            updated_at: 100,
        };

        let redb = state.db.redb_db.as_ref().unwrap();
        {
            let txn = redb.begin_write().unwrap();
            let mut r1 = sow_data::profile::initial_ranked_rating(a1.into(), 1400.0, 100);
            r1.score = 9200;
            r1.peak_score = 9400;
            r1.games_played = 20;
            r1.wins = 15;
            r1.normalize_ladder();
            let mut r2 = sow_data::profile::initial_ranked_rating(a2.into(), 1200.0, 100);
            r2.score = 4500;
            r2.peak_score = 4800;
            r2.games_played = 12;
            r2.wins = 4;
            r2.normalize_ladder();
            {
                let mut players = txn.open_table(sow_data::redb_store::PLAYERS_TABLE).unwrap();
                players.insert(p1.id.as_str(), serde_json::to_vec(&p1).unwrap().as_slice()).unwrap();
                players.insert(p2.id.as_str(), serde_json::to_vec(&p2).unwrap().as_slice()).unwrap();
            }
            {
                let mut profiles = txn.open_table(sow_data::redb_store::PUBLIC_PROFILES_TABLE).unwrap();
                for (acc, score) in [(&p1, r1.score), (&p2, r2.score)] {
                    let index = sow_data::PublicProfileIndex {
                        account_id: acc.id.clone(),
                        display_name: acc.display_name.clone(),
                        kind: "human".into(),
                        verified_wins: acc.profile.verified_wins,
                        updated_at: acc.updated_at,
                        level: acc.profile.level,
                        matches_played: acc.profile.matches_played,
                        preferred_leader: acc.profile.preferred_leader.clone(),
                        ranked_1v1_score: score,
                        ranked_ffa_score: 0,
                    };
                    profiles.insert(acc.id.as_str(), serde_json::to_vec(&index).unwrap().as_slice()).unwrap();
                }
            }
            {
                let mut ratings = txn.open_table(sow_data::redb_store::SEASON_RATINGS_TABLE).unwrap();
                ratings.insert(format!("rating:{a1}:1:Ranked1v1:FFA").as_str(), serde_json::to_vec(&r1).unwrap().as_slice()).unwrap();
                ratings.insert(format!("rating:{a2}:1:Ranked1v1:FFA").as_str(), serde_json::to_vec(&r2).unwrap().as_slice()).unwrap();
            }
            {
                let mut board = txn.open_table(sow_data::redb_store::RANKED_LEADERBOARD_TABLE).unwrap();
                for (acc, rating) in [(&p1, &r1), (&p2, &r2)] {
                    let key = sow_data::redb_store::ranked_leaderboard_key(1, "Ranked1v1", "FFA", rating.score, rating.wins, &acc.id);
                    let entry = sow_data::PublicLeaderboardEntry {
                        rank: 0,
                        account_id: acc.id.clone(),
                        handle: sow_data::profile::public_handle(&acc.display_name, &acc.id),
                        is_bot: false,
                        queue: "Ranked1v1".into(),
                        mode: "FFA".into(),
                        score: rating.score,
                        tier: rating.tier.clone(),
                        division: rating.division.clone(),
                        games_played: rating.games_played,
                        wins: rating.wins,
                        peak_score: rating.peak_score,
                        win_rate: sow_data::profile::win_rate(rating.wins, rating.games_played),
                        win_streak: 0,
                        level: acc.profile.level,
                        preferred_leader: acc.profile.preferred_leader.clone(),
                        updated_at: rating.updated_at,
                    };
                    board.insert(key.as_str(), serde_json::to_vec(&entry).unwrap().as_slice()).unwrap();
                }
            }
            txn.commit().unwrap();
        }

        // 1. Valid filtered query returns ETag, tier_counts, total_count, and matches filter.
        let (status, headers, json) = read_json(
            handle_public_leaderboard(
                State(state.clone()),
                HeaderMap::new(),
                Path(1),
                Query(PublicLeaderboardQuery {
                    queue: Some("Ranked1v1".into()),
                    tier: Some("deity".into()),
                    leader: Some("caesar".into()),
                    min_games: Some(15),
                    ..Default::default()
                }),
            )
            .await
            .into_response(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["total_count"], 1);
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        assert_eq!(json["items"][0]["account_id"], a1);
        assert_eq!(json["tier_counts"]["deity"], 1);
        assert!(headers.contains_key(header::ETAG));
        assert!(headers.contains_key(header::CACHE_CONTROL));

        // 2. If-None-Match yields 304 Not Modified.
        let etag = headers.get(header::ETAG).unwrap().clone();
        let mut inm_headers = HeaderMap::new();
        inm_headers.insert(header::IF_NONE_MATCH, etag);
        let (inm_status, _, _) = read_json(
            handle_public_leaderboard(
                State(state.clone()),
                inm_headers,
                Path(1),
                Query(PublicLeaderboardQuery {
                    queue: Some("Ranked1v1".into()),
                    tier: Some("deity".into()),
                    leader: Some("caesar".into()),
                    min_games: Some(15),
                    ..Default::default()
                }),
            )
            .await
            .into_response(),
        )
        .await;
        assert_eq!(inm_status, StatusCode::NOT_MODIFIED);

        // 3. Invalid params return 400 Bad Request.
        for bad_query in [
            PublicLeaderboardQuery { queue: Some("InvalidQueue".into()), ..Default::default() },
            PublicLeaderboardQuery { tier: Some("challenger".into()), ..Default::default() },
            PublicLeaderboardQuery { sort: Some("drop_table".into()), ..Default::default() },
            PublicLeaderboardQuery { leader: Some("nonexistent_hero".into()), ..Default::default() },
            PublicLeaderboardQuery { q: Some("x".repeat(49)), ..Default::default() },
        ] {
            let (bad_status, _, _) = read_json(
                handle_public_leaderboard(State(state.clone()), HeaderMap::new(), Path(1), Query(bad_query))
                    .await
                    .into_response(),
            )
            .await;
            assert_eq!(bad_status, StatusCode::BAD_REQUEST);
        }

        // 4. Invalid account_id & match_id return 400 Bad Request.
        let (bad_acc_status, _, _) = read_json(
            handle_public_profile(State(state.clone()), HeaderMap::new(), Path("../etc/passwd".into()))
                .await
                .into_response(),
        )
        .await;
        assert_eq!(bad_acc_status, StatusCode::BAD_REQUEST);

        let (bad_match_status, _, _) = read_json(
            handle_public_match_detail(State(state.clone()), HeaderMap::new(), Path("bad match!".into()))
                .await
                .into_response(),
        )
        .await;
        assert_eq!(bad_match_status, StatusCode::BAD_REQUEST);

        // 5. Public profile response strictly excludes secrets/balances/PII.
        let (prof_status, _, prof_json) = read_json(
            handle_public_profile(State(state.clone()), HeaderMap::new(), Path(a1.into()))
                .await
                .into_response(),
        )
        .await;
        assert_eq!(prof_status, StatusCode::OK);
        let raw_text = prof_json.to_string();
        assert!(!raw_text.contains("top-secret-token"));
        assert!(!raw_text.contains("private@example.com"));
        assert!(prof_json.get("auth_secret").is_none());
        assert!(prof_json.get("auth_secret_hash").is_none());
        assert!(prof_json.get("linked_identities").is_none());
        assert!(prof_json.get("crowns").is_none());
        assert!(prof_json.get("gems").is_none());

        // 6. /seasons/current/leaderboard alias resolves current season cleanly.
        let (cur_status, _, cur_json) = read_json(
            handle_current_season_leaderboard(
                State(state.clone()),
                HeaderMap::new(),
                Query(PublicLeaderboardQuery {
                    queue: Some("Ranked1v1".into()),
                    limit: Some(10),
                    ..Default::default()
                }),
            )
            .await
            .into_response(),
        )
        .await;
        assert_eq!(cur_status, StatusCode::OK);
        assert_eq!(cur_json["season_id"], sow_data::profile::CURRENT_SEASON_ID);
        assert_eq!(cur_json["total_count"], 2);
        assert_eq!(cur_json["items"][0]["account_id"], a1);
    }
}
