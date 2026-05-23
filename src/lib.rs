pub mod jellyfin;
pub mod plex;

// Re-export jellyfin types for backward compatibility
pub use jellyfin::{JellyfinAppState, JellyfinPlaybackSession, JellyfinUser, build_jellyfin_routes};
