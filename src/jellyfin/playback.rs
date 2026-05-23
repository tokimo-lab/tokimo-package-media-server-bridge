//! Playback endpoints: `/Items/{itemId}/PlaybackInfo`, `/Videos/{videoFileId}/stream`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::Deserialize;
use uuid::Uuid;

use super::{
    JellyfinAppState,
    auth::JellyfinAuth,
    models::{MediaSourceInfo, MediaStream, PlaybackInfoResponse},
};

pub(crate) fn seconds_to_ticks(secs: i32) -> i64 {
    i64::from(secs) * 10_000_000
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct PlaybackInfoQuery {
    #[serde(alias = "userId")]
    pub user_id: Option<String>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct StreamQuery {
    pub api_key: Option<String>,
    #[serde(alias = "Static")]
    pub is_static: Option<bool>,
    #[serde(alias = "mediaSourceId", alias = "MediaSourceId")]
    pub media_source_id: Option<String>,
}

/// `GET /jellyfin/Items/{itemId}/PlaybackInfo`
pub async fn get_playback_info<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(item_id): Path<Uuid>,
    Query(_q): Query<PlaybackInfoQuery>,
) -> impl IntoResponse {
    build_playback_info(state, item_id, &user.access_token).await
}

/// `POST /jellyfin/Items/{itemId}/PlaybackInfo`
pub async fn post_playback_info<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
    Path(item_id): Path<Uuid>,
) -> impl IntoResponse {
    build_playback_info(state, item_id, &user.access_token).await
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_video_stream(
    codec: &str,
    index: i32,
    is_default: bool,
    width: Option<i32>,
    height: Option<i32>,
    profile: Option<String>,
    language: Option<String>,
    display_title: Option<String>,
    bit_rate: Option<i64>,
) -> MediaStream {
    let is_hd = width.unwrap_or(0) >= 1280 || height.unwrap_or(0) >= 720;
    MediaStream {
        codec: codec.to_string(),
        stream_type: "Video".to_string(),
        index,
        is_default,
        is_forced: false,
        is_external: false,
        display_title,
        width,
        height,
        bit_rate,
        profile,
        level: 0,
        is_interlaced: false,
        is_avc: codec == "h264",
        is_text_subtitle_stream: false,
        supports_external_stream: false,
        audio_spatial_format: "None".to_string(),
        is_hearing_impaired: false,
        language,
        video_range: if is_hd { Some("SDR".to_string()) } else { None },
        video_range_type: if is_hd { Some("SDR".to_string()) } else { None },
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_audio_stream(
    codec: &str,
    index: i32,
    is_default: bool,
    language: Option<String>,
    display_title: Option<String>,
    bit_rate: Option<i64>,
    channels: Option<i32>,
    sample_rate: Option<i32>,
) -> MediaStream {
    let channel_layout = match channels {
        Some(8) => Some("7.1".to_string()),
        Some(6) => Some("5.1".to_string()),
        Some(2) => Some("stereo".to_string()),
        Some(1) => Some("mono".to_string()),
        _ => None,
    };
    MediaStream {
        codec: codec.to_string(),
        stream_type: "Audio".to_string(),
        index,
        is_default,
        is_forced: false,
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
    }
}

async fn build_playback_info<S: JellyfinAppState>(
    state: Arc<S>,
    item_id: Uuid,
    _access_token: &str,
) -> axum::response::Response {
    let db = state.db();

    let sql = r"
        SELECT vf.id, vf.path, vf.filename, vf.size, vf.duration, vf.mime_type,
               vf.video_codec, vf.video_width, vf.video_height, vf.video_profile,
               vf.video_streams, vf.audio_streams,
               fs.type as source_type
        FROM video_files vf
        LEFT JOIN file_systems fs ON fs.id = vf.source_id
        WHERE (vf.video_item_id = $1 OR vf.episode_id = $1)
          AND vf.is_available = true
        ORDER BY vf.size DESC
    ";
    let rows = match db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [item_id.into()],
        ))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("jellyfin playback_info: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if rows.is_empty() {
        return Json(PlaybackInfoResponse {
            media_sources: vec![],
            play_session_id: Uuid::new_v4().to_string(),
            error_code: Some("NotFound".to_string()),
        })
        .into_response();
    }

    let base_url = state.public_base_url();
    let _ = &base_url; // kept for future use
    let mut media_sources = Vec::with_capacity(rows.len());

    for r in &rows {
        let vf_id: Uuid = r.try_get("", "id").unwrap();
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
        let mut stream_idx = 0;

        // Primary video stream
        if let Some(codec) = &video_codec {
            let display_title = format!(
                "{}{}",
                codec.to_uppercase(),
                video_width.map_or(String::new(), |w| {
                    let h = video_height.unwrap_or(0);
                    format!(" {w}x{h}")
                })
            );
            streams.push(build_video_stream(
                codec,
                stream_idx,
                true,
                video_width,
                video_height,
                video_profile.clone(),
                None,
                Some(display_title),
                None,
            ));
            stream_idx += 1;
        }

        // Additional video streams from JSONB
        if let Some(serde_json::Value::Array(vs)) = &video_streams_json {
            for (i, v) in vs.iter().enumerate() {
                if i == 0 {
                    continue;
                }
                let codec = v.get("codec").and_then(|c| c.as_str()).unwrap_or("unknown");
                streams.push(build_video_stream(
                    codec,
                    stream_idx,
                    false,
                    v.get("width").and_then(serde_json::Value::as_i64).map(|w| w as i32),
                    v.get("height").and_then(serde_json::Value::as_i64).map(|h| h as i32),
                    v.get("profile").and_then(|p| p.as_str()).map(String::from),
                    v.get("language").and_then(|l| l.as_str()).map(String::from),
                    None,
                    v.get("bit_rate").and_then(serde_json::Value::as_i64),
                ));
                stream_idx += 1;
            }
        }

        // Audio streams from JSONB
        if let Some(serde_json::Value::Array(audio)) = &audio_streams_json {
            for (i, a) in audio.iter().enumerate() {
                let codec = a.get("codec").and_then(|c| c.as_str()).unwrap_or("aac");
                streams.push(build_audio_stream(
                    codec,
                    stream_idx,
                    i == 0,
                    a.get("language").and_then(|l| l.as_str()).map(String::from),
                    a.get("title").and_then(|t| t.as_str()).map(String::from),
                    a.get("bit_rate").and_then(serde_json::Value::as_i64),
                    a.get("channels").and_then(serde_json::Value::as_i64).map(|c| c as i32),
                    a.get("sample_rate")
                        .and_then(serde_json::Value::as_i64)
                        .map(|s| s as i32),
                ));
                stream_idx += 1;
            }
        }

        let bitrate = size.and_then(|s| duration.map(|d| if d > 0 { s * 8 / i64::from(d) } else { 0 }));

        media_sources.push(MediaSourceInfo {
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
            default_audio_stream_index: Some(1),
            default_subtitle_stream_index: None,
            transcoding_sub_protocol: Some("http".to_string()),
            has_segments: false,
            required_http_headers: HashMap::new(),
            direct_stream_url: None,
        });
    }

    Json(PlaybackInfoResponse {
        media_sources,
        play_session_id: Uuid::new_v4().to_string(),
        error_code: None,
    })
    .into_response()
}

/// `GET /jellyfin/Videos/{itemId}/stream`
///
/// Infuse passes the item UUID (movie/episode) in the path, and the actual
/// MediaSource UUID (= video_file UUID) as `?MediaSourceId=...`.
/// We prefer MediaSourceId for the lookup, falling back to the path param.
pub async fn stream_video<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    Path(item_id): Path<Uuid>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let api_key = q
        .api_key
        .clone()
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(super::auth::parse_mediabrowser_token)
        })
        .or_else(|| {
            headers
                .get("x-emby-token")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });

    let Some(api_key) = api_key else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let db = state.db();
    if super::auth::resolve_token(db, &api_key).await.ok().flatten().is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Prefer MediaSourceId (= video_file UUID), fall back to path param.
    let video_file_id = q
        .media_source_id
        .as_deref()
        .and_then(|s| s.parse::<Uuid>().ok())
        .unwrap_or(item_id);

    state.stream_video_file(video_file_id, headers).await
}

/// `GET /jellyfin/Videos/{videoFileId}/stream.{container}`
pub async fn stream_video_container<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    Path((video_file_id, _container)): Path<(Uuid, String)>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    stream_video(State(state), Path(video_file_id), Query(q), headers).await
}
