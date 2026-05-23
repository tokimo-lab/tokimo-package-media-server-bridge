//! Items endpoints: `/UserViews`, `/Items`, `/Items/{itemId}`,
//! `/Shows/{seriesId}/Seasons`, `/Shows/{seriesId}/Episodes`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Datelike;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::{
    JellyfinAppState,
    auth::JellyfinAuth,
    models::{BaseItemDto, MediaSourceInfo, QueryResult, UserItemDataDto},
};

// ── TMDB genre ID → name (mirrors rust-server's tmdb_genre_name) ─────────────

#[allow(dead_code)]
pub(crate) fn tmdb_genre_name(id: i32) -> &'static str {
    match id {
        12 => "Adventure",
        14 => "Fantasy",
        16 => "Animation",
        18 => "Drama",
        27 => "Horror",
        28 => "Action",
        35 => "Comedy",
        36 => "History",
        37 => "Western",
        53 => "Thriller",
        80 => "Crime",
        99 => "Documentary",
        878 => "Science Fiction",
        9648 => "Mystery",
        10402 => "Music",
        10749 => "Romance",
        10751 => "Family",
        10752 => "War",
        10759 => "Action & Adventure",
        10762 => "Kids",
        10763 => "News",
        10764 => "Reality",
        10765 => "Sci-Fi & Fantasy",
        10766 => "Soap",
        10767 => "Talk",
        10768 => "War & Politics",
        10770 => "TV Movie",
        _ => "Unknown",
    }
}

pub(crate) fn seconds_to_ticks(secs: i32) -> i64 {
    i64::from(secs) * 10_000_000
}

#[allow(dead_code)]
pub(crate) fn ticks_to_seconds(ticks: i64) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let secs = (ticks / 10_000_000) as i32;
    secs
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct ItemsQuery {
    #[serde(alias = "parentId", alias = "ParentId")]
    pub parent_id: Option<String>,
    #[serde(alias = "includeItemTypes", alias = "IncludeItemTypes")]
    pub include_item_types: Option<String>,
    #[serde(alias = "Recursive")]
    pub recursive: Option<bool>,
    #[serde(alias = "sortBy", alias = "SortBy")]
    pub sort_by: Option<String>,
    #[serde(alias = "sortOrder", alias = "SortOrder")]
    pub sort_order: Option<String>,
    #[serde(alias = "startIndex", alias = "StartIndex")]
    pub start_index: Option<i64>,
    #[serde(alias = "Limit")]
    pub limit: Option<i64>,
    #[serde(alias = "Fields")]
    pub fields: Option<String>,
    #[serde(alias = "searchTerm", alias = "SearchTerm")]
    pub search_term: Option<String>,
    #[serde(alias = "Filters")]
    pub filters: Option<String>,
    #[serde(alias = "userId", alias = "UserId")]
    pub user_id: Option<String>,
    #[serde(alias = "enableUserData", alias = "EnableUserData")]
    pub enable_user_data: Option<bool>,
    #[serde(alias = "enableImages", alias = "EnableImages")]
    pub enable_images: Option<bool>,
    #[serde(alias = "Ids")]
    pub ids: Option<String>,
    #[serde(alias = "isFavorite", alias = "IsFavorite")]
    pub is_favorite: Option<bool>,
    #[serde(alias = "personIds", alias = "PersonIds")]
    pub person_ids: Option<String>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct SeasonsQuery {
    #[serde(alias = "userId", alias = "UserId")]
    pub user_id: Option<String>,
    #[serde(alias = "Fields")]
    pub fields: Option<String>,
    #[serde(alias = "enableImages", alias = "EnableImages")]
    pub enable_images: Option<bool>,
    #[serde(alias = "enableUserData", alias = "EnableUserData")]
    pub enable_user_data: Option<bool>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct EpisodesQuery {
    #[serde(alias = "seasonId")]
    pub season_id: Option<String>,
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    pub fields: Option<String>,
    pub season: Option<i32>,
    #[serde(alias = "startIndex")]
    pub start_index: Option<i64>,
    pub limit: Option<i64>,
    #[serde(alias = "enableImages")]
    pub enable_images: Option<bool>,
    #[serde(alias = "enableUserData")]
    pub enable_user_data: Option<bool>,
}

// ── UserViews ─────────────────────────────────────────────────────────────────

/// `GET /jellyfin/UserViews` — list media libraries as Jellyfin collection folders.
pub async fn get_user_views<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    let db = state.db();
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r"
        SELECT a.id, a.name, a.type,
               CASE a.type
                   WHEN 'movie' THEN (SELECT COUNT(*) FROM video_items WHERE video_id = a.id)
                   WHEN 'tv'    THEN (SELECT COUNT(*) FROM tv_shows WHERE video_id = a.id)
                   ELSE 0
               END AS child_count
        FROM videos a
        WHERE a.type IN ('movie', 'tv')
        ORDER BY sort_order, created_at
        ",
        [],
    );
    match db.query_all_raw(stmt).await {
        Ok(rows) => {
            let mut items: Vec<BaseItemDto> = rows
                .iter()
                .filter_map(|r| {
                    let id: Uuid = r.try_get("", "id").ok()?;
                    let name: String = r.try_get("", "name").ok()?;
                    let app_type: String = r.try_get("", "type").ok()?;
                    let child_count: i64 = r.try_get("", "child_count").unwrap_or(0);
                    let collection_type = match app_type.as_str() {
                        "movie" => "movies",
                        "tv" => "tvshows",
                        _ => return None,
                    };
                    let id_str = id.to_string();
                    Some(BaseItemDto {
                        name,
                        server_id: state.server_id().to_string(),
                        id: id_str.clone(),
                        item_type: "CollectionFolder".to_string(),
                        collection_type: Some(collection_type.to_string()),
                        is_folder: true,
                        child_count: Some(child_count as i32),
                        enable_media_source_display: Some(true),
                        play_access: "Full".to_string(),
                        location_type: "FileSystem".to_string(),
                        media_type: "Unknown".to_string(),
                        user_data: Some(UserItemDataDto {
                            key: format_jellyfin_key(&id_str),
                            item_id: id_str.clone(),
                            ..Default::default()
                        }),
                        display_preferences_id: Some(id_str),
                        ..Default::default()
                    })
                })
                .collect();
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields user_views: {e}");
            }
            let total = items.len() as i64;
            Json(QueryResult {
                items: items.into_iter().map(to_json_value).collect(),
                total_record_count: total,
                start_index: 0,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_user_views: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── Items (browse) ────────────────────────────────────────────────────────────

/// `GET /jellyfin/Items` — unified browse/search endpoint.
#[allow(clippy::too_many_lines)]
pub async fn get_items<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Query(q): Query<ItemsQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    let user_id = q
        .user_id
        .as_deref()
        .and_then(|s| s.parse::<Uuid>().ok())
        .unwrap_or(user.user_id);

    // If specific IDs requested
    if let Some(ref ids_str) = q.ids {
        let ids: Vec<&str> = ids_str.split(',').map(str::trim).collect();
        return match fetch_items_by_ids(db, &ids, user_id, server_id).await {
            Ok(mut items) => {
                if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                    tracing::error!("enrich_with_shape_fields ids: {e}");
                }
                if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                    tracing::error!("enrich_with_media_sources ids: {e}");
                }
                let total = items.len() as i64;
                Json(media_list_query_result(items, total, 0)).into_response()
            }
            Err(e) => {
                tracing::error!("jellyfin get_items by ids: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }

    // Determine what to query based on parentId and IncludeItemTypes
    let parent_id = q.parent_id.as_deref().and_then(|s| s.parse::<Uuid>().ok());
    let include_types: Vec<&str> = q
        .include_item_types
        .as_deref()
        .map(|s| s.split(',').map(str::trim).collect())
        .unwrap_or_default();
    let search_term = q.search_term.as_deref().unwrap_or("");
    let is_resumable = q.filters.as_deref().is_some_and(|f| f.contains("IsResumable"));
    let start = q.start_index.unwrap_or(0);
    let limit = q.limit.unwrap_or(50).min(500);

    let sort_field = q.sort_by.as_deref().unwrap_or("SortName");
    let sort_dir = if q
        .sort_order
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("Descending"))
    {
        "DESC"
    } else {
        "ASC"
    };

    // person-based filter: personIds=uuid queries across both movies and TV shows
    if let Some(ref pid_str) = q.person_ids
        && let Some(pid) = pid_str.split(',').next().and_then(|s| s.trim().parse::<Uuid>().ok())
    {
        let fields = q.fields.as_deref();
        return match fetch_items_by_person(db, pid, user_id, server_id, sort_field, sort_dir, start, limit).await {
            Ok((mut items, total)) => {
                if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                    tracing::error!("enrich_with_shape_fields personIds: {e}");
                }
                if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                    tracing::error!("enrich_with_media_sources personIds: {e}");
                }
                if has_field(fields, "Genres")
                    && let Err(e) = enrich_with_genres(db, &mut items).await
                {
                    tracing::error!("enrich_with_genres personIds: {e}");
                }
                Json(media_list_query_result(items, total, start)).into_response()
            }
            Err(e) => {
                tracing::error!("fetch_items_by_person: {e}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }

    // Determine parent type
    let parent_type = if let Some(pid) = parent_id {
        determine_parent_type(db, pid).await
    } else {
        None
    };

    let fp = FetchParams {
        video_id: parent_id,
        search_term,
        is_resumable,
        user_id,
        server_id,
        sort_field,
        sort_dir,
        start,
        limit,
    };

    let result = match parent_type.as_deref() {
        Some("movie") => {
            // Movie library: only Movies match; Seasons/Episodes don't exist here
            if include_types.is_empty() || include_types.iter().any(|t| t.eq_ignore_ascii_case("Movie")) {
                fetch_video_items(db, &fp).await
            } else {
                Ok((vec![], 0))
            }
        }
        Some("tv") => {
            // TV library: route by requested item type
            if include_types.iter().any(|t| t.eq_ignore_ascii_case("Season")) {
                fetch_all_seasons_in_library(db, parent_id.unwrap(), user_id, server_id, start, limit).await
            } else if include_types.iter().any(|t| t.eq_ignore_ascii_case("Episode")) {
                fetch_all_episodes_in_library(db, parent_id.unwrap(), user_id, server_id, start, limit).await
            } else {
                // Series or default
                fetch_tv_shows(db, &fp).await
            }
        }
        Some("series") => fetch_seasons_for_series(db, parent_id.unwrap(), user_id, server_id).await,
        Some("season") => fetch_episodes_for_season(db, parent_id.unwrap(), user_id, server_id).await,
        _ => {
            // No parent or unknown — use IncludeItemTypes
            let fp_no_parent = FetchParams { video_id: None, ..fp };
            if include_types.iter().any(|t| t.eq_ignore_ascii_case("Movie")) {
                fetch_video_items(db, &fp_no_parent).await
            } else if include_types.iter().any(|t| t.eq_ignore_ascii_case("Series")) {
                fetch_tv_shows(db, &fp_no_parent).await
            } else if include_types.iter().any(|t| t.eq_ignore_ascii_case("Episode")) {
                fetch_all_episodes(db, search_term, is_resumable, user_id, server_id, start, limit).await
            } else if is_resumable {
                fetch_resumable(db, user_id, server_id, start, limit).await
            } else {
                // Default: return movies + tv shows combined
                fetch_all_media(db, search_term, user_id, server_id, sort_field, sort_dir, start, limit).await
            }
        }
    };

    match result {
        Ok((mut items, total)) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources: {e}");
            }
            let fields = q.fields.as_deref();
            if has_field(fields, "Genres")
                && let Err(e) = enrich_with_genres(db, &mut items).await
            {
                tracing::error!("enrich_with_genres: {e}");
            }
            Json(media_list_query_result(items, total, start)).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_items: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Items/{itemId}` — single item detail.
pub async fn get_item<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(item_id): Path<String>,
    Query(q): Query<ItemsQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    match fetch_items_by_ids(db, &[item_id.as_str()], user.user_id, server_id).await {
        Ok(mut items) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields item: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources: {e}");
            }
            let fields = q.fields.as_deref();
            if has_field(fields, "Genres")
                && let Err(e) = enrich_with_genres(db, &mut items).await
            {
                tracing::error!("enrich_with_genres: {e}");
            }
            if let Err(e) = enrich_with_people(db, &mut items).await {
                tracing::error!("enrich_with_people: {e}");
            }
            if let Some(item) = items.into_iter().next() {
                Json(item).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(e) => {
            tracing::error!("jellyfin get_item: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Shows/{seriesId}/Seasons`
pub async fn get_seasons<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(series_id): Path<Uuid>,
    Query(_q): Query<SeasonsQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    match fetch_seasons_for_series(db, series_id, user.user_id, server_id).await {
        Ok((mut items, total)) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields seasons: {e}");
            }
            Json(media_list_query_result(items, total, 0)).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_seasons: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Shows/{seriesId}/Episodes`
pub async fn get_episodes<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(series_id): Path<Uuid>,
    Query(q): Query<EpisodesQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();

    let season_id = q.season_id.as_deref().and_then(|s| s.parse::<Uuid>().ok());
    let result = if let Some(sid) = season_id {
        fetch_episodes_for_season(db, sid, user.user_id, server_id).await
    } else {
        fetch_episodes_for_series(db, series_id, user.user_id, server_id).await
    };

    match result {
        Ok((mut items, total)) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields episodes: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources episodes: {e}");
            }
            Json(media_list_query_result(items, total, q.start_index.unwrap_or(0))).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_episodes: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── Internal query helpers ────────────────────────────────────────────────────

async fn determine_parent_type(db: &sea_orm::DatabaseConnection, parent_id: Uuid) -> Option<String> {
    // Check videos table first (most common case for parentId in video context)
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT type FROM videos WHERE id = $1",
        [parent_id.into()],
    );
    if let Ok(Some(r)) = db.query_one_raw(stmt).await {
        let t: String = r.try_get("", "type").ok()?;
        return Some(t);
    }
    // Check apps table for non-video types
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT type FROM apps WHERE id = $1",
        [parent_id.into()],
    );
    if let Ok(Some(r)) = db.query_one_raw(stmt).await {
        let t: String = r.try_get("", "type").ok()?;
        return Some(t);
    }
    // Check tv_shows
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id FROM tv_shows WHERE id = $1",
        [parent_id.into()],
    );
    if let Ok(Some(_)) = db.query_one_raw(stmt).await {
        return Some("series".to_string());
    }
    // Check seasons
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id FROM seasons WHERE id = $1",
        [parent_id.into()],
    );
    if let Ok(Some(_)) = db.query_one_raw(stmt).await {
        return Some("season".to_string());
    }
    None
}

fn build_user_data(
    item_id: &str,
    resume_position: i32,
    play_count: i32,
    is_watched: bool,
    is_favorite: bool,
    last_watch_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> UserItemDataDto {
    build_user_data_with_key(
        item_id,
        item_id,
        resume_position,
        play_count,
        is_watched,
        is_favorite,
        last_watch_at,
    )
}

fn build_user_data_with_key(
    item_id: &str,
    provider_key: &str, // Use TMDB ID or other provider key when available; fall back to item_id
    resume_position: i32,
    play_count: i32,
    is_watched: bool,
    is_favorite: bool,
    last_watch_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> UserItemDataDto {
    UserItemDataDto {
        played_percentage: if is_watched { Some(100.0) } else { None },
        playback_position_ticks: seconds_to_ticks(resume_position),
        play_count,
        is_favorite,
        played: is_watched,
        last_played_date: last_watch_at.map(|d| d.to_rfc3339()),
        unplayed_item_count: None,
        key: provider_key.to_string(),
        item_id: item_id.to_string(),
    }
}

/// Format a UUID string into Jellyfin's hyphenated key format (8-4-4-4-12).
fn format_jellyfin_key(id: &str) -> String {
    let clean: String = id.replace('-', "");
    if clean.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &clean[0..8],
            &clean[8..12],
            &clean[12..16],
            &clean[16..20],
            &clean[20..32]
        )
    } else {
        id.to_string()
    }
}

fn uuids_to_pg_array(ids: &[Uuid]) -> sea_orm::Value {
    let joined = ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",");
    format!("{{{joined}}}").into()
}

fn format_jellyfin_date(date: chrono::NaiveDate) -> String {
    format!("{date}T00:00:00.0000000Z")
}

fn build_jellyfin_etag(id: &str, parts: &[Option<String>]) -> String {
    let mut values = vec![id.to_string()];
    for part in parts {
        if let Some(value) = part.as_ref().filter(|value| !value.is_empty()) {
            values.push(value.clone());
        }
    }
    values.join("|")
}

fn slash_parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let (parent, _) = trimmed.rsplit_once('/')?;
    if parent.is_empty() {
        Some("/".to_string())
    } else {
        Some(parent.to_string())
    }
}

fn maybe_strip_season_directory(path: &str) -> Option<String> {
    let last = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let is_season_folder = last == "specials"
        || last.starts_with("season ")
        || last.starts_with("season-")
        || last.starts_with("season_")
        || (last.starts_with('s') && last[1..].chars().all(|ch| ch.is_ascii_digit()));

    if is_season_folder {
        slash_parent(path).or_else(|| Some(path.to_string()))
    } else {
        Some(path.to_string())
    }
}

fn infer_series_path_from_sample(sample_path: &str) -> Option<String> {
    slash_parent(sample_path).and_then(|parent| maybe_strip_season_directory(&parent))
}

fn to_json_value<T: serde::Serialize>(value: T) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn media_list_item_to_json(item: BaseItemDto) -> JsonValue {
    let item_type = item.item_type.clone();
    let mut value = to_json_value(item);
    let Some(obj) = value.as_object_mut() else {
        return value;
    };

    let remove_common = |obj: &mut serde_json::Map<String, JsonValue>| {
        for key in [
            "CanDelete",
            "CanDownload",
            "ExternalUrls",
            "LockData",
            "LockedFields",
            "PlayAccess",
            "RemoteTrailers",
            "Studios",
            "Taglines",
            "Tags",
        ] {
            obj.remove(key);
        }
    };

    match item_type.as_str() {
        "Movie" => remove_common(obj),
        "Series" => {
            remove_common(obj);
            obj.remove("OriginalTitle");
        }
        "Season" => {
            remove_common(obj);
            obj.remove("Overview");
            obj.remove("ParentBackdropImageTags");
            obj.remove("ParentBackdropItemId");
        }
        "Episode" => {
            remove_common(obj);
            obj.remove("CommunityRating");
            obj.remove("HasSubtitles");
            obj.remove("Overview");
            obj.remove("PremiereDate");
        }
        _ => {}
    }

    value
}

fn media_list_query_result(
    items: Vec<BaseItemDto>,
    total_record_count: i64,
    start_index: i64,
) -> QueryResult<JsonValue> {
    QueryResult {
        items: items.into_iter().map(media_list_item_to_json).collect(),
        total_record_count,
        start_index,
    }
}

fn has_field(fields: Option<&str>, name: &str) -> bool {
    fields.is_some_and(|f| f.split(',').any(|s| s.trim().eq_ignore_ascii_case(name)))
}

#[allow(clippy::too_many_lines)]
async fn enrich_with_shape_fields(
    db: &sea_orm::DatabaseConnection,
    items: &mut [BaseItemDto],
) -> Result<(), sea_orm::DbErr> {
    if items.is_empty() {
        return Ok(());
    }

    let mut positions: HashMap<Uuid, Vec<usize>> = HashMap::new();
    let mut collection_ids = Vec::new();
    let mut video_item_ids = Vec::new();
    let mut series_ids = Vec::new();
    let mut season_ids = Vec::new();
    let mut episode_ids = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let Ok(id) = item.id.parse::<Uuid>() else {
            continue;
        };
        positions.entry(id).or_default().push(idx);
        match item.item_type.as_str() {
            "CollectionFolder" => collection_ids.push(id),
            "Movie" => video_item_ids.push(id),
            "Series" => series_ids.push(id),
            "Season" => season_ids.push(id),
            "Episode" => episode_ids.push(id),
            _ => {}
        }
    }

    if !collection_ids.is_empty() {
        let sql = r"
            SELECT a.id, a.poster_path, a.created_at, a.updated_at,
                   a.sources->0->>'root_path' AS root_path,
                   CASE
                       WHEN a.type = 'movie' THEN (
                           SELECT MAX(m.created_at) FROM video_items m WHERE m.video_id = a.id
                       )
                       WHEN a.type = 'tv' THEN (
                           SELECT MAX(t.created_at) FROM tv_shows t WHERE t.video_id = a.id
                       )
                       ELSE NULL
                   END AS date_last_media_added
            FROM videos a
            WHERE a.id = ANY($1::uuid[])
        ";

        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uuids_to_pg_array(&collection_ids)],
            ))
            .await?;

        for row in &rows {
            let id: Uuid = row.try_get("", "id").unwrap();
            let poster_path: Option<String> = row.try_get("", "poster_path").ok().flatten();
            let created_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "created_at").ok().flatten();
            let updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "updated_at").ok().flatten();
            let root_path: Option<String> = row.try_get("", "root_path").ok().flatten();
            let date_last_media_added: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "date_last_media_added").ok().flatten();

            let Some(indices) = positions.get(&id).cloned() else {
                continue;
            };

            for idx in indices {
                let item = &mut items[idx];
                item.date_created = created_at.map(|date| date.to_rfc3339());
                item.date_last_media_added = date_last_media_added.or(created_at).map(|date| date.to_rfc3339());
                item.etag = Some(build_jellyfin_etag(
                    &item.id,
                    &[
                        updated_at.map(|date| date.to_rfc3339()),
                        root_path.clone(),
                        item.collection_type.clone(),
                    ],
                ));
                item.parent_id.get_or_insert_with(|| "root".to_string());
                if item.path.is_none() {
                    item.path.clone_from(&root_path);
                }
                item.people.get_or_insert_with(Vec::new);
                if item.sort_name.is_none() {
                    item.sort_name = Some(item.name.to_lowercase());
                }
                item.special_feature_count = Some(0);
                if poster_path.is_some() {
                    item.primary_image_aspect_ratio = Some(2.0 / 3.0);
                    item.image_tags
                        .entry("Primary".to_string())
                        .or_insert_with(|| item.id.clone());
                }
            }
        }
    }

    if !video_item_ids.is_empty() {
        let sql = r"
            SELECT m.id, m.douban_rating, m.updated_at,
                   vf.path AS video_path,
                   vf.checksum AS video_checksum,
                   vf.created_at AS video_created_at,
                   vf.updated_at AS video_updated_at
            FROM video_items m
            LEFT JOIN LATERAL (
                SELECT vf.path, vf.checksum, vf.created_at, vf.updated_at
                FROM video_files vf
                WHERE vf.video_item_id = m.id
                  AND vf.is_available = true
                ORDER BY vf.size DESC NULLS LAST, vf.created_at DESC NULLS LAST
                LIMIT 1
            ) vf ON true
            WHERE m.id = ANY($1::uuid[])
        ";

        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uuids_to_pg_array(&video_item_ids)],
            ))
            .await?;

        for row in &rows {
            let id: Uuid = row.try_get("", "id").unwrap();
            let douban_rating: Option<f64> = row.try_get("", "douban_rating").ok().flatten();
            let updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "updated_at").ok().flatten();
            let video_path: Option<String> = row.try_get("", "video_path").ok().flatten();
            let video_checksum: Option<String> = row.try_get("", "video_checksum").ok().flatten();
            let video_created_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "video_created_at").ok().flatten();
            let video_updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "video_updated_at").ok().flatten();

            let Some(indices) = positions.get(&id).cloned() else {
                continue;
            };

            for idx in indices {
                let item = &mut items[idx];
                item.critic_rating = item.critic_rating.or(douban_rating.map(|rating| rating * 10.0));
                if item.path.is_none() {
                    item.path.clone_from(&video_path);
                }
                if item.etag.is_none() {
                    item.etag = Some(video_checksum.clone().unwrap_or_else(|| {
                        build_jellyfin_etag(
                            &item.id,
                            &[
                                updated_at.map(|date| date.to_rfc3339()),
                                video_updated_at.map(|date| date.to_rfc3339()),
                                video_path.clone(),
                            ],
                        )
                    }));
                }
                if item.date_created.is_none() {
                    item.date_created = video_created_at.or(video_updated_at).map(|date| date.to_rfc3339());
                }
                item.special_feature_count = Some(0);
            }
        }
    }

    if !series_ids.is_empty() {
        let sql = r"
            SELECT t.id, t.last_air_date, t.updated_at,
                   COALESCE((
                       SELECT SUM(COALESCE(e.runtime, 0))
                       FROM episodes e
                       WHERE e.tv_show_id = t.id
                   ), 0) AS total_runtime,
                   vf.path AS sample_path,
                   vf.checksum AS sample_checksum,
                   vf.created_at AS sample_created_at,
                   vf.updated_at AS sample_updated_at
            FROM tv_shows t
            LEFT JOIN LATERAL (
                SELECT vf.path, vf.checksum, vf.created_at, vf.updated_at
                FROM episodes e
                JOIN video_files vf ON vf.episode_id = e.id
                WHERE e.tv_show_id = t.id
                  AND vf.is_available = true
                ORDER BY vf.size DESC NULLS LAST, vf.created_at DESC NULLS LAST
                LIMIT 1
            ) vf ON true
            WHERE t.id = ANY($1::uuid[])
        ";

        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uuids_to_pg_array(&series_ids)],
            ))
            .await?;

        for row in &rows {
            let id: Uuid = row.try_get("", "id").unwrap();
            let last_air_date: Option<chrono::NaiveDate> = row.try_get("", "last_air_date").ok().flatten();
            let updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "updated_at").ok().flatten();
            let total_runtime: i64 = row.try_get("", "total_runtime").unwrap_or(0);
            let sample_path: Option<String> = row.try_get("", "sample_path").ok().flatten();
            let sample_checksum: Option<String> = row.try_get("", "sample_checksum").ok().flatten();
            let sample_updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "sample_updated_at").ok().flatten();

            let Some(indices) = positions.get(&id).cloned() else {
                continue;
            };

            for idx in indices {
                let item = &mut items[idx];
                if item.end_date.is_none() {
                    item.end_date = last_air_date.map(format_jellyfin_date);
                }
                if item.run_time_ticks.is_none() && total_runtime > 0 {
                    #[allow(clippy::cast_possible_truncation)]
                    let seconds = total_runtime as i32;
                    item.run_time_ticks = Some(seconds_to_ticks(seconds));
                }
                if item.path.is_none() {
                    item.path = sample_path.as_deref().and_then(infer_series_path_from_sample);
                }
                if item.etag.is_none() {
                    item.etag = Some(sample_checksum.clone().unwrap_or_else(|| {
                        build_jellyfin_etag(
                            &item.id,
                            &[
                                updated_at.map(|date| date.to_rfc3339()),
                                sample_updated_at.map(|date| date.to_rfc3339()),
                                sample_path.clone(),
                            ],
                        )
                    }));
                }
                item.special_feature_count = Some(0);
            }
        }
    }

    if !season_ids.is_empty() {
        let sql = r"
            SELECT s.id, s.tv_show_id, s.air_date,
                   t.year AS show_year,
                   t.poster_path AS series_poster_path,
                   vf.path AS sample_path,
                   vf.checksum AS sample_checksum,
                   vf.created_at AS sample_created_at,
                   vf.updated_at AS sample_updated_at
            FROM seasons s
            JOIN tv_shows t ON t.id = s.tv_show_id
            LEFT JOIN LATERAL (
                SELECT vf.path, vf.checksum, vf.created_at, vf.updated_at
                FROM episodes e
                JOIN video_files vf ON vf.episode_id = e.id
                WHERE e.season_id = s.id
                  AND vf.is_available = true
                ORDER BY vf.size DESC NULLS LAST, vf.created_at DESC NULLS LAST
                LIMIT 1
            ) vf ON true
            WHERE s.id = ANY($1::uuid[])
        ";

        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uuids_to_pg_array(&season_ids)],
            ))
            .await?;

        for row in &rows {
            let id: Uuid = row.try_get("", "id").unwrap();
            let tv_show_id: Uuid = row.try_get("", "tv_show_id").unwrap();
            let air_date: Option<chrono::NaiveDate> = row.try_get("", "air_date").ok().flatten();
            let show_year: Option<i32> = row.try_get("", "show_year").ok().flatten();
            let series_poster_path: Option<String> = row.try_get("", "series_poster_path").ok().flatten();
            let sample_path: Option<String> = row.try_get("", "sample_path").ok().flatten();
            let sample_checksum: Option<String> = row.try_get("", "sample_checksum").ok().flatten();
            let sample_created_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "sample_created_at").ok().flatten();
            let sample_updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "sample_updated_at").ok().flatten();

            let Some(indices) = positions.get(&id).cloned() else {
                continue;
            };

            for idx in indices {
                let item = &mut items[idx];
                if item.date_created.is_none() {
                    item.date_created = sample_created_at
                        .map(|date| date.to_rfc3339())
                        .or_else(|| air_date.map(format_jellyfin_date));
                }
                if item.path.is_none() {
                    item.path = sample_path.as_deref().and_then(slash_parent);
                }
                if item.etag.is_none() {
                    item.etag = Some(sample_checksum.clone().unwrap_or_else(|| {
                        build_jellyfin_etag(
                            &item.id,
                            &[sample_updated_at.map(|date| date.to_rfc3339()), sample_path.clone()],
                        )
                    }));
                }
                if item.parent_thumb_item_id.is_none() {
                    item.parent_thumb_item_id = Some(tv_show_id.to_string());
                }
                if item.parent_thumb_image_tag.is_none() && series_poster_path.is_some() {
                    item.parent_thumb_image_tag = Some(tv_show_id.to_string());
                }
                if item.production_year.is_none() {
                    item.production_year = air_date.map(|date| date.year()).or(show_year);
                }
                if item.recursive_item_count.is_none() {
                    item.recursive_item_count = item.child_count;
                }
                if item.sort_name.is_none() {
                    item.sort_name = Some(item.name.to_lowercase());
                }
                item.special_feature_count = Some(0);
            }
        }
    }

    if !episode_ids.is_empty() {
        let sql = r"
            SELECT e.id, e.tv_show_id, e.air_date,
                   t.poster_path AS series_poster_path,
                   t.backdrop_path AS series_backdrop_path,
                   EXISTS (
                       SELECT 1
                       FROM media_arts ma
                       WHERE ma.tv_show_id = t.id
                         AND ma.is_selected = true
                         AND ma.art_type = 'clearlogo'
                   ) AS has_series_logo,
                   vf.path AS sample_path,
                   vf.checksum AS sample_checksum,
                   vf.created_at AS sample_created_at,
                   vf.updated_at AS sample_updated_at
            FROM episodes e
            JOIN tv_shows t ON t.id = e.tv_show_id
            LEFT JOIN LATERAL (
                SELECT vf.path, vf.checksum, vf.created_at, vf.updated_at
                FROM video_files vf
                WHERE vf.episode_id = e.id
                  AND vf.is_available = true
                ORDER BY vf.size DESC NULLS LAST, vf.created_at DESC NULLS LAST
                LIMIT 1
            ) vf ON true
            WHERE e.id = ANY($1::uuid[])
        ";

        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uuids_to_pg_array(&episode_ids)],
            ))
            .await?;

        for row in &rows {
            let id: Uuid = row.try_get("", "id").unwrap();
            let tv_show_id: Uuid = row.try_get("", "tv_show_id").unwrap();
            let air_date: Option<chrono::NaiveDate> = row.try_get("", "air_date").ok().flatten();
            let series_poster_path: Option<String> = row.try_get("", "series_poster_path").ok().flatten();
            let series_backdrop_path: Option<String> = row.try_get("", "series_backdrop_path").ok().flatten();
            let has_series_logo: bool = row.try_get("", "has_series_logo").unwrap_or(false);
            let sample_path: Option<String> = row.try_get("", "sample_path").ok().flatten();
            let sample_checksum: Option<String> = row.try_get("", "sample_checksum").ok().flatten();
            let sample_created_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "sample_created_at").ok().flatten();
            let sample_updated_at: Option<chrono::DateTime<chrono::FixedOffset>> =
                row.try_get("", "sample_updated_at").ok().flatten();

            let Some(indices) = positions.get(&id).cloned() else {
                continue;
            };

            for idx in indices {
                let item = &mut items[idx];
                let show_id = tv_show_id.to_string();
                if item.date_created.is_none() {
                    item.date_created = sample_created_at
                        .map(|date| date.to_rfc3339())
                        .or_else(|| air_date.map(format_jellyfin_date));
                }
                if item.path.is_none() {
                    item.path.clone_from(&sample_path);
                }
                if item.etag.is_none() {
                    item.etag = Some(sample_checksum.clone().unwrap_or_else(|| {
                        build_jellyfin_etag(
                            &item.id,
                            &[sample_updated_at.map(|date| date.to_rfc3339()), sample_path.clone()],
                        )
                    }));
                }
                if item.parent_backdrop_item_id.is_none() {
                    item.parent_backdrop_item_id = Some(show_id.clone());
                }
                if item.parent_backdrop_image_tags.is_none() && series_backdrop_path.is_some() {
                    item.parent_backdrop_image_tags = Some(vec![show_id.clone()]);
                }
                if item.parent_thumb_item_id.is_none() {
                    item.parent_thumb_item_id = Some(show_id.clone());
                }
                if item.parent_thumb_image_tag.is_none() && series_poster_path.is_some() {
                    item.parent_thumb_image_tag = Some(show_id.clone());
                }
                if item.series_primary_image_tag.is_none() && series_poster_path.is_some() {
                    item.series_primary_image_tag = Some(show_id.clone());
                }
                if item.parent_logo_item_id.is_none() && has_series_logo {
                    item.parent_logo_item_id = Some(show_id.clone());
                }
                if item.parent_logo_image_tag.is_none() && has_series_logo {
                    item.parent_logo_image_tag = Some(show_id);
                }
                if item.sort_name.is_none() {
                    item.sort_name = Some(item.name.to_lowercase());
                }
                item.special_feature_count = Some(0);
            }
        }
    }

    Ok(())
}

fn season_row_to_dto(r: &sea_orm::QueryResult, server_id: &str) -> BaseItemDto {
    let id: Uuid = r.try_get("", "id").unwrap();
    let id_str = id.to_string();
    let tv_show_id: Uuid = r.try_get("", "tv_show_id").unwrap();
    let season_number: i32 = r.try_get("", "season_number").unwrap_or(0);
    let title: Option<String> = r.try_get("", "title").ok().flatten();
    let overview: Option<String> = r.try_get("", "overview").ok().flatten();
    let poster_path: Option<String> = r.try_get("", "poster_path").ok().flatten();
    let episode_count: Option<i32> = r.try_get("", "episode_count").ok().flatten();
    let series_name: Option<String> = r.try_get("", "series_name").ok();
    let air_date: Option<chrono::NaiveDate> = r.try_get("", "air_date").ok().flatten();
    let series_poster: Option<String> = r.try_get("", "series_poster_path").ok().flatten();
    let series_backdrop: Option<String> = r.try_get("", "series_backdrop_path").ok().flatten();

    let name = title.unwrap_or_else(|| {
        if season_number == 0 {
            "Specials".to_string()
        } else {
            format!("Season {season_number}")
        }
    });
    let mut image_tags = HashMap::new();
    if poster_path.is_some() {
        image_tags.insert("Primary".to_string(), id_str.clone());
    }
    let tv_id = tv_show_id.to_string();

    BaseItemDto {
        name,
        server_id: server_id.to_string(),
        id: id_str.clone(),
        item_type: "Season".to_string(),
        index_number: Some(season_number),
        parent_id: Some(tv_id.clone()),
        series_id: Some(tv_id.clone()),
        series_name,
        overview,
        premiere_date: air_date.map(|d| format!("{d}T00:00:00.0000000Z")),
        is_folder: true,
        child_count: episode_count,
        image_tags,
        location_type: "FileSystem".to_string(),
        media_type: "Unknown".to_string(),
        play_access: "Full".to_string(),
        genres: vec![],
        genre_items: vec![],
        user_data: Some(UserItemDataDto {
            key: format_jellyfin_key(&id_str),
            item_id: id_str,
            ..Default::default()
        }),
        parent_backdrop_item_id: Some(tv_id.clone()),
        parent_backdrop_image_tags: if series_backdrop.is_some() {
            Some(vec![tv_id.clone()])
        } else {
            None
        },
        series_primary_image_tag: if series_poster.is_some() { Some(tv_id) } else { None },
        ..Default::default()
    }
}

fn parse_frame_rate(s: &str) -> Option<f64> {
    let mut parts = s.splitn(2, '/');
    let num: f64 = parts.next()?.parse().ok()?;
    let den: f64 = parts.next()?.parse().ok()?;
    if den == 0.0 { None } else { Some(num / den) }
}

/// Batch-populate `people` for single-item detail responses (never called for list endpoints).
///
/// - Movie items: joins `video_cast` + `video_persons`
/// - Series items: joins `tv_season_cast` + `tv_persons` (deduped by person across all seasons)
/// - Season/Episode items: joins `tv_season_cast` + `tv_persons` via the item's tv_show_id
async fn enrich_with_people(db: &sea_orm::DatabaseConnection, items: &mut [BaseItemDto]) -> Result<(), sea_orm::DbErr> {
    use super::models::PersonDto;
    for item in items.iter_mut() {
        let Ok(id) = item.id.parse::<Uuid>() else {
            continue;
        };
        let mut people: Vec<PersonDto> = Vec::new();

        match item.item_type.as_str() {
            "Movie" => {
                let sql = format!(
                    r"SELECT mp.id, mp.name, mp.profile_path, mc.role, mc.character
                      FROM video_cast mc
                      JOIN video_persons mp ON mp.id = mc.video_person_id
                      WHERE mc.video_item_id = '{id}'
                      ORDER BY mc.sort_order ASC
                      LIMIT 50"
                );
                let rows = db
                    .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, []))
                    .await?;
                for r in &rows {
                    let person_id: Uuid = r.try_get("", "id").unwrap_or_default();
                    let name: String = r.try_get("", "name").unwrap_or_default();
                    let profile_path: Option<String> = r.try_get("", "profile_path").ok().flatten();
                    let role: String = r.try_get("", "role").unwrap_or_default();
                    let character: Option<String> = r.try_get("", "character").ok().flatten();
                    let person_type = capitalize_role(&role);
                    let has_image = profile_path.as_deref().is_some_and(|p| !p.is_empty());
                    people.push(PersonDto {
                        name,
                        id: person_id.to_string(),
                        role: if person_type == "Actor" { character } else { None },
                        person_type,
                        primary_image_tag: if has_image { Some(person_id.to_string()) } else { None },
                        image_blur_hashes: HashMap::default(),
                    });
                }
            }
            "Series" => {
                let sql = format!(
                    r"SELECT DISTINCT ON (tp.id) tp.id, tp.name, tp.profile_path, tsc.role, tsc.character, tsc.sort_order
                      FROM tv_season_cast tsc
                      JOIN tv_persons tp ON tp.id = tsc.tv_person_id
                      WHERE tsc.tv_show_id = '{id}'
                      ORDER BY tp.id, tsc.sort_order ASC
                      LIMIT 50"
                );
                let rows = db
                    .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, []))
                    .await?;
                // Re-sort by sort_order after deduplication
                let mut cast: Vec<(i32, PersonDto)> = rows
                    .iter()
                    .filter_map(|r| {
                        let person_id: Uuid = r.try_get("", "id").ok()?;
                        let name: String = r.try_get("", "name").ok()?;
                        let profile_path: Option<String> = r.try_get("", "profile_path").ok().flatten();
                        let role: String = r.try_get("", "role").unwrap_or_default();
                        let character: Option<String> = r.try_get("", "character").ok().flatten();
                        let sort_order: i32 = r.try_get("", "sort_order").unwrap_or(0);
                        let person_type = capitalize_role(&role);
                        let has_image = profile_path.as_deref().is_some_and(|p| !p.is_empty());
                        Some((
                            sort_order,
                            PersonDto {
                                name,
                                id: person_id.to_string(),
                                role: if person_type == "Actor" { character } else { None },
                                person_type,
                                primary_image_tag: if has_image { Some(person_id.to_string()) } else { None },
                                image_blur_hashes: HashMap::default(),
                            },
                        ))
                    })
                    .collect();
                cast.sort_by_key(|(order, _)| *order);
                people = cast.into_iter().map(|(_, p)| p).collect();
            }
            "Season" | "Episode" => {
                // Look up tv_show_id via the item's series_id field
                let show_id_str = item.series_id.as_deref().unwrap_or("");
                let Ok(show_id) = show_id_str.parse::<Uuid>() else {
                    continue;
                };
                let sql = format!(
                    r"SELECT DISTINCT ON (tp.id) tp.id, tp.name, tp.profile_path, tsc.role, tsc.character, tsc.sort_order
                      FROM tv_season_cast tsc
                      JOIN tv_persons tp ON tp.id = tsc.tv_person_id
                      WHERE tsc.tv_show_id = '{show_id}'
                      ORDER BY tp.id, tsc.sort_order ASC
                      LIMIT 50"
                );
                let rows = db
                    .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, []))
                    .await?;
                let mut cast: Vec<(i32, PersonDto)> = rows
                    .iter()
                    .filter_map(|r| {
                        let person_id: Uuid = r.try_get("", "id").ok()?;
                        let name: String = r.try_get("", "name").ok()?;
                        let profile_path: Option<String> = r.try_get("", "profile_path").ok().flatten();
                        let role: String = r.try_get("", "role").unwrap_or_default();
                        let character: Option<String> = r.try_get("", "character").ok().flatten();
                        let sort_order: i32 = r.try_get("", "sort_order").unwrap_or(0);
                        let person_type = capitalize_role(&role);
                        let has_image = profile_path.as_deref().is_some_and(|p| !p.is_empty());
                        Some((
                            sort_order,
                            PersonDto {
                                name,
                                id: person_id.to_string(),
                                role: if person_type == "Actor" { character } else { None },
                                person_type,
                                primary_image_tag: if has_image { Some(person_id.to_string()) } else { None },
                                image_blur_hashes: HashMap::default(),
                            },
                        ))
                    })
                    .collect();
                cast.sort_by_key(|(order, _)| *order);
                people = cast.into_iter().map(|(_, p)| p).collect();
            }
            _ => {}
        }

        item.people = Some(people);
    }
    Ok(())
}

fn capitalize_role(role: &str) -> String {
    match role.to_lowercase().as_str() {
        "actor" => "Actor".to_string(),
        "director" => "Director".to_string(),
        "writer" => "Writer".to_string(),
        "producer" => "Producer".to_string(),
        "composer" | "music" => "Composer".to_string(),
        "gueststar" | "guest_star" => "GuestStar".to_string(),
        _ => {
            let mut s = role.to_string();
            if let Some(first) = s.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            s
        }
    }
}

fn build_video_stream_from_json(
    codec: &str,
    width: Option<i32>,
    height: Option<i32>,
    profile: Option<String>,
    video_streams_json: &Option<serde_json::Value>,
) -> super::models::MediaStream {
    use super::models::MediaStream;
    let v = video_streams_json
        .as_ref()
        .and_then(|j| if j.is_object() { Some(j) } else { None });
    let bit_rate = v.and_then(|j| {
        j.get("tags")
            .and_then(|t| t.get("BPS"))
            .and_then(|b| b.as_str())
            .and_then(|s| s.parse::<i64>().ok())
    });
    let level = v
        .and_then(|j| j.get("level").and_then(serde_json::Value::as_i64))
        .unwrap_or(0) as i32;
    let pixel_format = v.and_then(|j| j.get("pix_fmt").and_then(|p| p.as_str()).map(String::from));
    let color_space = v.and_then(|j| j.get("color_space").and_then(|c| c.as_str()).map(String::from));
    let color_transfer = v.and_then(|j| j.get("color_transfer").and_then(|c| c.as_str()).map(String::from));
    let color_primaries = v.and_then(|j| j.get("color_primaries").and_then(|c| c.as_str()).map(String::from));
    let aspect_ratio = v.and_then(|j| j.get("display_aspect_ratio").and_then(|a| a.as_str()).map(String::from));
    let avg_fps = v
        .and_then(|j| {
            j.get("avg_frame_rate")
                .and_then(|f| f.as_str())
                .and_then(parse_frame_rate)
        })
        .map(|f| f as f32);
    let real_fps = v
        .and_then(|j| {
            j.get("r_frame_rate")
                .and_then(|f| f.as_str())
                .and_then(parse_frame_rate)
        })
        .map(|f| f as f32);
    let time_base = v.and_then(|j| j.get("time_base").and_then(|t| t.as_str()).map(String::from));
    let is_hdr = color_transfer
        .as_deref()
        .is_some_and(|ct| ct.contains("smpte") || ct == "arib-std-b67");
    let (video_range, video_range_type) = if is_hdr {
        (Some("HDR".to_string()), Some("HDR10".to_string()))
    } else {
        (Some("SDR".to_string()), Some("SDR".to_string()))
    };
    let h = height.unwrap_or(0);
    let w = width.unwrap_or(0);
    let display_title = if h >= 2160 {
        format!(
            "{} 4K {}",
            codec.to_uppercase(),
            video_range_type.as_deref().unwrap_or("SDR")
        )
    } else if w > 0 {
        format!("{} {}x{}", codec.to_uppercase(), w, h)
    } else {
        codec.to_uppercase()
    };
    MediaStream {
        codec: codec.to_string(),
        stream_type: "Video".to_string(),
        index: 0,
        is_default: true,
        is_forced: false,
        is_external: false,
        display_title: Some(display_title),
        width,
        height,
        bit_rate,
        profile,
        level,
        pixel_format,
        color_space,
        color_transfer,
        color_primaries,
        aspect_ratio,
        average_frame_rate: avg_fps,
        real_frame_rate: real_fps,
        reference_frame_rate: avg_fps.or(real_fps),
        time_base,
        video_range,
        video_range_type,
        is_interlaced: false,
        is_avc: codec == "h264",
        is_text_subtitle_stream: false,
        supports_external_stream: false,
        audio_spatial_format: "None".to_string(),
        is_hearing_impaired: false,
        is_anamorphic: Some(false),
        ..Default::default()
    }
}

fn build_audio_streams_from_json(
    audio_streams_json: &Option<serde_json::Value>,
    start_idx: i32,
) -> (Vec<super::models::MediaStream>, Option<i32>) {
    use super::models::MediaStream;
    let mut streams = Vec::new();
    let mut default_audio_idx: Option<i32> = None;
    let Some(serde_json::Value::Array(audio)) = audio_streams_json else {
        return (streams, default_audio_idx);
    };
    let mut stream_idx = start_idx;
    for a in audio {
        let codec = a.get("codec_name").and_then(|c| c.as_str()).unwrap_or("aac");
        let json_idx = a
            .get("index")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i64::from(stream_idx)) as i32;
        let is_default = a
            .get("disposition")
            .and_then(|d| d.get("default"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
            == 1;
        let is_forced = a
            .get("disposition")
            .and_then(|d| d.get("forced"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
            == 1;
        let language = a
            .get("tags")
            .and_then(|t| t.get("language"))
            .and_then(|l| l.as_str())
            .map(String::from);
        let bit_rate = a
            .get("bit_rate")
            .and_then(|b| b.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| {
                a.get("tags")
                    .and_then(|t| t.get("BPS"))
                    .and_then(|b| b.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
            });
        let channels = a.get("channels").and_then(serde_json::Value::as_i64).map(|c| c as i32);
        let sample_rate = a
            .get("sample_rate")
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse::<i32>().ok())
            .or_else(|| {
                a.get("sample_rate")
                    .and_then(serde_json::Value::as_i64)
                    .map(|s| s as i32)
            });
        let raw_layout = a.get("channel_layout").and_then(|l| l.as_str()).unwrap_or("");
        let channel_layout = if raw_layout.starts_with("7.1") {
            Some("7.1".to_string())
        } else if raw_layout.starts_with("5.1") {
            Some("5.1".to_string())
        } else if raw_layout.starts_with("stereo") {
            Some("stereo".to_string())
        } else if raw_layout.starts_with("mono") {
            Some("mono".to_string())
        } else {
            channels
                .map(|c| {
                    match c {
                        8 => "7.1",
                        6 => "5.1",
                        2 => "stereo",
                        1 => "mono",
                        _ => "",
                    }
                    .to_string()
                })
                .filter(|s| !s.is_empty())
        };
        let display_title = a
            .get("tags")
            .and_then(|t| t.get("title"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .or_else(|| {
                Some(format!(
                    "{} - {}",
                    codec.to_uppercase(),
                    channel_layout.as_deref().unwrap_or("")
                ))
            });
        if is_default && default_audio_idx.is_none() {
            default_audio_idx = Some(json_idx);
        }
        streams.push(MediaStream {
            codec: codec.to_string(),
            stream_type: "Audio".to_string(),
            index: json_idx,
            is_default,
            is_forced,
            is_external: false,
            language,
            display_title,
            bit_rate,
            channels,
            channel_layout,
            sample_rate,
            level: 0,
            is_interlaced: false,
            is_avc: false,
            is_text_subtitle_stream: false,
            supports_external_stream: false,
            audio_spatial_format: "None".to_string(),
            is_hearing_impaired: false,
            ..Default::default()
        });
        stream_idx = json_idx + 1;
    }
    (streams, default_audio_idx)
}

/// Batch-load `MediaSourceInfo` for Movie/Episode items that have video files.
/// Sets `media_sources`, `container`, `has_subtitles`, `video_type` on each matching item.
/// For listings, MediaSources use `Protocol: "File"` (no DirectStreamUrl).
async fn enrich_with_media_sources(
    db: &sea_orm::DatabaseConnection,
    items: &mut [BaseItemDto],
) -> Result<(), sea_orm::DbErr> {
    use super::playback::seconds_to_ticks;

    // Separate UUIDs by type — movie_id and episode_id are different FK columns
    let mut video_item_ids: Vec<Uuid> = Vec::new();
    let mut episode_ids: Vec<Uuid> = Vec::new();
    for item in items.iter() {
        let Ok(uid) = item.id.parse::<Uuid>() else {
            continue;
        };
        match item.item_type.as_str() {
            "Movie" => video_item_ids.push(uid),
            "Episode" => episode_ids.push(uid),
            _ => {}
        }
    }
    if video_item_ids.is_empty() && episode_ids.is_empty() {
        return Ok(());
    }
    tracing::debug!(
        "enrich_with_media_sources: {} movies, {} episodes",
        video_item_ids.len(),
        episode_ids.len()
    );

    // Use = ANY($n::uuid[]) — passes UUIDs as a single PostgreSQL array parameter instead of
    // inlining hundreds of UUID literals into the SQL string.
    fn uuids_to_pg_array(ids: &[Uuid]) -> sea_orm::Value {
        let s = format!(
            "{{{}}}",
            ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
        );
        s.into()
    }

    let (where_clause, params): (&str, Vec<sea_orm::Value>) = match (video_item_ids.is_empty(), episode_ids.is_empty())
    {
        (false, false) => (
            "(vf.video_item_id = ANY($1::uuid[]) OR vf.episode_id = ANY($2::uuid[]))",
            vec![uuids_to_pg_array(&video_item_ids), uuids_to_pg_array(&episode_ids)],
        ),
        (false, true) => (
            "vf.video_item_id = ANY($1::uuid[])",
            vec![uuids_to_pg_array(&video_item_ids)],
        ),
        (true, false) => ("vf.episode_id = ANY($1::uuid[])", vec![uuids_to_pg_array(&episode_ids)]),
        (true, true) => unreachable!(),
    };

    let sql = format!(
        r"SELECT vf.id, vf.video_item_id, vf.episode_id, vf.path, vf.filename, vf.size, vf.duration,
                vf.video_codec, vf.video_width, vf.video_height, vf.video_profile,
                vf.video_streams, vf.audio_streams
         FROM video_files vf
         WHERE vf.is_available = true
           AND {where_clause}
         ORDER BY vf.size DESC"
    );
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, params))
        .await?;

    // Group by item id (first movie_id, then episode_id)
    let mut sources_map: HashMap<Uuid, Vec<MediaSourceInfo>> = HashMap::new();
    for r in &rows {
        let vf_id: Uuid = r.try_get("", "id").unwrap();
        let video_item_id: Option<Uuid> = r.try_get("", "video_item_id").ok().flatten();
        let episode_id: Option<Uuid> = r.try_get("", "episode_id").ok().flatten();
        let owner_id = video_item_id.or(episode_id).unwrap_or(vf_id);

        let path: String = r.try_get("", "path").unwrap_or_default();
        let filename: String = r.try_get("", "filename").unwrap_or_default();
        let size: Option<i64> = r.try_get("", "size").ok().flatten();
        let duration: Option<i32> = r.try_get("", "duration").ok().flatten();
        let video_codec: Option<String> = r.try_get("", "video_codec").ok().flatten();
        let video_width: Option<i32> = r.try_get("", "video_width").ok().flatten();
        let video_height: Option<i32> = r.try_get("", "video_height").ok().flatten();
        let video_profile: Option<String> = r.try_get("", "video_profile").ok().flatten();
        let video_streams_json: Option<serde_json::Value> = r.try_get("", "video_streams").ok().flatten();
        let audio_streams_json: Option<serde_json::Value> = r.try_get("", "audio_streams").ok().flatten();

        let container = filename.rsplit('.').next().unwrap_or("mkv").to_lowercase();

        let mut streams = Vec::new();
        let mut next_idx = 0i32;

        // Primary video stream (index 0)
        if let Some(ref codec) = video_codec {
            streams.push(build_video_stream_from_json(
                codec,
                video_width,
                video_height,
                video_profile.clone(),
                &video_streams_json,
            ));
            next_idx = 1;
        }

        // Audio streams
        let (audio_streams, default_audio_idx) = build_audio_streams_from_json(&audio_streams_json, next_idx);
        streams.extend(audio_streams);

        let bitrate = size.and_then(|s| duration.map(|d| if d > 0 { s * 8 / i64::from(d) } else { 0 }));

        let ms = MediaSourceInfo {
            protocol: "File".to_string(),
            id: vf_id.to_string(),
            path,
            source_type: "Default".to_string(),
            container,
            size,
            name: filename,
            is_remote: false,
            e_tag: None,
            run_time_ticks: duration.map(seconds_to_ticks),
            read_at_native_framerate: false,
            ignore_dts: false,
            ignore_index: false,
            gen_pts_input: false,
            supports_transcoding: true,
            supports_direct_stream: true,
            supports_direct_play: true,
            is_infinite_stream: false,
            use_most_compatible_transcoding_profile: false,
            requires_opening: false,
            requires_closing: false,
            requires_looping: false,
            supports_probing: true,
            video_type: Some("VideoFile".to_string()),
            media_streams: streams,
            media_attachments: Some(vec![]),
            formats: vec![],
            bitrate,
            default_audio_stream_index: default_audio_idx,
            default_subtitle_stream_index: None,
            transcoding_sub_protocol: Some("http".to_string()),
            has_segments: false,
            required_http_headers: HashMap::new(),
            direct_stream_url: None,
        };
        sources_map.entry(owner_id).or_default().push(ms);
    }

    // Apply to items — always set media_sources (even empty) for Movie/Episode
    for item in items.iter_mut() {
        if matches!(item.item_type.as_str(), "Movie" | "Episode")
            && let Ok(uid) = item.id.parse::<Uuid>()
        {
            let sources = sources_map.remove(&uid).unwrap_or_default();
            if let Some(first) = sources.first() {
                item.container = Some(first.container.clone());
                item.video_type = Some("VideoFile".to_string());
                item.path = Some(first.path.clone());
                if item.run_time_ticks.is_none() {
                    item.run_time_ticks = first.run_time_ticks;
                }
                let has_subs = first.media_streams.iter().any(|s| s.stream_type == "Subtitle");
                item.has_subtitles = Some(has_subs);
            }
            item.media_sources = Some(sources);
        }
    }

    Ok(())
}

/// Batch-load genres for Movie and Series items.
/// Sets `genres` (name list) and `genre_items` ({Name, Id} list) on each matching item.
async fn enrich_with_genres(db: &sea_orm::DatabaseConnection, items: &mut [BaseItemDto]) -> Result<(), sea_orm::DbErr> {
    let mut video_item_ids: Vec<Uuid> = Vec::new();
    let mut series_ids: Vec<Uuid> = Vec::new();

    for item in items.iter() {
        if let Ok(uid) = item.id.parse::<Uuid>() {
            match item.item_type.as_str() {
                "Movie" => video_item_ids.push(uid),
                "Series" => series_ids.push(uid),
                _ => {}
            }
        }
    }

    // Map from item UUID → Vec<(genre_uuid, tmdb_genre_id)>
    let mut genre_map: HashMap<Uuid, Vec<(Uuid, i32)>> = HashMap::new();

    fn ids_to_pg_array(ids: &[Uuid]) -> sea_orm::Value {
        let s = format!(
            "{{{}}}",
            ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
        );
        s.into()
    }

    if !video_item_ids.is_empty() {
        let sql = "SELECT mg.video_item_id, g.id, g.tmdb_genre_id FROM video_genres mg JOIN genres g ON g.id = mg.genre_id WHERE mg.video_item_id = ANY($1::uuid[])";
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [ids_to_pg_array(&video_item_ids)],
            ))
            .await?;
        for r in &rows {
            let video_item_id: Uuid = r.try_get("", "video_item_id").unwrap();
            let genre_id: Uuid = r.try_get("", "id").unwrap();
            let tmdb_id: i32 = r.try_get("", "tmdb_genre_id").unwrap_or(0);
            genre_map.entry(video_item_id).or_default().push((genre_id, tmdb_id));
        }
    }

    if !series_ids.is_empty() {
        let sql = "SELECT tg.tv_show_id, g.id, g.tmdb_genre_id FROM tv_show_genres tg JOIN genres g ON g.id = tg.genre_id WHERE tg.tv_show_id = ANY($1::uuid[])";
        let rows = db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [ids_to_pg_array(&series_ids)],
            ))
            .await?;
        for r in &rows {
            let show_id: Uuid = r.try_get("", "tv_show_id").unwrap();
            let genre_id: Uuid = r.try_get("", "id").unwrap();
            let tmdb_id: i32 = r.try_get("", "tmdb_genre_id").unwrap_or(0);
            genre_map.entry(show_id).or_default().push((genre_id, tmdb_id));
        }
    }

    for item in items.iter_mut() {
        if let Ok(uid) = item.id.parse::<Uuid>()
            && let Some(genres) = genre_map.remove(&uid)
        {
            item.genres = genres
                .iter()
                .map(|(_, tmdb_id)| tmdb_genre_name(*tmdb_id).to_string())
                .collect();
            item.genre_items = genres
                .iter()
                .map(|(genre_uuid, tmdb_id)| super::models::NameIdPair {
                    name: tmdb_genre_name(*tmdb_id).to_string(),
                    id: genre_uuid.to_string(),
                })
                .collect();
        }
    }

    Ok(())
}
struct FetchParams<'a> {
    video_id: Option<Uuid>,
    search_term: &'a str,
    is_resumable: bool,
    user_id: Uuid,
    server_id: &'a str,
    sort_field: &'a str,
    sort_dir: &'a str,
    start: i64,
    limit: i64,
}

/// Fetch movies and TV shows featuring a given person UUID.
/// Queries both `video_cast` (video_persons) and `tv_season_cast` (tv_persons),
/// then merges and sorts the results in-memory (person result sets are typically small).
async fn fetch_items_by_person(
    db: &sea_orm::DatabaseConnection,
    person_id: Uuid,
    user_id: Uuid,
    server_id: &str,
    sort_field: &str,
    sort_dir: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let pid = person_id.to_string();
    let uid = user_id.to_string();

    // --- Movies featuring this person ---
    let movie_sql = format!(
        r"SELECT m.id, m.title, m.original_title, m.sort_title, m.year, m.release_date,
                  m.runtime, m.tmdb_rating, m.imdb_rating, m.overview, m.tagline,
                  m.poster_path, m.backdrop_path, m.content_rating, m.is_favorite,
                  m.tmdb_id, m.imdb_id, m.created_at, m.video_id,
                  ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at
           FROM video_items m
           JOIN video_cast mc ON mc.video_item_id = m.id AND mc.video_person_id = '{pid}'
           LEFT JOIN user_media_states ums ON ums.video_item_id = m.id AND ums.user_id = '{uid}'"
    );
    let movie_rows = db
        .query_all_raw(Statement::from_string(DatabaseBackend::Postgres, movie_sql))
        .await?;

    let mut all_items: Vec<BaseItemDto> = movie_rows.iter().map(|r| movie_row_to_dto(r, server_id)).collect();

    // --- TV shows featuring this person (de-duplicate by show) ---
    let tv_sql = format!(
        r"SELECT t.id, t.title, t.original_title, t.sort_title, t.year, t.first_air_date,
                  t.tmdb_rating, t.imdb_rating, t.overview, t.status,
                  t.poster_path, t.backdrop_path, t.content_rating, t.is_favorite,
                  t.tmdb_id, t.imdb_id, t.tvdb_id, t.created_at, t.video_id,
                  (SELECT COUNT(*) FROM seasons s WHERE s.tv_show_id = t.id) as season_count,
                  (SELECT COUNT(*) FROM episodes e WHERE e.tv_show_id = t.id) as episode_count
           FROM tv_shows t
           WHERE EXISTS (
               SELECT 1 FROM tv_season_cast tsc
               JOIN seasons s ON s.id = tsc.season_id
               WHERE s.tv_show_id = t.id AND tsc.tv_person_id = '{pid}'
           )"
    );
    let tv_rows = db
        .query_all_raw(Statement::from_string(DatabaseBackend::Postgres, tv_sql))
        .await?;

    for r in &tv_rows {
        all_items.push(tv_show_row_to_dto(r, server_id));
    }

    let total = all_items.len() as i64;

    // Sort in-memory
    let order_key = |dto: &BaseItemDto| -> String {
        let key = match sort_field {
            "DateCreated" => dto.date_created.clone().unwrap_or_default(),
            "PremiereDate" => dto.premiere_date.clone().unwrap_or_default(),
            "CommunityRating" => dto.community_rating.map(|r| format!("{r:010.6}")).unwrap_or_default(),
            _ => dto.sort_name.clone().unwrap_or_else(|| dto.name.clone()),
        };
        key.to_lowercase()
    };

    if sort_dir == "DESC" {
        all_items.sort_by_key(|b| std::cmp::Reverse(order_key(b)));
    } else {
        all_items.sort_by_key(|a| order_key(a));
    }

    // Paginate
    let start = start.max(0) as usize;
    let limit = limit.max(1) as usize;
    let paged: Vec<BaseItemDto> = all_items.into_iter().skip(start).take(limit).collect();

    Ok((paged, total))
}

async fn fetch_video_items(
    db: &sea_orm::DatabaseConnection,
    p: &FetchParams<'_>,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let mut conditions = Vec::new();
    let mut params: Vec<sea_orm::Value> = Vec::new();
    let mut idx = 1;

    if let Some(aid) = p.video_id {
        conditions.push(format!("m.video_id = ${idx}"));
        params.push(aid.into());
        idx += 1;
    }
    if !p.search_term.is_empty() {
        conditions.push(format!("(m.title ILIKE ${idx} OR m.original_title ILIKE ${idx})"));
        params.push(format!("%{}%", p.search_term).into());
        idx += 1;
    }
    if p.is_resumable {
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM user_media_states ums WHERE ums.video_item_id = m.id AND ums.user_id = ${idx} AND ums.resume_position > 0 AND ums.is_watched = false)"
        ));
        params.push(p.user_id.into());
        idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let order = match p.sort_field {
        "DateCreated" => "m.created_at",
        "PremiereDate" => "COALESCE(m.release_date, m.created_at::date)",
        "CommunityRating" => "COALESCE(m.tmdb_rating, m.imdb_rating, 0)",
        "ProductionYear" => "m.year",
        _ => "COALESCE(m.sort_title, m.title)",
    };

    // Count
    let count_sql = format!("SELECT COUNT(*) as cnt FROM video_items m {where_clause}");
    let count_stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, &count_sql, params.clone());
    let total: i64 = db
        .query_one_raw(count_stmt)
        .await?
        .map_or(0, |r| r.try_get::<i64>("", "cnt").unwrap_or(0));

    // Fetch
    let user_id_param_idx = idx;
    params.push(p.user_id.into());
    params.push(p.limit.into());
    params.push(p.start.into());

    let sql = format!(
        r"SELECT m.id, m.title, m.original_title, m.sort_title, m.year, m.release_date,
                  m.runtime, m.tmdb_rating, m.imdb_rating, m.overview, m.tagline,
                  m.poster_path, m.backdrop_path, m.content_rating, m.is_favorite,
                  m.tmdb_id, m.imdb_id, m.created_at, m.video_id,
                  ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at
           FROM video_items m
           LEFT JOIN user_media_states ums ON ums.video_item_id = m.id AND ums.user_id = ${user_id_param_idx}
           {where_clause}
           ORDER BY {order} {}
           LIMIT ${} OFFSET ${}",
        p.sort_dir,
        user_id_param_idx + 1,
        user_id_param_idx + 2,
    );

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, params))
        .await?;
    let mut items = Vec::with_capacity(rows.len());
    for r in &rows {
        items.push(movie_row_to_dto(r, p.server_id));
    }
    Ok((items, total))
}

fn movie_row_to_dto(r: &sea_orm::QueryResult, server_id: &str) -> BaseItemDto {
    let id: Uuid = r.try_get("", "id").unwrap();
    let id_str = id.to_string();
    let title: String = r.try_get("", "title").unwrap_or_default();
    let original_title: Option<String> = r.try_get("", "original_title").ok().flatten();
    let sort_title: Option<String> = r.try_get("", "sort_title").ok().flatten();
    let year: Option<i32> = r.try_get("", "year").ok().flatten();
    let runtime: Option<i32> = r.try_get("", "runtime").ok().flatten();
    let tmdb_rating: Option<f64> = r.try_get("", "tmdb_rating").ok().flatten();
    let imdb_rating: Option<f64> = r.try_get("", "imdb_rating").ok().flatten();
    let overview: Option<String> = r.try_get("", "overview").ok().flatten();
    let tagline: Option<String> = r.try_get("", "tagline").ok().flatten();
    let poster_path: Option<String> = r.try_get("", "poster_path").ok().flatten();
    let backdrop_path: Option<String> = r.try_get("", "backdrop_path").ok().flatten();
    let content_rating: Option<String> = r.try_get("", "content_rating").ok().flatten();
    let is_favorite: bool = r.try_get("", "is_favorite").unwrap_or(false);
    let tmdb_id: Option<String> = r.try_get("", "tmdb_id").ok().flatten();
    let imdb_id: Option<String> = r.try_get("", "imdb_id").ok().flatten();
    let created_at: Option<chrono::DateTime<chrono::FixedOffset>> = r.try_get("", "created_at").ok().flatten();
    let video_id: Option<Uuid> = r.try_get("", "video_id").ok();
    let release_date: Option<chrono::NaiveDate> = r.try_get("", "release_date").ok().flatten();

    let resume_position: i32 = r.try_get("", "resume_position").unwrap_or(0);
    let play_count: i32 = r.try_get("", "play_count").unwrap_or(0);
    let is_watched: bool = r.try_get("", "is_watched").unwrap_or(false);
    let last_watch_at: Option<chrono::DateTime<chrono::FixedOffset>> = r.try_get("", "last_watch_at").ok().flatten();

    let mut image_tags = HashMap::new();
    if poster_path.is_some() {
        image_tags.insert("Primary".to_string(), id_str.clone());
    }
    let backdrop_tags = if backdrop_path.is_some() {
        vec![id_str.clone()]
    } else {
        vec![]
    };

    let mut provider_ids = HashMap::new();
    if let Some(ref tid) = tmdb_id {
        provider_ids.insert("Tmdb".to_string(), tid.clone());
    }
    if let Some(ref iid) = imdb_id {
        provider_ids.insert("Imdb".to_string(), iid.clone());
    }

    let taglines = tagline.map(|t| vec![t]).unwrap_or_default();
    // SortName falls back to title.to_lowercase() when not explicitly set
    let sort_name = sort_title.unwrap_or_else(|| title.to_lowercase());

    BaseItemDto {
        name: title,
        original_title,
        server_id: server_id.to_string(),
        id: id_str.clone(),
        item_type: "Movie".to_string(),
        sort_name: Some(sort_name),
        production_year: year,
        premiere_date: release_date.map(|d| format!("{d}T00:00:00.0000000Z")),
        run_time_ticks: runtime.map(seconds_to_ticks),
        community_rating: tmdb_rating.or(imdb_rating),
        official_rating: content_rating,
        overview,
        taglines,
        date_created: created_at.map(|d| d.to_rfc3339()),
        parent_id: video_id.map(|a| a.to_string()),
        is_folder: false,
        media_type: "Video".to_string(),
        location_type: "FileSystem".to_string(),
        play_access: "Full".to_string(),
        image_tags,
        backdrop_image_tags: backdrop_tags,
        provider_ids,
        user_data: Some(build_user_data_with_key(
            &id_str,
            tmdb_id.as_deref().unwrap_or(&id_str),
            resume_position,
            play_count,
            is_watched,
            is_favorite,
            last_watch_at,
        )),
        ..Default::default()
    }
}

/// Fetch all seasons in a TV library (by video_id via tv_shows.video_id).
async fn fetch_all_seasons_in_library(
    db: &sea_orm::DatabaseConnection,
    library_id: Uuid,
    user_id: Uuid,
    server_id: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let count_sql = r"
        SELECT COUNT(*) as cnt FROM seasons s
        JOIN tv_shows t ON t.id = s.tv_show_id
        WHERE t.video_id = $1
    ";
    let total: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            count_sql,
            [library_id.into()],
        ))
        .await?
        .map_or(0, |r| r.try_get::<i64>("", "cnt").unwrap_or(0));

    let sql = r"
        SELECT s.id, s.tv_show_id, s.season_number, s.title, s.overview,
               s.air_date, s.poster_path,
               COALESCE(
                   s.episode_count,
                   (SELECT COUNT(*)::int FROM episodes e WHERE e.season_id = s.id)
               ) AS episode_count,
               t.title as series_name, t.video_id,
               t.poster_path as series_poster_path, t.backdrop_path as series_backdrop_path
        FROM seasons s
        JOIN tv_shows t ON t.id = s.tv_show_id
        WHERE t.video_id = $1
        ORDER BY t.title, s.season_number
        LIMIT $2 OFFSET $3
    ";
    // reuse user_id for user data enrichment (not needed for seasons, but keep signature consistent)
    let _ = user_id;
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [library_id.into(), limit.into(), start.into()],
        ))
        .await?;

    let items: Vec<BaseItemDto> = rows.iter().map(|r| season_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

/// Fetch all episodes in a TV library (by video_id via tv_shows.video_id).
async fn fetch_all_episodes_in_library(
    db: &sea_orm::DatabaseConnection,
    library_id: Uuid,
    user_id: Uuid,
    server_id: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let count_sql = r"
        SELECT COUNT(*) as cnt FROM episodes e
        JOIN tv_shows t ON t.id = e.tv_show_id
        WHERE t.video_id = $1
    ";
    let total: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            count_sql,
            [library_id.into()],
        ))
        .await?
        .map_or(0, |r| r.try_get::<i64>("", "cnt").unwrap_or(0));

    let sql = r"
        SELECT e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
               e.air_date, e.runtime, e.still_path, e.tmdb_rating,
               s.season_number, t.title as series_name,
               ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
               EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
        FROM episodes e
        JOIN seasons s ON s.id = e.season_id
        JOIN tv_shows t ON t.id = e.tv_show_id
        LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = $2
        WHERE t.video_id = $1
        ORDER BY t.title, s.season_number, e.episode_number
        LIMIT $3 OFFSET $4
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [library_id.into(), user_id.into(), limit.into(), start.into()],
        ))
        .await?;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| episode_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

async fn fetch_tv_shows(
    db: &sea_orm::DatabaseConnection,
    p: &FetchParams<'_>,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let mut conditions = Vec::new();
    let mut params: Vec<sea_orm::Value> = Vec::new();
    let mut idx = 1;

    if let Some(aid) = p.video_id {
        conditions.push(format!("t.video_id = ${idx}"));
        params.push(aid.into());
        idx += 1;
    }
    if !p.search_term.is_empty() {
        conditions.push(format!("(t.title ILIKE ${idx} OR t.original_title ILIKE ${idx})"));
        params.push(format!("%{}%", p.search_term).into());
        idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let order = match p.sort_field {
        "DateCreated" => "t.created_at",
        "PremiereDate" => "COALESCE(t.first_air_date, t.created_at::date)",
        "CommunityRating" => "COALESCE(t.tmdb_rating, t.imdb_rating, 0)",
        "ProductionYear" => "t.year",
        _ => "COALESCE(t.sort_title, t.title)",
    };

    let count_sql = format!("SELECT COUNT(*) as cnt FROM tv_shows t {where_clause}");
    let count_stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, &count_sql, params.clone());
    let total: i64 = db
        .query_one_raw(count_stmt)
        .await?
        .map_or(0, |r| r.try_get::<i64>("", "cnt").unwrap_or(0));

    let user_idx = idx;
    params.push(p.user_id.into());
    params.push(p.limit.into());
    params.push(p.start.into());

    let sql = format!(
        r"SELECT t.id, t.title, t.original_title, t.sort_title, t.year, t.first_air_date,
                  t.tmdb_rating, t.imdb_rating, t.overview, t.status,
                  t.poster_path, t.backdrop_path, t.content_rating, t.is_favorite,
                  t.tmdb_id, t.imdb_id, t.tvdb_id, t.created_at, t.video_id,
                  (SELECT COUNT(*) FROM seasons s WHERE s.tv_show_id = t.id) as season_count,
                  (SELECT COUNT(*) FROM episodes e WHERE e.tv_show_id = t.id) as episode_count
           FROM tv_shows t
           {where_clause}
           ORDER BY {order} {}
           LIMIT ${} OFFSET ${}",
        p.sort_dir,
        user_idx + 1,
        user_idx + 2,
    );

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, params))
        .await?;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| tv_show_row_to_dto(r, p.server_id)).collect();
    Ok((items, total))
}

fn tv_show_row_to_dto(r: &sea_orm::QueryResult, server_id: &str) -> BaseItemDto {
    let id: Uuid = r.try_get("", "id").unwrap();
    let id_str = id.to_string();
    let title: String = r.try_get("", "title").unwrap_or_default();
    let original_title: Option<String> = r.try_get("", "original_title").ok().flatten();
    let sort_title: Option<String> = r.try_get("", "sort_title").ok().flatten();
    let year: Option<i32> = r.try_get("", "year").ok().flatten();
    let tmdb_rating: Option<f64> = r.try_get("", "tmdb_rating").ok().flatten();
    let imdb_rating: Option<f64> = r.try_get("", "imdb_rating").ok().flatten();
    let overview: Option<String> = r.try_get("", "overview").ok().flatten();
    let status: Option<String> = r.try_get("", "status").ok().flatten();
    let poster_path: Option<String> = r.try_get("", "poster_path").ok().flatten();
    let backdrop_path: Option<String> = r.try_get("", "backdrop_path").ok().flatten();
    let content_rating: Option<String> = r.try_get("", "content_rating").ok().flatten();
    let is_favorite: bool = r.try_get("", "is_favorite").unwrap_or(false);
    let tmdb_id: Option<String> = r.try_get("", "tmdb_id").ok().flatten();
    let imdb_id: Option<String> = r.try_get("", "imdb_id").ok().flatten();
    let tvdb_id: Option<String> = r.try_get("", "tvdb_id").ok().flatten();
    let created_at: Option<chrono::DateTime<chrono::FixedOffset>> = r.try_get("", "created_at").ok().flatten();
    let video_id: Option<Uuid> = r.try_get("", "video_id").ok();
    let first_air_date: Option<chrono::NaiveDate> = r.try_get("", "first_air_date").ok().flatten();
    let season_count: Option<i64> = r.try_get("", "season_count").ok();
    let episode_count: Option<i64> = r.try_get("", "episode_count").ok();

    let mut image_tags = HashMap::new();
    if poster_path.is_some() {
        image_tags.insert("Primary".to_string(), id_str.clone());
    }
    let backdrop_tags = if backdrop_path.is_some() {
        vec![id_str.clone()]
    } else {
        vec![]
    };

    let mut provider_ids = HashMap::new();
    if let Some(ref tid) = tmdb_id {
        provider_ids.insert("Tmdb".to_string(), tid.clone());
    }
    if let Some(ref iid) = imdb_id {
        provider_ids.insert("Imdb".to_string(), iid.clone());
    }
    if let Some(ref vid) = tvdb_id {
        provider_ids.insert("Tvdb".to_string(), vid.clone());
    }

    let sort_name = sort_title.unwrap_or_else(|| title.to_lowercase());

    BaseItemDto {
        name: title,
        original_title,
        server_id: server_id.to_string(),
        id: id_str.clone(),
        item_type: "Series".to_string(),
        sort_name: Some(sort_name),
        production_year: year,
        premiere_date: first_air_date.map(|d| format!("{d}T00:00:00.0000000Z")),
        community_rating: tmdb_rating.or(imdb_rating),
        official_rating: content_rating,
        overview,
        status,
        date_created: created_at.map(|d| d.to_rfc3339()),
        parent_id: video_id.map(|a| a.to_string()),
        is_folder: true,
        media_type: "Unknown".to_string(),
        location_type: "FileSystem".to_string(),
        play_access: "Full".to_string(),
        child_count: season_count.map(|c| c as i32),
        recursive_item_count: episode_count.map(|c| c as i32),
        image_tags,
        backdrop_image_tags: backdrop_tags,
        provider_ids,
        air_days: Some(vec![]),
        display_order: Some("aired".to_string()),
        user_data: Some(UserItemDataDto {
            is_favorite,
            key: tmdb_id.as_deref().or(tvdb_id.as_deref()).unwrap_or(&id_str).to_string(),
            item_id: id_str,
            ..Default::default()
        }),
        ..Default::default()
    }
}

async fn fetch_seasons_for_series(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    _user_id: Uuid,
    server_id: &str,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let sql = r"
        SELECT s.id, s.tv_show_id, s.season_number, s.title, s.overview,
               s.air_date, s.poster_path,
               COALESCE(
                   s.episode_count,
                   (SELECT COUNT(*)::int FROM episodes e WHERE e.season_id = s.id)
               ) AS episode_count,
               t.title as series_name, t.video_id,
               t.poster_path as series_poster_path, t.backdrop_path as series_backdrop_path
        FROM seasons s
        JOIN tv_shows t ON t.id = s.tv_show_id
        WHERE s.tv_show_id = $1
        ORDER BY s.season_number
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [series_id.into()],
        ))
        .await?;

    let total = rows.len() as i64;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| season_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

async fn fetch_episodes_for_season(
    db: &sea_orm::DatabaseConnection,
    season_id: Uuid,
    user_id: Uuid,
    server_id: &str,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let sql = r"
        SELECT e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
               e.air_date, e.runtime, e.still_path, e.tmdb_rating,
               s.season_number, t.title as series_name,
               ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
               EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
        FROM episodes e
        JOIN seasons s ON s.id = e.season_id
        JOIN tv_shows t ON t.id = e.tv_show_id
        LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = $2
        WHERE e.season_id = $1
        ORDER BY e.episode_number
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [season_id.into(), user_id.into()],
        ))
        .await?;

    let total = rows.len() as i64;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| episode_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

async fn fetch_episodes_for_series(
    db: &sea_orm::DatabaseConnection,
    series_id: Uuid,
    user_id: Uuid,
    server_id: &str,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let sql = r"
        SELECT e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
               e.air_date, e.runtime, e.still_path, e.tmdb_rating,
               s.season_number, t.title as series_name,
               ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
               EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
        FROM episodes e
        JOIN seasons s ON s.id = e.season_id
        JOIN tv_shows t ON t.id = e.tv_show_id
        LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = $2
        WHERE e.tv_show_id = $1
        ORDER BY s.season_number, e.episode_number
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [series_id.into(), user_id.into()],
        ))
        .await?;

    let total = rows.len() as i64;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| episode_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

fn episode_row_to_dto(r: &sea_orm::QueryResult, server_id: &str) -> BaseItemDto {
    let id: Uuid = r.try_get("", "id").unwrap();
    let id_str = id.to_string();
    let tv_show_id: Uuid = r.try_get("", "tv_show_id").unwrap();
    let season_id: Uuid = r.try_get("", "season_id").unwrap();
    let episode_number: i32 = r.try_get("", "episode_number").unwrap_or(0);
    let title: Option<String> = r.try_get("", "title").ok().flatten();
    let overview: Option<String> = r.try_get("", "overview").ok().flatten();
    let runtime: Option<i32> = r.try_get("", "runtime").ok().flatten();
    let still_path: Option<String> = r.try_get("", "still_path").ok().flatten();
    let tmdb_rating: Option<f64> = r.try_get("", "tmdb_rating").ok().flatten();
    let season_number: i32 = r.try_get("", "season_number").unwrap_or(0);
    let series_name: Option<String> = r.try_get("", "series_name").ok();
    let air_date: Option<chrono::NaiveDate> = r.try_get("", "air_date").ok().flatten();

    let resume_position: i32 = r.try_get("", "resume_position").unwrap_or(0);
    let play_count: i32 = r.try_get("", "play_count").unwrap_or(0);
    let is_watched: bool = r.try_get("", "is_watched").unwrap_or(false);
    let last_watch_at: Option<chrono::DateTime<chrono::FixedOffset>> = r.try_get("", "last_watch_at").ok().flatten();

    // has_file is present when the query includes the EXISTS subquery; fall back to true
    // if the column is absent (e.g. fetch_items_by_ids path which always finds a real file).
    let has_file: bool = r.try_get("", "has_file").unwrap_or(true);
    let location_type = if has_file { "FileSystem" } else { "Virtual" }.to_string();

    let name = title.unwrap_or_else(|| format!("Episode {episode_number}"));

    let mut image_tags = HashMap::new();
    if still_path.is_some() {
        image_tags.insert("Primary".to_string(), id_str.clone());
    }

    BaseItemDto {
        name,
        server_id: server_id.to_string(),
        id: id_str.clone(),
        item_type: "Episode".to_string(),
        index_number: Some(episode_number),
        parent_index_number: Some(season_number),
        parent_id: Some(season_id.to_string()),
        series_id: Some(tv_show_id.to_string()),
        season_id: Some(season_id.to_string()),
        series_name,
        season_name: Some(if season_number == 0 {
            "Specials".to_string()
        } else {
            format!("Season {season_number}")
        }),
        overview,
        premiere_date: air_date.map(|d| format!("{d}T00:00:00.0000000Z")),
        run_time_ticks: runtime.map(seconds_to_ticks),
        community_rating: tmdb_rating,
        is_folder: false,
        media_type: "Video".to_string(),
        location_type,
        play_access: "Full".to_string(),
        image_tags,
        parent_backdrop_item_id: Some(tv_show_id.to_string()),
        user_data: Some(build_user_data(
            &id_str,
            resume_position,
            play_count,
            is_watched,
            false,
            last_watch_at,
        )),
        ..Default::default()
    }
}

async fn fetch_all_episodes(
    db: &sea_orm::DatabaseConnection,
    search_term: &str,
    is_resumable: bool,
    user_id: Uuid,
    server_id: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let mut conditions = Vec::new();
    let mut params: Vec<sea_orm::Value> = Vec::new();
    let mut idx = 1;

    params.push(user_id.into());
    let user_idx = idx;
    idx += 1;

    if !search_term.is_empty() {
        conditions.push(format!("e.title ILIKE ${idx}"));
        params.push(format!("%{search_term}%").into());
        idx += 1;
    }
    if is_resumable {
        conditions.push("ums.resume_position > 0 AND ums.is_watched = false".to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("AND {}", conditions.join(" AND "))
    };

    let count_sql = format!(
        r"SELECT COUNT(*) as cnt FROM episodes e
           LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = ${user_idx}
           WHERE 1=1 {where_clause}"
    );
    let total: i64 = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            &count_sql,
            params.clone(),
        ))
        .await?
        .map_or(0, |r| r.try_get::<i64>("", "cnt").unwrap_or(0));

    params.push(limit.into());
    params.push(start.into());

    let sql = format!(
        r"SELECT e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
                  e.air_date, e.runtime, e.still_path, e.tmdb_rating,
                  s.season_number, t.title as series_name,
                  ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
                  EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
           FROM episodes e
           JOIN seasons s ON s.id = e.season_id
           JOIN tv_shows t ON t.id = e.tv_show_id
           LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = ${user_idx}
           WHERE 1=1 {where_clause}
           ORDER BY t.title, s.season_number, e.episode_number
           LIMIT ${} OFFSET ${}",
        idx,
        idx + 1,
    );

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres, &sql, params))
        .await?;
    let items: Vec<BaseItemDto> = rows.iter().map(|r| episode_row_to_dto(r, server_id)).collect();
    Ok((items, total))
}

async fn fetch_resumable(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    server_id: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    // Fetch resumable movies
    let fp = FetchParams {
        video_id: None,
        search_term: "",
        is_resumable: true,
        user_id,
        server_id,
        sort_field: "SortName",
        sort_dir: "ASC",
        start,
        limit,
    };
    let (video_items, video_total) = fetch_video_items(db, &fp).await?;
    // Fetch resumable episodes
    let (episodes, ep_total) = fetch_all_episodes(db, "", true, user_id, server_id, start, limit).await?;

    let mut combined = video_items;
    combined.extend(episodes);
    let total = video_total + ep_total;
    Ok((combined, total))
}

async fn fetch_all_media(
    db: &sea_orm::DatabaseConnection,
    search_term: &str,
    user_id: Uuid,
    server_id: &str,
    sort_field: &str,
    sort_dir: &str,
    start: i64,
    limit: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    let half = limit / 2;
    let fp = FetchParams {
        video_id: None,
        search_term,
        is_resumable: false,
        user_id,
        server_id,
        sort_field,
        sort_dir,
        start,
        limit: half,
    };
    let (video_items, vi_total) = fetch_video_items(db, &fp).await?;
    let (shows, s_total) = fetch_tv_shows(db, &fp).await?;
    let mut items = video_items;
    items.extend(shows);
    Ok((items, vi_total + s_total))
}

async fn fetch_items_by_ids(
    db: &sea_orm::DatabaseConnection,
    ids: &[&str],
    user_id: Uuid,
    server_id: &str,
) -> Result<Vec<BaseItemDto>, sea_orm::DbErr> {
    let mut items = Vec::new();
    for id_str in ids {
        let Ok(uid) = id_str.parse::<Uuid>() else {
            continue;
        };

        // Try movie
        let sql = r"
            SELECT m.id, m.title, m.original_title, m.sort_title, m.year, m.release_date,
                   m.runtime, m.tmdb_rating, m.imdb_rating, m.overview, m.tagline,
                   m.poster_path, m.backdrop_path, m.content_rating, m.is_favorite,
                   m.tmdb_id, m.imdb_id, m.created_at, m.video_id,
                   ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at
            FROM video_items m
            LEFT JOIN user_media_states ums ON ums.video_item_id = m.id AND ums.user_id = $2
            WHERE m.id = $1
        ";
        if let Some(r) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uid.into(), user_id.into()],
            ))
            .await?
        {
            items.push(movie_row_to_dto(&r, server_id));
            continue;
        }

        // Try tv_show
        let sql = r"
            SELECT t.id, t.title, t.original_title, t.sort_title, t.year, t.first_air_date,
                   t.tmdb_rating, t.imdb_rating, t.overview, t.status,
                   t.poster_path, t.backdrop_path, t.content_rating, t.is_favorite,
                   t.tmdb_id, t.imdb_id, t.tvdb_id, t.created_at, t.video_id,
                   (SELECT COUNT(*) FROM seasons s WHERE s.tv_show_id = t.id) as season_count,
                   (SELECT COUNT(*) FROM episodes e WHERE e.tv_show_id = t.id) as episode_count
            FROM tv_shows t WHERE t.id = $1
        ";
        if let Some(r) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uid.into()],
            ))
            .await?
        {
            items.push(tv_show_row_to_dto(&r, server_id));
            continue;
        }

        // Try season
        let sql = r"
            SELECT s.id, s.tv_show_id, s.season_number, s.title, s.overview,
                   s.air_date, s.poster_path,
                   COALESCE(
                       s.episode_count,
                       (SELECT COUNT(*)::int FROM episodes e WHERE e.season_id = s.id)
                   ) AS episode_count,
                   t.title as series_name, t.video_id,
                   t.poster_path as series_poster_path, t.backdrop_path as series_backdrop_path
            FROM seasons s
            JOIN tv_shows t ON t.id = s.tv_show_id
            WHERE s.id = $1
        ";
        if let Some(r) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uid.into()],
            ))
            .await?
        {
            items.push(season_row_to_dto(&r, server_id));
            continue;
        }

        // Try episode
        let sql = r"
            SELECT e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
                   e.air_date, e.runtime, e.still_path, e.tmdb_rating,
                   s.season_number, t.title as series_name,
                   ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
                   EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
            FROM episodes e
            JOIN seasons s ON s.id = e.season_id
            JOIN tv_shows t ON t.id = e.tv_show_id
            LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = $2
            WHERE e.id = $1
        ";
        if let Some(r) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uid.into(), user_id.into()],
            ))
            .await?
        {
            items.push(episode_row_to_dto(&r, server_id));
            continue;
        }

        // Try video library (CollectionFolder)
        let sql = r"
            SELECT a.id, a.name, a.type,
                   CASE a.type
                       WHEN 'movie' THEN (SELECT COUNT(*) FROM video_items WHERE video_id = a.id)
                       WHEN 'tv'    THEN (SELECT COUNT(*) FROM tv_shows WHERE video_id = a.id)
                       ELSE 0
                   END AS child_count
            FROM videos a
            WHERE a.id = $1 AND a.type IN ('movie', 'tv')
        ";
        if let Some(r) = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                [uid.into()],
            ))
            .await?
        {
            let id: Uuid = r.try_get("", "id").unwrap();
            let name: String = r.try_get("", "name").unwrap_or_default();
            let app_type: String = r.try_get("", "type").unwrap_or_default();
            let child_count: i64 = r.try_get("", "child_count").unwrap_or(0);
            let collection_type = match app_type.as_str() {
                "movie" => "movies",
                "tv" => "tvshows",
                _ => "mixed",
            };
            let id_str = id.to_string();
            items.push(BaseItemDto {
                name,
                server_id: server_id.to_string(),
                id: id_str.clone(),
                item_type: "CollectionFolder".to_string(),
                collection_type: Some(collection_type.to_string()),
                is_folder: true,
                child_count: Some(child_count as i32),
                recursive_item_count: Some(child_count as i32),
                enable_media_source_display: Some(true),
                play_access: "Full".to_string(),
                location_type: "FileSystem".to_string(),
                media_type: "Unknown".to_string(),
                user_data: Some(UserItemDataDto {
                    key: format_jellyfin_key(&id_str),
                    item_id: id_str,
                    ..Default::default()
                }),
                display_preferences_id: Some(id.to_string()),
                ..Default::default()
            });
        }
    }
    Ok(items)
}

// ── Additional public endpoints ───────────────────────────────────────────────

/// `GET /jellyfin/Users/{userId}/Items/{itemId}` — single item (userId-scoped alias for /Items/{itemId})
pub async fn get_user_item<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path((_user_id, item_id)): Path<(String, String)>,
    Query(q): Query<ItemsQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    match fetch_items_by_ids(db, &[item_id.as_str()], user.user_id, server_id).await {
        Ok(mut items) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields user_item: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources user_item: {e}");
            }
            let fields = q.fields.as_deref();
            if has_field(fields, "Genres")
                && let Err(e) = enrich_with_genres(db, &mut items).await
            {
                tracing::error!("enrich_with_genres: {e}");
            }
            if let Err(e) = enrich_with_people(db, &mut items).await {
                tracing::error!("enrich_with_people: {e}");
            }
            if let Some(item) = items.into_iter().next() {
                Json(item).into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(e) => {
            tracing::error!("jellyfin get_user_item: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Users/{userId}/Items/Root` — virtual root folder
pub async fn get_items_root<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    let server_id = state.server_id();
    Json(BaseItemDto {
        name: "Media Library".to_string(),
        server_id: server_id.to_string(),
        id: "root".to_string(),
        item_type: "UserRootFolder".to_string(),
        is_folder: true,
        play_access: "Full".to_string(),
        location_type: "FileSystem".to_string(),
        media_type: "Unknown".to_string(),
        ..Default::default()
    })
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub struct LatestQuery {
    #[serde(alias = "parentId")]
    pub parent_id: Option<String>,
    pub limit: Option<i64>,
    pub fields: Option<String>,
    #[serde(alias = "includeItemTypes")]
    pub include_item_types: Option<String>,
    #[serde(alias = "enableImages")]
    pub enable_images: Option<bool>,
    #[serde(alias = "enableUserData")]
    pub enable_user_data: Option<bool>,
}

/// `GET /jellyfin/Items/Latest` and `GET /jellyfin/Users/{userId}/Items/Latest`
/// Returns recently added items (movies + episodes), newest first.
pub async fn get_latest_items<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Query(q): Query<LatestQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    let limit = q.limit.unwrap_or(20).min(50);

    match fetch_latest(db, user.user_id, server_id, limit).await {
        Ok(mut items) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields latest: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources latest: {e}");
            }
            Json(items.into_iter().map(media_list_item_to_json).collect::<Vec<_>>()).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_latest_items: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_latest(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    server_id: &str,
    limit: i64,
) -> Result<Vec<BaseItemDto>, sea_orm::DbErr> {
    let sql = r"
        (SELECT 'movie' as kind, m.id::text, m.created_at
         FROM video_items m ORDER BY m.created_at DESC LIMIT $1)
        UNION ALL
        (SELECT 'show' as kind, t.id::text, t.created_at
         FROM tv_shows t ORDER BY t.created_at DESC LIMIT $1)
        ORDER BY created_at DESC
        LIMIT $1
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [limit.into()],
        ))
        .await?;

    let row_kinds: Vec<(String, String)> = rows
        .iter()
        .filter_map(|r| {
            let kind: String = r.try_get("", "kind").ok()?;
            let id: String = r.try_get("", "id").ok()?;
            Some((kind, id))
        })
        .collect();

    let mut items = Vec::with_capacity(row_kinds.len());
    for (kind, id) in &row_kinds {
        let fetched = if kind == "movie" {
            fetch_single_movie(db, id, user_id, server_id).await?
        } else {
            fetch_single_show(db, id, server_id).await?
        };
        if let Some(item) = fetched {
            items.push(item);
        }
    }
    Ok(items)
}

async fn fetch_single_movie(
    db: &sea_orm::DatabaseConnection,
    id: &str,
    user_id: Uuid,
    server_id: &str,
) -> Result<Option<BaseItemDto>, sea_orm::DbErr> {
    let sql = r"
        SELECT m.id, m.title, m.original_title, m.sort_title, m.year, m.release_date,
               m.runtime, m.tmdb_rating, m.imdb_rating, m.overview, m.tagline,
               m.poster_path, m.backdrop_path, m.content_rating, m.is_favorite,
               m.tmdb_id, m.imdb_id, m.created_at, m.video_id,
               ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at
        FROM video_items m
        LEFT JOIN user_media_states ums ON ums.video_item_id = m.id AND ums.user_id = $2
        WHERE m.id = $1
    ";
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [id.parse::<Uuid>().unwrap_or_default().into(), user_id.into()],
        ))
        .await?;
    Ok(row.as_ref().map(|r| movie_row_to_dto(r, server_id)))
}

async fn fetch_single_show(
    db: &sea_orm::DatabaseConnection,
    id: &str,
    server_id: &str,
) -> Result<Option<BaseItemDto>, sea_orm::DbErr> {
    let sql = r"
        SELECT t.id, t.title, t.original_title, t.sort_title, t.year, t.first_air_date,
               t.tmdb_rating, t.imdb_rating, t.overview, t.status,
               t.poster_path, t.backdrop_path, t.content_rating, t.is_favorite,
               t.tmdb_id, t.imdb_id, t.tvdb_id, t.created_at, t.video_id,
               (SELECT COUNT(*) FROM seasons s WHERE s.tv_show_id = t.id) as season_count,
               (SELECT COUNT(*) FROM episodes e WHERE e.tv_show_id = t.id) as episode_count
        FROM tv_shows t WHERE t.id = $1
    ";
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [id.parse::<Uuid>().unwrap_or_default().into()],
        ))
        .await?;
    Ok(row.as_ref().map(|r| tv_show_row_to_dto(r, server_id)))
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub struct NextUpQuery {
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
    #[serde(alias = "seriesId")]
    pub series_id: Option<String>,
    #[serde(alias = "parentId")]
    pub parent_id: Option<String>,
    pub limit: Option<i64>,
    #[serde(alias = "startIndex")]
    pub start_index: Option<i64>,
    pub fields: Option<String>,
    #[serde(alias = "enableImages")]
    pub enable_images: Option<bool>,
    #[serde(alias = "enableUserData")]
    pub enable_user_data: Option<bool>,
    #[serde(alias = "enableResumable")]
    pub enable_resumable: Option<bool>,
}

/// `GET /jellyfin/Shows/NextUp` — next unwatched episode per series.
pub async fn get_next_up<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Query(q): Query<NextUpQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    let limit = q.limit.unwrap_or(20).min(100);
    let start = q.start_index.unwrap_or(0);

    match fetch_next_up(db, user.user_id, server_id, limit, start).await {
        Ok((mut items, total)) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields next_up: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources next_up: {e}");
            }
            Json(media_list_query_result(items, total, start)).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_next_up: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_next_up(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    server_id: &str,
    limit: i64,
    start: i64,
) -> Result<(Vec<BaseItemDto>, i64), sea_orm::DbErr> {
    // For each series the user has started, find the next unwatched episode.
    // Priority: resumable (in-progress) > first unwatched after last watched.
    let sql = r"
        SELECT DISTINCT ON (e.tv_show_id)
            e.id, e.tv_show_id, e.season_id, e.episode_number, e.title, e.overview,
            e.air_date, e.runtime, e.still_path, e.tmdb_rating,
            s.season_number, t.title as series_name,
            ums.resume_position, ums.play_count, ums.is_watched, ums.last_watch_at,
            EXISTS(SELECT 1 FROM video_files vf WHERE vf.episode_id = e.id AND vf.is_available = true) as has_file
        FROM episodes e
        JOIN seasons s ON s.id = e.season_id
        JOIN tv_shows t ON t.id = e.tv_show_id
        LEFT JOIN user_media_states ums ON ums.episode_id = e.id AND ums.user_id = $1
        WHERE e.tv_show_id IN (
            SELECT DISTINCT e2.tv_show_id
            FROM episodes e2
            JOIN user_media_states ums2 ON ums2.episode_id = e2.id AND ums2.user_id = $1
            WHERE ums2.is_watched = true OR ums2.resume_position > 0
        )
        AND (ums.is_watched = false OR ums.is_watched IS NULL)
        ORDER BY e.tv_show_id, s.season_number, e.episode_number
    ";
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [user_id.into()],
        ))
        .await?;

    let total = rows.len() as i64;
    let items: Vec<BaseItemDto> = rows
        .iter()
        .skip(start as usize)
        .take(limit as usize)
        .map(|r| episode_row_to_dto(r, server_id))
        .collect();
    Ok((items, total))
}

/// `GET /jellyfin/Shows/Upcoming` — upcoming episodes (we don't have schedule data, return empty).
pub async fn get_upcoming<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Query(_q): Query<NextUpQuery>,
) -> impl IntoResponse {
    Json(QueryResult::<BaseItemDto> {
        items: vec![],
        total_record_count: 0,
        start_index: 0,
    })
}

/// `GET /jellyfin/UserItems/Resume` and `GET /jellyfin/Users/{userId}/Items/Resume`
pub async fn get_resume_items<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Query(q): Query<ItemsQuery>,
) -> impl IntoResponse {
    let db = state.db();
    let server_id = state.server_id();
    let limit = q.limit.unwrap_or(20).min(100);
    let start = q.start_index.unwrap_or(0);

    match fetch_resumable(db, user.user_id, server_id, start, limit).await {
        Ok((mut items, total)) => {
            if let Err(e) = enrich_with_shape_fields(db, &mut items).await {
                tracing::error!("enrich_with_shape_fields resumable: {e}");
            }
            if let Err(e) = enrich_with_media_sources(db, &mut items).await {
                tracing::error!("enrich_with_media_sources resumable: {e}");
            }
            Json(media_list_query_result(items, total, start)).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_resume_items: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// `GET /jellyfin/Users/{userId}/Suggestions` and `GET /jellyfin/Items/Suggestions`
/// Return empty — we don't implement a recommendation engine.
pub async fn get_suggestions<S: JellyfinAppState>(JellyfinAuth(_user, _): JellyfinAuth<S>) -> impl IntoResponse {
    Json(QueryResult::<BaseItemDto> {
        items: vec![],
        total_record_count: 0,
        start_index: 0,
    })
}

/// `GET /jellyfin/Items/{itemId}/LocalTrailers` — no local trailers.
pub async fn get_item_local_trailers<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path(_item_id): Path<String>,
) -> impl IntoResponse {
    Json(Vec::<BaseItemDto>::new())
}

/// `GET /jellyfin/Items/{itemId}/SpecialFeatures` — no special features.
pub async fn get_item_special_features<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path(_item_id): Path<String>,
) -> impl IntoResponse {
    Json(Vec::<BaseItemDto>::new())
}

/// `GET /jellyfin/Users/{userId}/Items/{itemId}/LocalTrailers`
pub async fn get_user_item_local_trailers<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path(_ids): Path<(String, String)>,
) -> impl IntoResponse {
    Json(Vec::<BaseItemDto>::new())
}

/// `GET /jellyfin/Users/{userId}/Items/{itemId}/SpecialFeatures`
pub async fn get_user_item_special_features<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    Path(_ids): Path<(String, String)>,
) -> impl IntoResponse {
    Json(Vec::<BaseItemDto>::new())
}
