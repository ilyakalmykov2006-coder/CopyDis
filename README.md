# Hush

A deliberately small, private Windows desktop room for friends: one host runs the embedded Rust server and SQLite database; participants connect using the host's address over a private VPN such as Tailscale or ZeroTier.

## What is included

- Tauri 2 + React strict TypeScript desktop shell with a dark room UI.
- Embedded Axum WebSocket service for authentication, presence, chat signaling and a hard server-side 15-user limit.
- SQLite-backed message history and Argon2id room-password hashing.
- Actual browser WebRTC audio mesh: audio tracks use native WebRTC/Opus, while the WebSocket carries SDP and ICE signaling only.
- Local per-peer volume preferences, mute/deafen, device enumeration, microphone test, audio processing constraints, reconnecting WebSocket, typing notifications, editable/deletable messages, and voice/audio settings.

## Development

```sh
npm install
npm run tauri dev
```

To make a room, use **Create & host room**. The embedded host listens on port `43210`; give friends the `ws://<your-private-VPN-IP>:43210/ws` address and the room password. Do not expose this server to the public internet without placing it behind an authenticated private network.

## Production Windows build

```sh
npm run tauri build
```

The Tauri configuration requests an NSIS current-user installer and an app bundle. Build this on Windows 10/11 x64 with the Tauri Windows prerequisites installed.

## Privacy and security

There is no cloud backend or telemetry. Passwords are never persisted in plain text; diagnostics/logging intentionally avoid passwords, tokens, and keys. Message rendering uses React text nodes rather than inserting untrusted HTML.
