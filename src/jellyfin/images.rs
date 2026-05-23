//! Image proxy: `/Items/{itemId}/Images/{imageType}`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use super::{JellyfinAppState, auth::JellyfinAuth};

/// `GET /jellyfin/Items/{itemId}/Images/{imageType}`
pub async fn get_item_image<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path((item_id, image_type)): Path<(String, String)>,
) -> impl IntoResponse {
    serve_image(state, &item_id, &image_type).await
}

/// `GET /jellyfin/Items/{itemId}/Images/{imageType}/{imageIndex}`
pub async fn get_item_image_by_index<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path((item_id, image_type, _index)): Path<(String, String, String)>,
) -> impl IntoResponse {
    serve_image(state, &item_id, &image_type).await
}

async fn serve_image<S: JellyfinAppState>(state: Arc<S>, item_id: &str, image_type: &str) -> axum::response::Response {
    let Ok(uid) = item_id.parse::<Uuid>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let db = state.db();
    let art_type = match image_type.to_lowercase().as_str() {
        "backdrop" | "banner" => "backdrop",
        "thumb" => "thumb",
        // "primary", "poster", and anything else → poster
        _ => "poster",
    };

    // 1. Try person image (movie_persons / tv_persons)
    //    The thumb/person endpoint computes the key from UUID directly — no source needed.
    if art_type == "poster" && try_is_person(db, uid).await {
        let w = 400;
        let thumb_url = format!("/api/thumb/person/{item_id}?w={w}");
        return Redirect::temporary(&thumb_url).into_response();
    }

    // 2. Try media_arts table (selected art for this entity)
    let url = try_media_arts(db, uid, art_type).await;

    // 3. Fallback: check entity tables for poster_path / backdrop_path / still_path
    let url = match url {
        Some(u) => Some(u),
        None => try_entity_image(db, uid, art_type).await,
    };

    match url {
        Some(url) => {
            let entity_type = detect_entity_type(db, uid).await.unwrap_or("movie".to_string());
            // Thumb endpoint requires w= (width) parameter; use sensible defaults
            let w = match art_type {
                "backdrop" => 1280,
                _ => 400,
            };
            let thumb_url = format!(
                "/api/thumb/{entity_type}/{item_id}?source={}&w={w}",
                urlencoding::encode(&url)
            );
            Redirect::temporary(&thumb_url).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn try_media_arts(db: &sea_orm::DatabaseConnection, entity_id: Uuid, art_type: &str) -> Option<String> {
    let sql = r"
        SELECT url FROM media_arts
        WHERE (movie_id = $1 OR tv_show_id = $1 OR season_id = $1 OR album_id = $1)
          AND art_type = $2
          AND is_selected = true
        LIMIT 1
    ";
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [entity_id.into(), art_type.into()],
        ))
        .await
        .ok()
        .flatten()?;
    row.try_get::<String>("", "url").ok()
}

async fn try_entity_image(db: &sea_orm::DatabaseConnection, entity_id: Uuid, art_type: &str) -> Option<String> {
    let column = match art_type {
        "backdrop" => "backdrop_path",
        _ => "poster_path",
    };

    // Try movies
    let sql = format!("SELECT {column} FROM movies WHERE id = $1");
    if let Ok(Some(r)) = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            &sql,
            [entity_id.into()],
        ))
        .await
        && let Ok(Some(path)) = r.try_get::<Option<String>>("", column)
    {
        return Some(path);
    }

    // Try tv_shows
    let sql = format!("SELECT {column} FROM tv_shows WHERE id = $1");
    if let Ok(Some(r)) = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            &sql,
            [entity_id.into()],
        ))
        .await
        && let Ok(Some(path)) = r.try_get::<Option<String>>("", column)
    {
        return Some(path);
    }

    // Try seasons (poster_path only)
    if art_type != "backdrop" {
        let sql = "SELECT poster_path FROM seasons WHERE id = $1";
        if let Ok(Some(r)) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [entity_id.into()],
            ))
            .await
            && let Ok(Some(path)) = r.try_get::<Option<String>>("", "poster_path")
        {
            return Some(path);
        }
    }

    // Try episodes (still_path)
    let sql = "SELECT still_path FROM episodes WHERE id = $1";
    if let Ok(Some(r)) = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [entity_id.into()],
        ))
        .await
        && let Ok(Some(path)) = r.try_get::<Option<String>>("", "still_path")
    {
        return Some(path);
    }

    None
}

async fn detect_entity_type(db: &sea_orm::DatabaseConnection, entity_id: Uuid) -> Option<String> {
    let checks = [
        ("SELECT id FROM movies WHERE id = $1", "movie"),
        ("SELECT id FROM tv_shows WHERE id = $1", "tv_show"),
        ("SELECT id FROM seasons WHERE id = $1", "season"),
        ("SELECT id FROM episodes WHERE id = $1", "episode"),
    ];
    for (sql, etype) in &checks {
        if db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                *sql,
                [entity_id.into()],
            ))
            .await
            .ok()
            .flatten()
            .is_some()
        {
            return Some((*etype).to_string());
        }
    }
    None
}

/// Returns true if `entity_id` belongs to a person (`movie_persons` or `tv_persons`)
/// that has a non-empty `profile_path` stored on disk.
async fn try_is_person(db: &sea_orm::DatabaseConnection, entity_id: Uuid) -> bool {
    let checks = [
        "SELECT id FROM movie_persons WHERE id = $1 AND profile_path IS NOT NULL AND profile_path <> ''",
        "SELECT id FROM tv_persons WHERE id = $1 AND profile_path IS NOT NULL AND profile_path <> ''",
    ];
    for sql in &checks {
        if db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                *sql,
                [entity_id.into()],
            ))
            .await
            .ok()
            .flatten()
            .is_some()
        {
            return true;
        }
    }
    false
}
