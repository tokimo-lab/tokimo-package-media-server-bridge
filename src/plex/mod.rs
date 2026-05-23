use std::sync::Arc;

use axum::Router;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

/// Minimal trait for the host AppState to implement Plex compatibility.
pub trait PlexAppState: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
    fn server_name(&self) -> &str;
    fn server_uuid(&self) -> &str;
    fn public_base_url(&self) -> &str;

    /// Stream a video file directly (Range-aware).
    fn stream_video_file(
        &self,
        file_id: Uuid,
        headers: axum::http::HeaderMap,
    ) -> impl std::future::Future<Output = axum::response::Response> + Send;

    /// Record a new active playback session. Default is a no-op.
    fn create_playback_session(&self, _session: PlexPlaybackSession) -> impl std::future::Future<Output = ()> + Send {
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

/// Input passed from a Plex client handler to record an active playback session.
pub struct PlexPlaybackSession {
    pub user_id: Uuid,
    pub file_id: Uuid,
    pub client_name: Option<String>,
    pub user_agent: Option<String>,
    pub position: i32,
}

/// Build all Plex-compatible routes.
/// Plex routes are mounted at the root (no `/plex/` prefix) since Plex clients
/// expect specific paths like `/library/sections`, `/video/{id}/stream`, etc.
pub fn build_plex_routes<S: PlexAppState>() -> Router<Arc<S>> {
    // TODO: Implement Plex route tree
    // Key endpoints:
    //   GET  /library/sections
    //   GET  /library/sections/{sectionId}/all
    //   GET  /library/metadata/{ratingKey}
    //   GET  /video/{id}/stream
    //   GET  /video/{id}/stream.{container}
    //   GET  /accounts
    //   POST /sessions
    //   POST /:/progress
    //   POST /:/scrobble
    Router::new()
}
