//! Session reporting and played-state management.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use super::{
    JellyfinAppState, JellyfinPlaybackSession,
    auth::{JellyfinAuth, parse_mediabrowser_header},
    models::{PlaybackProgressInfo, PlaybackStartInfo, PlaybackStopInfo, UserItemDataDto},
};

fn ticks_to_seconds(ticks: Option<i64>) -> i32 {
    ticks.map_or(0, |t| (t / 10_000_000) as i32)
}

/// `POST /jellyfin/Sessions/Playing`
pub async fn on_playback_start<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    headers: HeaderMap,
    Json(body): Json<PlaybackStartInfo>,
) -> impl IntoResponse {
    let position = ticks_to_seconds(body.position_ticks);
    upsert_playback_state(state.db(), user.user_id, body.item_id, position, false).await;

    if let Some(file_id_str) = &body.media_source_id
        && let Ok(file_id) = file_id_str.parse::<Uuid>()
    {
        let ua = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let client_name = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| parse_mediabrowser_header(v).client)
            .filter(|s| !s.is_empty());
        state
            .create_playback_session(JellyfinPlaybackSession {
                user_id: user.user_id,
                file_id,
                client_name,
                user_agent: ua,
                position,
            })
            .await;
    }

    StatusCode::NO_CONTENT.into_response()
}

/// `POST /jellyfin/Sessions/Playing/Progress`
pub async fn on_playback_progress<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Json(body): Json<PlaybackProgressInfo>,
) -> impl IntoResponse {
    let position = ticks_to_seconds(body.position_ticks);
    upsert_playback_state(state.db(), user.user_id, body.item_id, position, false).await;

    if let Some(file_id_str) = &body.media_source_id
        && let Ok(file_id) = file_id_str.parse::<Uuid>()
    {
        state
            .update_playback_session_progress(user.user_id, file_id, position)
            .await;
    }

    StatusCode::NO_CONTENT.into_response()
}

/// `POST /jellyfin/Sessions/Playing/Stopped`
pub async fn on_playback_stopped<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Json(body): Json<PlaybackStopInfo>,
) -> impl IntoResponse {
    let position = ticks_to_seconds(body.position_ticks);
    upsert_playback_state(state.db(), user.user_id, body.item_id, position, false).await;

    if let Some(file_id_str) = &body.media_source_id
        && let Ok(file_id) = file_id_str.parse::<Uuid>()
    {
        state.stop_playback_session(user.user_id, file_id, position).await;
    }

    StatusCode::NO_CONTENT.into_response()
}

/// `POST /jellyfin/UserPlayedItems/{itemId}`
pub async fn mark_played<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(item_id): Path<Uuid>,
) -> impl IntoResponse {
    upsert_playback_state(state.db(), user.user_id, item_id, 0, true).await;
    Json(UserItemDataDto {
        played_percentage: Some(100.0),
        playback_position_ticks: 0,
        play_count: 1,
        is_favorite: false,
        played: true,
        last_played_date: None,
        unplayed_item_count: None,
        key: item_id.to_string(),
        item_id: item_id.to_string(),
    })
}

/// `DELETE /jellyfin/UserPlayedItems/{itemId}`
pub async fn mark_unplayed<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(item_id): Path<Uuid>,
) -> impl IntoResponse {
    mark_unplayed_inner(state.db(), user.user_id, item_id).await;
    Json(UserItemDataDto {
        played_percentage: Some(0.0),
        playback_position_ticks: 0,
        play_count: 0,
        is_favorite: false,
        played: false,
        last_played_date: None,
        unplayed_item_count: None,
        key: item_id.to_string(),
        item_id: item_id.to_string(),
    })
}

/// `POST /jellyfin/Users/{userId}/PlayedItems/{itemId}` (legacy)
pub async fn mark_played_legacy<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path((_user_id, item_id)): Path<(String, Uuid)>,
) -> impl IntoResponse {
    upsert_playback_state(state.db(), user.user_id, item_id, 0, true).await;
    Json(UserItemDataDto {
        played_percentage: Some(100.0),
        playback_position_ticks: 0,
        play_count: 1,
        is_favorite: false,
        played: true,
        last_played_date: None,
        unplayed_item_count: None,
        key: item_id.to_string(),
        item_id: item_id.to_string(),
    })
}

/// `DELETE /jellyfin/Users/{userId}/PlayedItems/{itemId}` (legacy)
pub async fn mark_unplayed_legacy<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path((_user_id, item_id)): Path<(String, Uuid)>,
) -> impl IntoResponse {
    mark_unplayed_inner(state.db(), user.user_id, item_id).await;
    Json(UserItemDataDto {
        played_percentage: Some(0.0),
        playback_position_ticks: 0,
        play_count: 0,
        is_favorite: false,
        played: false,
        last_played_date: None,
        unplayed_item_count: None,
        key: item_id.to_string(),
        item_id: item_id.to_string(),
    })
}

// ── helpers ──────────────────────────────────────────────────────

/// Determine whether `item_id` is a movie or episode and upsert `user_media_states`.
async fn upsert_playback_state(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    item_id: Uuid,
    position_seconds: i32,
    mark_watched: bool,
) {
    let is_movie = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM video_items WHERE id = $1",
            [item_id.into()],
        ))
        .await
        .ok()
        .flatten()
        .is_some();

    let is_episode = if is_movie {
        false
    } else {
        db.query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM episodes WHERE id = $1",
            [item_id.into()],
        ))
        .await
        .ok()
        .flatten()
        .is_some()
    };

    if !is_movie && !is_episode {
        return;
    }

    let now = Utc::now();
    let (fk_col, fk_unique) = if is_movie {
        ("video_item_id", "user_id, video_item_id")
    } else {
        ("episode_id", "user_id, episode_id")
    };

    let watched = if mark_watched { "true" } else { "false" };

    let sql = format!(
        r"
        INSERT INTO user_media_states (id, user_id, {fk_col}, resume_position, play_count, is_watched, last_watch_at, updated_at)
        VALUES ($1, $2, $3, $4, 1, {watched}, $5, $5)
        ON CONFLICT ({fk_unique})
        DO UPDATE SET
            resume_position = $4,
            play_count = user_media_states.play_count + 1,
            is_watched = CASE WHEN {watched} THEN true ELSE user_media_states.is_watched END,
            last_watch_at = $5,
            updated_at = $5
        "
    );

    let _ = db
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            &sql,
            [
                Uuid::new_v4().into(),
                user_id.into(),
                item_id.into(),
                position_seconds.into(),
                now.into(),
            ],
        ))
        .await;
}

async fn mark_unplayed_inner(db: &sea_orm::DatabaseConnection, user_id: Uuid, item_id: Uuid) {
    let now = Utc::now();
    let sql = r"
        UPDATE user_media_states
        SET is_watched = false, resume_position = 0, updated_at = $3
        WHERE user_id = $1 AND (video_item_id = $2 OR episode_id = $2)
    ";
    let _ = db
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [user_id.into(), item_id.into(), now.into()],
        ))
        .await;
}
