# CloakyNode

Ultra-light Raspberry Pi monitoring dashboard written in Rust.

## What it does

- Runs as a single Rust binary
- Polls system and Pi-specific metrics every 5 seconds
- Stores only a small in-memory history ring buffer
- Serves a dark-mode dashboard plus JSON API
- Avoids persistent metric storage

## Features

- **System Metrics:**
  - Aggregate and per-core CPU usage
  - Memory usage (total, used, buffers, cached, shared, swap)
  - Disk usage on the root filesystem
  - Uptime and load averages (1m / 5m)
  - Network interface rx/tx statistics
  - Top 10 CPU-consuming processes tracking
- **Raspberry Pi Specifics (`vcgencmd` integration):**
  - CPU and GPU temperatures
  - Core, SDRAM-C, SDRAM-I, SDRAM-P voltages
  - ARM and GPU clock frequencies
  - Throttling events and flags (under-voltage, frequency capped, etc.)
- **Built-in UI:** Beautiful, responsive dark-mode dashboard displaying charts and gauges.

## Installation

Pre-compiled binaries are available for all Raspberry Pi models on the [Releases](https://github.com/cloaky/cloakynode/releases) page.

- `armv6`: Raspberry Pi Zero / 1
- `armv7`: Raspberry Pi 2 / 3
- `aarch64`: Raspberry Pi 4 / 5 / Zero 2 W (running 64-bit OS)

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

## CI/CD and Releases

- The `ci.yml` GitHub Actions workflow runs `cargo fmt`, `clippy`, and `cargo test` on PRs and pushes to `main`.
- The `release.yml` GitHub Actions workflow runs when a `v*` tag is pushed. It cross-compiles release binaries for `armv6`, `armv7`, and `aarch64` (Raspberry Pi architectures) and creates a new GitHub release with these artifacts.

## Current design constraints

- No persistent history
- No authentication
- LAN-oriented deployment
- Optimized for low overhead and easy extension, not long-term storage
