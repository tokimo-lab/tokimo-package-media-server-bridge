//! Jellyfin `Authorization: MediaBrowser …` header + `?api_key=` query parsing.

use std::sync::Arc;

use axum::{
    Json,
    extract::{FromRequestParts, Query},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use sea_orm::*;
use uuid::Uuid;

use super::{JellyfinAppState, JellyfinUser};

/// Extract the token from a MediaBrowser Authorization header.
///
/// Format: `MediaBrowser Client="…", Device="…", DeviceId="…", Version="…", Token="<token>"`
/// Also: `Emby Client="…", …, Token="<token>"`
pub fn parse_mediabrowser_token(header_value: &str) -> Option<String> {
    let s = header_value.trim();
    // Strip scheme prefix
    let body = s.strip_prefix("MediaBrowser ").or_else(|| s.strip_prefix("Emby "))?;

    for part in body.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("Token=") {
            let token = rest.trim().trim_matches('"');
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Parse `Client`, `Device`, `DeviceId`, `Version` from the MediaBrowser header.
pub fn parse_mediabrowser_header(header_value: &str) -> MediaBrowserClientInfo {
    let mut info = MediaBrowserClientInfo::default();
    let s = header_value.trim();
    let body = s
        .strip_prefix("MediaBrowser ")
        .or_else(|| s.strip_prefix("Emby "))
        .unwrap_or(s);

    for part in body.split(',') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once('=') {
            let val = val.trim().trim_matches('"');
            match key.trim() {
                "Client" => info.client = val.to_string(),
                "Device" => info.device = val.to_string(),
                "DeviceId" => info.device_id = val.to_string(),
                "Version" => info.version = val.to_string(),
                _ => {}
            }
        }
    }
    info
}

#[derive(Debug, Default, Clone)]
pub struct MediaBrowserClientInfo {
    pub client: String,
    pub device: String,
    pub device_id: String,
    pub version: String,
}

// ── DB: api_keys table ────────────────────────────────────────────────────────

/// Resolve a bearer token to a user. Looks up `api_keys.note = token` (plaintext
/// for simplicity; the real Jellyfin also stores tokens in plaintext).
pub async fn resolve_token(db: &DatabaseConnection, token: &str) -> Result<Option<(Uuid, String)>, DbErr> {
    // We use SeaORM raw-ish approach via entities. We need access to api_keys
    // and users entities from the host crate, but we don't have them here.
    // Instead, use raw SQL.
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r"SELECT ak.user_id, u.name
           FROM api_keys ak
           JOIN users u ON u.id = ak.user_id
           WHERE ak.note = $1
           AND (ak.expires_at IS NULL OR ak.expires_at > NOW())
           LIMIT 1",
        [token.into()],
    );
    let row = db.query_one_raw(stmt).await?;
    match row {
        Some(r) => {
            let user_id: Uuid = r.try_get("", "user_id")?;
            let name: String = r.try_get("", "name")?;
            Ok(Some((user_id, name)))
        }
        None => Ok(None),
    }
}

/// Store a new Jellyfin token into api_keys.
pub async fn store_token(db: &DatabaseConnection, user_id: Uuid, token: &str, device_id: &str) -> Result<(), DbErr> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let id = Uuid::new_v4();
    let name = format!("jellyfin:{device_id}");
    let key_prefix = &token[..usize::min(8, token.len())];

    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r"INSERT INTO api_keys (id, user_id, name, key_hash, key_prefix, note, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, NOW())",
        [
            id.into(),
            user_id.into(),
            name.into(),
            token.into(), // key_hash = token (simplified)
            key_prefix.into(),
            token.into(), // note = plaintext token for lookup
        ],
    );
    db.execute_raw(stmt).await?;
    Ok(())
}

// ── Axum extractor ────────────────────────────────────────────────────────────

/// Axum extractor that reads the Jellyfin token from:
/// 1. `Authorization: MediaBrowser … Token="<token>"`
/// 2. `X-Emby-Token: <token>` header
/// 3. `?api_key=<token>` query param
pub struct JellyfinAuth<S: JellyfinAppState>(pub JellyfinUser, pub std::marker::PhantomData<S>);

#[derive(serde::Deserialize, Default)]
struct ApiKeyQuery {
    api_key: Option<String>,
}

impl<S: JellyfinAppState> FromRequestParts<Arc<S>> for JellyfinAuth<S> {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<S>) -> Result<Self, Self::Rejection> {
        // 1. Authorization header
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_mediabrowser_token);

        // 2. X-Emby-Token header
        let token = token.or_else(|| {
            parts
                .headers
                .get("x-emby-token")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });

        // 3. ?api_key= query
        let token = if let Some(t) = token {
            Some(t)
        } else {
            let query: Query<ApiKeyQuery> = Query::try_from_uri(&parts.uri).unwrap_or_default();
            query.api_key.clone()
        };

        let Some(token) = token else {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Missing authentication token"})),
            )
                .into_response());
        };

        let db = state.db();
        match resolve_token(db, &token).await {
            Ok(Some((user_id, user_name))) => Ok(Self(
                JellyfinUser {
                    user_id,
                    user_name,
                    access_token: token,
                },
                std::marker::PhantomData,
            )),
            Ok(None) => Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid or expired token"})),
            )
                .into_response()),
            Err(e) => {
                tracing::error!("jellyfin auth db error: {e}");
                Err(StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }
        }
    }
}
