//! Jellyfin-compatible DTO models.
//!
//! Field names use `PascalCase` to match Jellyfin's JSON serialization.
//! Every field matches the real Jellyfin 10.11.x wire format for Infuse compatibility.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── System ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublicSystemInfo {
    pub local_address: String,
    pub server_name: String,
    pub version: String,
    pub product_name: String,
    pub operating_system: String,
    pub id: String,
    pub startup_wizard_completed: bool,
}

// ── Users ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct UserDto {
    pub name: String,
    pub server_id: String,
    pub id: String,
    pub has_password: bool,
    pub has_configured_password: bool,
    pub has_configured_easy_password: bool,
    pub enable_auto_login: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity_date: Option<String>,
    pub configuration: UserConfiguration,
    pub policy: UserPolicy,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct UserPolicy {
    pub is_administrator: bool,
    pub is_hidden: bool,
    pub enable_collection_management: bool,
    pub enable_subtitle_management: bool,
    pub enable_lyric_management: bool,
    pub is_disabled: bool,
    pub blocked_tags: Vec<String>,
    pub allowed_tags: Vec<String>,
    pub enable_user_preference_access: bool,
    pub access_schedules: Vec<serde_json::Value>,
    pub block_unrated_items: Vec<String>,
    pub enable_remote_control_of_other_users: bool,
    pub enable_shared_device_control: bool,
    pub enable_remote_access: bool,
    pub enable_live_tv_management: bool,
    pub enable_live_tv_access: bool,
    pub enable_media_playback: bool,
    pub enable_audio_playback_transcoding: bool,
    pub enable_video_playback_transcoding: bool,
    pub enable_playback_remuxing: bool,
    pub force_remote_source_transcoding: bool,
    pub enable_content_deletion: bool,
    pub enable_content_deletion_from_folders: Vec<String>,
    pub enable_content_downloading: bool,
    pub enable_sync_transcoding: bool,
    pub enable_media_conversion: bool,
    pub enabled_devices: Vec<String>,
    pub enable_all_devices: bool,
    pub enabled_channels: Vec<String>,
    pub enable_all_channels: bool,
    pub enabled_folders: Vec<String>,
    pub enable_all_folders: bool,
    pub invalid_login_attempt_count: i32,
    pub login_attempts_before_lockout: i32,
    pub max_active_sessions: i32,
    pub enable_public_sharing: bool,
    pub blocked_media_folders: Vec<String>,
    pub blocked_channels: Vec<String>,
    pub remote_client_bitrate_limit: i64,
    pub authentication_provider_id: String,
    pub password_reset_provider_id: String,
    pub sync_play_access: String,
}

impl Default for UserPolicy {
    fn default() -> Self {
        Self {
            is_administrator: true,
            is_hidden: true,
            enable_collection_management: false,
            enable_subtitle_management: false,
            enable_lyric_management: false,
            is_disabled: false,
            blocked_tags: vec![],
            allowed_tags: vec![],
            enable_user_preference_access: true,
            access_schedules: vec![],
            block_unrated_items: vec![],
            enable_remote_control_of_other_users: true,
            enable_shared_device_control: true,
            enable_remote_access: true,
            enable_live_tv_management: false,
            enable_live_tv_access: true,
            enable_media_playback: true,
            enable_audio_playback_transcoding: true,
            enable_video_playback_transcoding: true,
            enable_playback_remuxing: true,
            force_remote_source_transcoding: false,
            enable_content_deletion: true,
            enable_content_deletion_from_folders: vec![],
            enable_content_downloading: true,
            enable_sync_transcoding: true,
            enable_media_conversion: true,
            enabled_devices: vec![],
            enable_all_devices: true,
            enabled_channels: vec![],
            enable_all_channels: true,
            enabled_folders: vec![],
            enable_all_folders: true,
            invalid_login_attempt_count: 0,
            login_attempts_before_lockout: -1,
            max_active_sessions: 0,
            enable_public_sharing: true,
            blocked_media_folders: vec![],
            blocked_channels: vec![],
            remote_client_bitrate_limit: 0,
            authentication_provider_id: "Jellyfin.Server.Implementations.Users.DefaultAuthenticationProvider"
                .to_string(),
            password_reset_provider_id: "Jellyfin.Server.Implementations.Users.DefaultPasswordResetProvider"
                .to_string(),
            sync_play_access: "CreateAndJoinGroups".to_string(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct UserConfiguration {
    pub play_default_audio_track: bool,
    pub subtitle_language_preference: String,
    pub display_missing_episodes: bool,
    pub grouped_folders: Vec<String>,
    pub subtitle_mode: String,
    pub display_collections_view: bool,
    pub enable_local_password: bool,
    pub ordered_views: Vec<String>,
    pub latest_items_excludes: Vec<String>,
    pub my_media_excludes: Vec<String>,
    pub hide_played_in_latest: bool,
    pub remember_audio_selections: bool,
    pub remember_subtitle_selections: bool,
    pub enable_next_episode_auto_play: bool,
    pub cast_receiver_id: String,
}

impl Default for UserConfiguration {
    fn default() -> Self {
        Self {
            play_default_audio_track: true,
            subtitle_language_preference: String::new(),
            display_missing_episodes: false,
            grouped_folders: vec![],
            subtitle_mode: "Default".to_string(),
            display_collections_view: false,
            enable_local_password: false,
            ordered_views: vec![],
            latest_items_excludes: vec![],
            my_media_excludes: vec![],
            hide_played_in_latest: true,
            remember_audio_selections: true,
            remember_subtitle_selections: true,
            enable_next_episode_auto_play: true,
            cast_receiver_id: "F007D354".to_string(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticateByNameRequest {
    pub username: String,
    pub pw: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticationResult {
    pub user: UserDto,
    pub session_info: SessionInfoDto,
    pub access_token: String,
    pub server_id: String,
}

/// Session info returned inside AuthenticationResult.
#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SessionInfoDto {
    pub play_state: PlayStateDto,
    pub additional_users: Vec<serde_json::Value>,
    pub capabilities: SessionCapabilities,
    pub remote_end_point: String,
    pub playable_media_types: Vec<String>,
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub client: String,
    pub last_activity_date: String,
    pub last_playback_check_in: String,
    pub device_name: String,
    pub device_id: String,
    pub application_version: String,
    pub is_active: bool,
    pub supports_media_control: bool,
    pub supports_remote_control: bool,
    pub now_playing_queue: Vec<serde_json::Value>,
    pub now_playing_queue_full_items: Vec<serde_json::Value>,
    pub has_custom_device_name: bool,
    pub server_id: String,
    pub supported_commands: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct PlayStateDto {
    pub can_seek: bool,
    pub is_paused: bool,
    pub is_muted: bool,
    pub repeat_mode: String,
    pub playback_order: String,
}

impl Default for PlayStateDto {
    fn default() -> Self {
        Self {
            can_seek: false,
            is_paused: false,
            is_muted: false,
            repeat_mode: "RepeatNone".to_string(),
            playback_order: "Default".to_string(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SessionCapabilities {
    pub playable_media_types: Vec<String>,
    pub supported_commands: Vec<String>,
    pub supports_media_control: bool,
    pub supports_persistent_identifier: bool,
}

impl Default for SessionCapabilities {
    fn default() -> Self {
        Self {
            playable_media_types: vec![],
            supported_commands: vec![],
            supports_media_control: false,
            supports_persistent_identifier: true,
        }
    }
}

// ── Items (BaseItemDto) ───────────────────────────────────────────────────────

/// Matches Jellyfin's BaseItemDto exactly. Fields that are always present
/// in real Jellyfin are non-Option; rarely-present fields use skip_serializing_if.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItemDto {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    pub server_id: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_last_media_added: Option<String>,
    pub can_delete: bool,
    pub can_download: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_subtitles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_sort_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premiere_date: Option<String>,
    pub external_urls: Vec<ExternalUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_sources: Option<Vec<MediaSourceInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub critic_rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_locations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_media_source_display: Option<bool>,
    pub channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    pub taglines: Vec<String>,
    pub genres: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_time_ticks: Option<i64>,
    pub play_access: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_year: Option<i32>,
    pub remote_trailers: Vec<serde_json::Value>,
    pub provider_ids: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_h_d: Option<bool>,
    pub is_folder: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(rename = "Type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub people: Option<Vec<PersonDto>>,
    pub studios: Vec<NameIdPair>,
    pub genre_items: Vec<NameIdPair>,
    pub local_trailer_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<UserItemDataDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_feature_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_preferences_id: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_image_aspect_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_type: Option<String>,
    pub image_tags: HashMap<String, String>,
    pub backdrop_image_tags: Vec<String>,
    pub image_blur_hashes: HashMap<String, HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_streams: Option<Vec<MediaStream>>,
    pub location_type: String,
    pub media_type: String,
    pub locked_fields: Vec<String>,
    pub lock_data: bool,

    // Movie/Episode specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapters: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trickplay: Option<serde_json::Value>,

    // Series specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_index_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive_item_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_days: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_order: Option<String>,

    // Season/Episode parent references
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_primary_image_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_backdrop_item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_backdrop_image_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_logo_item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_logo_image_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thumb_item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thumb_image_tag: Option<String>,

    // Episode specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso_type: Option<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ExternalUrl {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct PersonDto {
    pub name: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(rename = "Type")]
    pub person_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_image_tag: Option<String>,
    pub image_blur_hashes: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct NameIdPair {
    pub name: String,
    pub id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
#[derive(Default)]
pub struct UserItemDataDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub played_percentage: Option<f64>,
    pub playback_position_ticks: i64,
    pub play_count: i32,
    pub is_favorite: bool,
    pub played: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unplayed_item_count: Option<i32>,
    pub key: String,
    pub item_id: String,
}

// ── QueryResult ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueryResult<T: Serialize> {
    pub items: Vec<T>,
    pub total_record_count: i64,
    pub start_index: i64,
}

// ── MediaSource / Stream ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSourceInfo {
    pub protocol: String,
    pub id: String,
    pub path: String,
    #[serde(rename = "Type")]
    pub source_type: String,
    pub container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    pub name: String,
    pub is_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_time_ticks: Option<i64>,
    pub read_at_native_framerate: bool,
    pub ignore_dts: bool,
    pub ignore_index: bool,
    pub gen_pts_input: bool,
    pub supports_transcoding: bool,
    pub supports_direct_stream: bool,
    pub supports_direct_play: bool,
    pub is_infinite_stream: bool,
    pub use_most_compatible_transcoding_profile: bool,
    pub requires_opening: bool,
    pub requires_closing: bool,
    pub requires_looping: bool,
    pub supports_probing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_type: Option<String>,
    pub media_streams: Vec<MediaStream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_attachments: Option<Vec<serde_json::Value>>,
    pub formats: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_audio_stream_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_subtitle_stream_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcoding_sub_protocol: Option<String>,
    pub has_segments: bool,
    pub required_http_headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_stream_url: Option<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStream {
    pub codec: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_transfer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_primaries: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_range_type: Option<String>,
    pub audio_spatial_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_title: Option<String>,
    pub is_interlaced: bool,
    #[serde(rename = "IsAVC")]
    pub is_avc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_frames: Option<i32>,
    pub is_default: bool,
    pub is_forced: bool,
    pub is_hearing_impaired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_frame_rate: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_frame_rate: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_frame_rate: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(rename = "Type")]
    pub stream_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    pub index: i32,
    pub is_external: bool,
    pub is_text_subtitle_stream: bool,
    pub supports_external_stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<String>,
    pub level: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_anamorphic: Option<bool>,
    // audio-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localized_default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localized_external: Option<String>,
}

// ── Playback Info ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfoResponse {
    pub media_sources: Vec<MediaSourceInfo>,
    pub play_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// ── Session Reporting ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackStartInfo {
    pub item_id: Uuid,
    #[serde(default)]
    pub media_source_id: Option<String>,
    #[serde(default)]
    pub position_ticks: Option<i64>,
    #[serde(default)]
    pub is_paused: Option<bool>,
    #[serde(default)]
    pub play_session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackProgressInfo {
    pub item_id: Uuid,
    #[serde(default)]
    pub media_source_id: Option<String>,
    #[serde(default)]
    pub position_ticks: Option<i64>,
    #[serde(default)]
    pub is_paused: Option<bool>,
    #[serde(default)]
    pub play_session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackStopInfo {
    pub item_id: Uuid,
    #[serde(default)]
    pub media_source_id: Option<String>,
    #[serde(default)]
    pub position_ticks: Option<i64>,
    #[serde(default)]
    pub play_session_id: Option<String>,
}
