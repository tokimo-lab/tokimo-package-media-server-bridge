//! User endpoints: `/Users/Public`, `/Users/AuthenticateByName`, `/Users/Me`, `/Users/{userId}`.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use super::{
    JellyfinAppState,
    auth::{self, JellyfinAuth, parse_mediabrowser_header},
    models::{
        AuthenticateByNameRequest, AuthenticationResult, PlayStateDto, SessionCapabilities, SessionInfoDto,
        UserConfiguration, UserDto, UserPolicy,
    },
};

fn user_dto(user_id: Uuid, name: &str, last_login: Option<String>, server_id: &str) -> UserDto {
    UserDto {
        name: name.to_string(),
        server_id: server_id.to_string(),
        id: user_id.to_string().replace('-', ""),
        has_password: true,
        has_configured_password: true,
        has_configured_easy_password: false,
        enable_auto_login: false,
        last_login_date: last_login.clone(),
        last_activity_date: last_login,
        configuration: UserConfiguration::default(),
        policy: UserPolicy::default(),
    }
}

/// `GET /jellyfin/Users/Public`
pub async fn get_public_users<S: JellyfinAppState>(State(state): State<Arc<S>>) -> impl IntoResponse {
    let db = state.db();
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id, name, last_login_at FROM users ORDER BY name",
        [],
    );
    match db.query_all_raw(stmt).await {
        Ok(rows) => {
            let users: Vec<UserDto> = rows
                .iter()
                .filter_map(|r| {
                    let id: Uuid = r.try_get("", "id").ok()?;
                    let name: String = r.try_get("", "name").ok()?;
                    let last_login: Option<chrono::DateTime<chrono::FixedOffset>> =
                        r.try_get("", "last_login_at").ok().flatten();
                    Some(user_dto(
                        id,
                        &name,
                        last_login.map(|d| d.to_rfc3339()),
                        state.server_id(),
                    ))
                })
                .collect();
            Json(users).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_public_users: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `POST /jellyfin/Users/AuthenticateByName`
pub async fn authenticate_by_name<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    headers: HeaderMap,
    Json(body): Json<AuthenticateByNameRequest>,
) -> impl IntoResponse {
    let db = state.db();

    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id, name, email, password_hash, last_login_at FROM users WHERE name = $1 OR email = $1 LIMIT 1",
        [body.username.clone().into()],
    );
    let row = match db.query_one_raw(stmt).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid username or password"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("jellyfin auth db error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let user_id: Uuid = row.try_get("", "id").unwrap();
    let name: String = row.try_get("", "name").unwrap();
    let password_hash: String = row.try_get("", "password_hash").unwrap();
    let last_login: Option<chrono::DateTime<chrono::FixedOffset>> = row.try_get("", "last_login_at").ok().flatten();

    if !verify_password(&body.pw, &password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid username or password"})),
        )
            .into_response();
    }

    let client_info = headers
        .get("authorization")
        .or_else(|| headers.get("x-emby-authorization"))
        .and_then(|v| v.to_str().ok())
        .map(parse_mediabrowser_header)
        .unwrap_or_default();

    let device_id = if client_info.device_id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        client_info.device_id.clone()
    };

    let token = Uuid::new_v4().to_string().replace('-', "");

    if let Err(e) = auth::store_token(db, user_id, &token, &device_id).await {
        tracing::error!("jellyfin store token: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let now = chrono::Utc::now().to_rfc3339();
    let user = user_dto(user_id, &name, last_login.map(|d| d.to_rfc3339()), state.server_id());

    let user_id_str = user.id.clone();
    let session_id = Uuid::new_v4().to_string().replace('-', "");

    let session_info = SessionInfoDto {
        play_state: PlayStateDto::default(),
        additional_users: vec![],
        capabilities: SessionCapabilities::default(),
        remote_end_point: String::new(),
        playable_media_types: vec![],
        id: session_id,
        user_id: user_id_str,
        user_name: name,
        client: client_info.client.clone(),
        last_activity_date: now.clone(),
        last_playback_check_in: "0001-01-01T00:00:00.0000000Z".to_string(),
        device_name: client_info.device.clone(),
        device_id,
        application_version: client_info.version.clone(),
        is_active: true,
        supports_media_control: false,
        supports_remote_control: false,
        now_playing_queue: vec![],
        now_playing_queue_full_items: vec![],
        has_custom_device_name: false,
        server_id: state.server_id().to_string(),
        supported_commands: vec![],
    };

    Json(AuthenticationResult {
        user,
        session_info,
        access_token: token,
        server_id: state.server_id().to_string(),
    })
    .into_response()
}

/// `GET /jellyfin/Users/Me`
pub async fn get_me<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    let db = state.db();
    match fetch_user(db, user.user_id, state.server_id()).await {
        Ok(Some(dto)) => Json(dto).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("jellyfin get_me: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Users/{userId}`
pub async fn get_user<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let db = state.db();
    match fetch_user(db, user_id, state.server_id()).await {
        Ok(Some(dto)) => Json(dto).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("jellyfin get_user: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_user(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    server_id: &str,
) -> Result<Option<UserDto>, sea_orm::DbErr> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id, name, last_login_at FROM users WHERE id = $1",
        [user_id.into()],
    );
    let row = db.query_one_raw(stmt).await?;
    Ok(row.map(|r| {
        let id: Uuid = r.try_get("", "id").unwrap();
        let name: String = r.try_get("", "name").unwrap();
        let last_login: Option<chrono::DateTime<chrono::FixedOffset>> = r.try_get("", "last_login_at").ok().flatten();
        user_dto(id, &name, last_login.map(|d| d.to_rfc3339()), server_id)
    }))
}

/// Minimal password verification (argon2 or plaintext legacy).
fn verify_password(password: &str, stored: &str) -> bool {
    if stored.starts_with("$argon2") {
        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        if let Ok(parsed) = PasswordHash::new(stored) {
            return Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok();
        }
        false
    } else {
        password == stored
    }
}
