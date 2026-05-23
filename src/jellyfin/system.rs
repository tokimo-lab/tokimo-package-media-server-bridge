//! System/misc stub endpoints for Jellyfin client compatibility.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use super::{JellyfinAppState, auth::JellyfinAuth, models::PublicSystemInfo};

const JELLYFIN_VERSION: &str = "10.11.8";

/// `GET /jellyfin/System/Info/Public`
pub async fn get_public_info<S: JellyfinAppState>(State(state): State<Arc<S>>) -> impl IntoResponse {
    Json(PublicSystemInfo {
        local_address: state.public_base_url().to_string(),
        server_name: state.server_name().to_string(),
        version: JELLYFIN_VERSION.to_string(),
        product_name: "Jellyfin Server".to_string(),
        operating_system: String::new(),
        id: state.server_id().to_string(),
        startup_wizard_completed: true,
    })
}

/// `GET /jellyfin/System/Info` — full SystemInfo (extends PublicSystemInfo).
pub async fn get_system_info<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    Json(json!({
        "OperatingSystemDisplayName": "",
        "HasPendingRestart": false,
        "IsShuttingDown": false,
        "SupportsLibraryMonitor": true,
        "WebSocketPortNumber": 8096,
        "CompletedInstallations": [],
        "CanSelfRestart": true,
        "CanLaunchWebBrowser": false,
        "ProgramDataPath": "/config",
        "WebPath": "/jellyfin/jellyfin-web",
        "ItemsByNamePath": "/config/metadata",
        "CachePath": "/cache",
        "LogPath": "/config/log",
        "InternalMetadataPath": "/config/metadata",
        "TranscodingTempPath": "/cache/transcodes",
        "CastReceiverApplications": [
            { "Id": "F007D354", "Name": "Stable" },
            { "Id": "6F511C87", "Name": "Unstable" }
        ],
        "HasUpdateAvailable": false,
        "EncoderLocation": "System",
        "SystemArchitecture": "X64",
        "LocalAddress": state.public_base_url(),
        "ServerName": state.server_name(),
        "Version": JELLYFIN_VERSION,
        "ProductName": "Jellyfin Server",
        "OperatingSystem": "",
        "Id": state.server_id(),
        "StartupWizardCompleted": true,
    }))
}

/// `GET/POST /jellyfin/System/Ping`
pub async fn ping() -> impl IntoResponse {
    "Jellyfin Server"
}

/// `GET /jellyfin/Plugins` — return empty list (no plugins installed).
pub async fn get_plugins() -> impl IntoResponse {
    Json(serde_json::json!([
        {
            "Id": "9f064ad5-c0de-4e2f-8f0c-6f58ce8f1d10",
            "Name": "Tokimo Jellyfin Compatibility",
            "Description": "Jellyfin-compatible API bridge exposed by Tokimo for Infuse and other Jellyfin clients.",
            "Version": env!("CARGO_PKG_VERSION"),
            "Status": "Active",
            "HasImage": false,
            "CanUninstall": false,
            "ConfigurationFileName": serde_json::Value::Null
        }
    ]))
}

/// `GET /jellyfin/Packages` — return empty list (no packages available).
pub async fn get_packages() -> impl IntoResponse {
    Json(serde_json::json!([
        {
            "name": "Tokimo Jellyfin Compatibility",
            "guid": "9f064ad5-c0de-4e2f-8f0c-6f58ce8f1d10",
            "owner": "tokimo",
            "category": "General",
            "overview": "Jellyfin-compatible endpoints for browsing and direct-playback of Tokimo media libraries.",
            "description": "Provides the compatibility surface used by Infuse and other Jellyfin clients without requiring database schema changes or transcoding support.",
            "versions": [
                {
                    "version": env!("CARGO_PKG_VERSION"),
                    "targetAbi": JELLYFIN_VERSION,
                    "framework": "native",
                    "sourceUrl": "https://tokimo.io"
                }
            ]
        }
    ]))
}

/// `GET /jellyfin/Branding/Configuration` — matches real Jellyfin.
pub async fn get_branding_config() -> impl IntoResponse {
    Json(json!({
        "SplashscreenEnabled": false
    }))
}

/// `POST /jellyfin/Sessions/Capabilities` — client reports capabilities, we ignore.
pub async fn post_capabilities<S: JellyfinAppState>(JellyfinAuth(_, _): JellyfinAuth<S>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// `POST /jellyfin/Sessions/Capabilities/Full` — same.
pub async fn post_capabilities_full<S: JellyfinAppState>(JellyfinAuth(_, _): JellyfinAuth<S>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// `GET /jellyfin/Sessions/Logout` — revoke the token.
pub async fn post_logout<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(user, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let db = state.db();
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "DELETE FROM api_keys WHERE note = $1 AND user_id = $2",
        [user.access_token.into(), user.user_id.into()],
    );
    let _ = db.execute_raw(stmt).await;
    StatusCode::NO_CONTENT
}

/// `GET /jellyfin/UserViews/GroupingOptions` and `GET /jellyfin/Users/{userId}/GroupingOptions`
/// We don't support grouped views, return empty array.
pub async fn get_grouping_options<S: JellyfinAppState>(JellyfinAuth(_user, _): JellyfinAuth<S>) -> impl IntoResponse {
    Json(serde_json::Value::Array(vec![]))
}

/// `GET /jellyfin/DisplayPreferences/{displayPreferencesId}` — return sensible defaults.
pub async fn get_display_preferences<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "Id": id,
        "SortBy": "SortName",
        "RememberIndexing": false,
        "PrimaryImageHeight": 250,
        "PrimaryImageWidth": 250,
        "CustomPrefs": {},
        "ScrollDirection": "Horizontal",
        "ShowBackdrop": true,
        "RememberSorting": false,
        "SortOrder": "Ascending",
        "ShowSidebar": false,
        "Client": "emby"
    }))
}

/// `POST /jellyfin/DisplayPreferences/{displayPreferencesId}` — accept and ignore client prefs.
pub async fn post_display_preferences<S: JellyfinAppState>(
    JellyfinAuth(_user, _): JellyfinAuth<S>,
    axum::extract::Path(_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// `GET /jellyfin/Library/VirtualFolders` — list media libraries in VirtualFolderInfo format.
pub async fn get_virtual_folders<S: JellyfinAppState>(
    State(state): State<Arc<S>>,
    JellyfinAuth(_user, _): JellyfinAuth<S>,
) -> impl IntoResponse {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let db = state.db();
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r"
        SELECT a.id, a.name, a.type,
               COALESCE((
                   SELECT json_agg(afs.root_path ORDER BY afs.sort_order)
                   FROM app_file_systems afs
                   WHERE afs.app_id = a.id
               ), '[]'::json) AS locations
        FROM apps a
        WHERE a.type IN ('movie', 'tv')
        ORDER BY a.sort_order, a.created_at
        ",
        [],
    );
    match db.query_all_raw(stmt).await {
        Ok(rows) => {
            let folders: Vec<serde_json::Value> = rows
                .iter()
                .filter_map(|r| {
                    let id: uuid::Uuid = r.try_get("", "id").ok()?;
                    let name: String = r.try_get("", "name").ok()?;
                    let app_type: String = r.try_get("", "type").ok()?;
                    let locations: serde_json::Value =
                        r.try_get("", "locations").unwrap_or_else(|_| serde_json::json!([]));
                    let collection_type = match app_type.as_str() {
                        "movie" => "movies",
                        "tv" => "tvshows",
                        _ => return None,
                    };
                    Some(serde_json::json!({
                        "Name": name,
                        "Locations": locations,
                        "CollectionType": collection_type,
                        "LibraryOptions": {},
                        "ItemId": id.to_string(),
                        "PrimaryImageItemId": id.to_string(),
                        "RefreshStatus": "Idle"
                    }))
                })
                .collect();
            Json(folders).into_response()
        }
        Err(e) => {
            tracing::error!("jellyfin get_virtual_folders: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
