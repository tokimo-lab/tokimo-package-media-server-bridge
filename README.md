# tokimo-package-media-server-bridge

Media server compatibility layer for [TokimoOS](https://github.com/tokimo-lab/tokimo) — expose your media library via Jellyfin and Plex API protocols.

## Supported Protocols

| Protocol | Module | Status |
|----------|--------|--------|
| Jellyfin | `jellyfin` | Stable — full API coverage |
| Plex | `plex` | Skeleton — in progress |

## Architecture

Each protocol defines a trait that your application state must implement:

- **`JellyfinAppState`** — auth, users, items, images, playback, sessions
- **`PlexAppState`** — auth, library, media, playback

Both return an `axum::Router` that you merge into your server:

```rust
use tokimo_media_server_bridge::{JellyfinAppState, build_jellyfin_routes};

let jellyfin_router = build_jellyfin_routes(app_state);
// merge into your Axum router
```

## Endpoints

### Jellyfin

- `/Users/AuthenticateByName` — password login
- `/System/Info` — server info
- `/Users/{id}` — user profile
- `/Items` — browse library
- `/Items/{id}/Images/{type}` — artwork proxy
- `/Videos/{id}/stream` — direct play / HLS
- `/Sessions` — playback session tracking

### Plex (planned)

- `/library/sections` — library browse
- `/video/:/transcode` — streaming
- `/accounts` — X-Plex-Token auth

## Usage

```toml
[dependencies]
tokimo-media-server-bridge = { git = "https://github.com/tokimo-lab/tokimo-package-media-server-bridge.git", rev = "c066a43" }
```

## License

MIT OR Apache-2.0
