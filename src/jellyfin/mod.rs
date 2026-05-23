pub mod auth;
pub mod images;
pub mod items;
pub mod models;
pub mod playback;
pub mod session;
pub mod system;
pub mod users;

use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::{Router, body::Body, http::HeaderMap, response::Response};
use http_body_util::BodyExt;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

/// Input passed from a Jellyfin client handler to record an active playback session.
/// The host `AppState` implementation resolves the rest of the file metadata itself.
pub struct JellyfinPlaybackSession {
    pub user_id: Uuid,
    pub file_id: Uuid,
    pub client_name: Option<String>,
    pub user_agent: Option<String>,
    pub position: i32,
}

/// Minimal trait that the host AppState must satisfy.
/// `tokimo-server` implements this for its own `AppState`.
pub trait JellyfinAppState: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
    fn server_id(&self) -> &str;
    fn server_name(&self) -> &str;
    /// Full external URL prefix, e.g. `http://192.168.1.100:5678`.
    fn public_base_url(&self) -> &str;
    /// Stream a video file directly (Range-aware). The implementor has access
    /// to VFS / SourceRegistry and can call the existing stream infrastructure.
    fn stream_video_file(
        &self,
        file_id: Uuid,
        headers: HeaderMap,
    ) -> impl std::future::Future<Output = Response> + Send;

    /// Record a new active playback session. Default implementation is a no-op.
    fn create_playback_session(
        &self,
        _session: JellyfinPlaybackSession,
    ) -> impl std::future::Future<Output = ()> + Send {
        std::future::ready(())
    }

    /// Update playback position for an active session (matched by file + user).
    fn update_playback_session_progress(
        &self,
        _user_id: Uuid,
        _file_id: Uuid,
        _position: i32,
    ) -> impl std::future::Future<Output = ()> + Send {
        std::future::ready(())
    }

    /// Mark the active session for a file + user as stopped.
    fn stop_playback_session(
        &self,
        _user_id: Uuid,
        _file_id: Uuid,
        _position: i32,
    ) -> impl std::future::Future<Output = ()> + Send {
        std::future::ready(())
    }
}

/// Authenticated Jellyfin user extracted by middleware.
#[derive(Clone, Debug)]
pub struct JellyfinUser {
    pub user_id: Uuid,
    pub user_name: String,
    pub access_token: String,
}

/// Build all Jellyfin-compatible routes under `/jellyfin/`.
pub fn build_jellyfin_routes<S: JellyfinAppState>() -> Router<Arc<S>> {
    let public = Router::new()
        .route("/jellyfin/System/Info/Public", axum::routing::get(system::get_public_info::<S>))
        .route("/jellyfin/System/Ping", axum::routing::get(system::ping).post(system::ping))
        .route("/jellyfin/Users/Public", axum::routing::get(users::get_public_users::<S>))
        .route(
            "/jellyfin/Users/AuthenticateByName",
            axum::routing::post(users::authenticate_by_name::<S>),
        )
        // Infuse probes these on connect (no auth required fallback)
        .route("/jellyfin/Plugins", axum::routing::get(system::get_plugins))
        .route("/jellyfin/Packages", axum::routing::get(system::get_packages))
        .route("/jellyfin/Branding/Configuration", axum::routing::get(system::get_branding_config));

    let authed = Router::new()
        .route("/jellyfin/System/Info", axum::routing::get(system::get_system_info::<S>))
        .route("/jellyfin/Sessions/Capabilities", axum::routing::post(system::post_capabilities::<S>))
        .route("/jellyfin/Sessions/Capabilities/Full", axum::routing::post(system::post_capabilities_full::<S>))
        .route("/jellyfin/Sessions/Logout", axum::routing::post(system::post_logout::<S>))
        // GroupingOptions — return [] (no grouped views)
        .route("/jellyfin/UserViews/GroupingOptions", axum::routing::get(system::get_grouping_options::<S>))
        .route("/jellyfin/Users/{userId}/GroupingOptions", axum::routing::get(system::get_grouping_options::<S>))
        // Library virtual folders
        .route("/jellyfin/Library/VirtualFolders", axum::routing::get(system::get_virtual_folders::<S>))
        // DisplayPreferences — stub
        .route("/jellyfin/DisplayPreferences/{displayPreferencesId}", axum::routing::get(system::get_display_preferences::<S>).post(system::post_display_preferences::<S>))
        .route("/jellyfin/Users/Me", axum::routing::get(users::get_me::<S>))
        .route("/jellyfin/Users/{userId}", axum::routing::get(users::get_user::<S>))
        // Library views
        .route("/jellyfin/UserViews", axum::routing::get(items::get_user_views::<S>))
        .route("/jellyfin/Users/{userId}/Views", axum::routing::get(items::get_user_views::<S>))
        // Items root
        .route("/jellyfin/Items/Root", axum::routing::get(items::get_items_root::<S>))
        .route("/jellyfin/Users/{userId}/Items/Root", axum::routing::get(items::get_items_root::<S>))
        // Items browse
        .route("/jellyfin/Items", axum::routing::get(items::get_items::<S>))
        .route("/jellyfin/Users/{userId}/Items", axum::routing::get(items::get_items::<S>))
        // Single item — must register specific paths before the wildcard /{itemId}
        .route("/jellyfin/Items/Latest", axum::routing::get(items::get_latest_items::<S>))
        .route("/jellyfin/Users/{userId}/Items/Latest", axum::routing::get(items::get_latest_items::<S>))
        .route("/jellyfin/UserItems/Resume", axum::routing::get(items::get_resume_items::<S>))
        .route("/jellyfin/Users/{userId}/Items/Resume", axum::routing::get(items::get_resume_items::<S>))
        .route("/jellyfin/Items/{itemId}", axum::routing::get(items::get_item::<S>))
        .route("/jellyfin/Users/{userId}/Items/{itemId}", axum::routing::get(items::get_user_item::<S>))
        // Trailers / special features — return empty
        .route("/jellyfin/Items/{itemId}/LocalTrailers", axum::routing::get(items::get_item_local_trailers::<S>))
        .route("/jellyfin/Items/{itemId}/SpecialFeatures", axum::routing::get(items::get_item_special_features::<S>))
        .route("/jellyfin/Users/{userId}/Items/{itemId}/LocalTrailers", axum::routing::get(items::get_user_item_local_trailers::<S>))
        .route("/jellyfin/Users/{userId}/Items/{itemId}/SpecialFeatures", axum::routing::get(items::get_user_item_special_features::<S>))
        // TV Shows
        .route("/jellyfin/Shows/{seriesId}/Seasons", axum::routing::get(items::get_seasons::<S>))
        .route("/jellyfin/Shows/{seriesId}/Episodes", axum::routing::get(items::get_episodes::<S>))
        .route("/jellyfin/Shows/NextUp", axum::routing::get(items::get_next_up::<S>))
        .route("/jellyfin/Shows/Upcoming", axum::routing::get(items::get_upcoming::<S>))
        // Suggestions — return empty
        .route("/jellyfin/Items/Suggestions", axum::routing::get(items::get_suggestions::<S>))
        .route("/jellyfin/Users/{userId}/Suggestions", axum::routing::get(items::get_suggestions::<S>))
        // Images
        .route("/jellyfin/Items/{itemId}/Images/{imageType}", axum::routing::get(images::get_item_image::<S>))
        .route("/jellyfin/Items/{itemId}/Images/{imageType}/{imageIndex}", axum::routing::get(images::get_item_image_by_index::<S>))
        // Playback info + stream
        .route("/jellyfin/Items/{itemId}/PlaybackInfo", axum::routing::get(playback::get_playback_info::<S>).post(playback::post_playback_info::<S>))
        .route("/jellyfin/Videos/{videoFileId}/stream", axum::routing::get(playback::stream_video::<S>))
        .route("/jellyfin/Videos/{videoFileId}/stream.{container}", axum::routing::get(playback::stream_video_container::<S>))
        // Session / playstate
        .route("/jellyfin/Sessions/Playing", axum::routing::post(session::on_playback_start::<S>))
        .route("/jellyfin/Sessions/Playing/Progress", axum::routing::post(session::on_playback_progress::<S>))
        .route("/jellyfin/Sessions/Playing/Stopped", axum::routing::post(session::on_playback_stopped::<S>))
        .route("/jellyfin/UserPlayedItems/{itemId}", axum::routing::post(session::mark_played::<S>).delete(session::mark_unplayed::<S>))
        .route("/jellyfin/Users/{userId}/PlayedItems/{itemId}", axum::routing::post(session::mark_played_legacy::<S>).delete(session::mark_unplayed_legacy::<S>));

    public.merge(authed).layer(middleware::from_fn(jellyfin_request_logger))
}

async fn jellyfin_request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let auth_header = req.headers().get("Authorization").cloned();
    let emby_token = req.headers().get("X-Emby-Token").cloned();
    let user_agent = req.headers().get("User-Agent").cloned();

    tracing::info!(
        "\n╔══ JELLYFIN REQUEST ══════════════════════════════════════\n\
         ║ {} {}\n\
         ║ User-Agent: {}\n\
         ║ Auth: {}\n\
         ║ X-Emby-Token: {}\n\
         ╚════════════════════════════════════════════════════════════",
        method,
        uri,
        user_agent.as_ref().map_or("-", |v| v.to_str().unwrap_or("?")),
        auth_header.as_ref().map_or("-", |v| v.to_str().unwrap_or("?")),
        emby_token.as_ref().map_or("-", |v| v.to_str().unwrap_or("?")),
    );

    let resp = next.run(req).await;
    let status = resp.status();

    // Collect response body for debugging (JSON responses only)
    let (parts, body) = resp.into_parts();
    let is_json = parts
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("json"));

    if is_json {
        match body.collect().await {
            Ok(collected) => {
                let bytes = collected.to_bytes();
                let body_str = std::str::from_utf8(&bytes).unwrap_or("<binary>");
                let preview = if body_str.len() > 2000 {
                    // Find safe UTF-8 char boundary at or before 2000 bytes
                    let mut end = 2000;
                    while !body_str.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…(+{})", &body_str[..end], body_str.len() - end)
                } else {
                    body_str.to_string()
                };
                tracing::info!(
                    "╔══ JELLYFIN RESPONSE ═════════════════════════════════════\n\
                     ║ {} → {}\n\
                     ║ Body: {}\n\
                     ╚════════════════════════════════════════════════════════════",
                    uri,
                    status,
                    preview,
                );
                Response::from_parts(parts, Body::from(bytes))
            }
            Err(e) => {
                tracing::warn!("jellyfin logger: failed to read body: {e}");
                tracing::info!(
                    "╔══ JELLYFIN RESPONSE (body read error) ══════════════════\n\
                     ║ {} → {}\n\
                     ╚════════════════════════════════════════════════════════════",
                    uri,
                    status,
                );
                Response::from_parts(parts, Body::empty())
            }
        }
    } else {
        tracing::info!(
            "╔══ JELLYFIN RESPONSE ═════════════════════════════════════\n\
             ║ {} → {}\n\
             ╚════════════════════════════════════════════════════════════",
            uri,
            status,
        );
        Response::from_parts(parts, body)
    }
}
