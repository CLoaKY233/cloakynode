# CloakyNode

Ultra-light Raspberry Pi monitoring dashboard written in Rust.

## What it does

- Runs as a single Rust binary
- Polls system and Pi-specific metrics every 5 seconds
- Stores only a small in-memory history ring buffer
- Serves a dark-mode dashboard plus JSON API
- Avoids persistent metric storage

## API

- `GET /` dashboard
- `GET /api/current` latest sample
- `GET /api/history?limit=120` recent samples
- `GET /api/health` process and sampling health

## Project Layout

- `src/main.rs`: binary entrypoint
- `src/lib.rs`: app bootstrap and server startup
- `src/config.rs`: CLI and environment configuration
- `src/collector.rs`: sampling loop and `vcgencmd` integration
- `src/system.rs`: `/proc`, `/sys`, and parser helpers
- `src/state.rs`: shared runtime state
- `src/models.rs`: API and internal data models
- `src/web.rs`: HTTP routes and handlers
- `src/dashboard.rs`: embedded HTML/CSS/JS dashboard

## Run locally

```bash
cargo run --release
```

Custom options:

```bash
cargo run --release -- --host 0.0.0.0 --port 8080 --interval-seconds 5 --history-size 120
```

Environment variables:

- `CLOAKYNODE_HOST`
- `CLOAKYNODE_PORT`
- `CLOAKYNODE_INTERVAL_SECONDS`
- `CLOAKYNODE_HISTORY_SIZE`

## Raspberry Pi notes

- Best results come from Raspberry Pi OS where `vcgencmd` is available.
- General Linux metrics come from `/proc` and `/sys`.
- Pi-only fields degrade to `null` if `vcgencmd` is missing or fails.

## Development

```bash
cargo fmt
cargo test
```

## Current design constraints

- No persistent history
- No authentication
- LAN-oriented deployment
- Optimized for low overhead and easy extension, not long-term storage
