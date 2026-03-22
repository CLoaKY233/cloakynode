# AGENTS.md

Guidelines for agentic coding assistants working in the CloakyNode repository.

## Project Overview

CloakyNode is an ultra-light Raspberry Pi monitoring dashboard written in Rust. It polls system metrics and serves them via a web dashboard and JSON API.

## Build, Lint, and Test Commands

```bash
# Build (release mode recommended for deployment)
cargo build --release

# Run the application
cargo run --release

# Run with custom options
cargo run --release -- --host 0.0.0.0 --port 8080 --interval-seconds 5 --history-size 120

# Run all tests
cargo test

# Run a single test
cargo test test_name
cargo test parses_meminfo
cargo test --lib system::tests::parses_loadavg

# Run tests in a specific file
cargo test --lib web

# Format code
cargo fmt

# Lint with clippy
cargo clippy -- -D warnings

# Check for compilation errors
cargo check
```

## Code Style Guidelines

### Imports

Group imports in this order with blank lines between groups:

1. `std` imports
2. External crate imports (alphabetically)
3. Internal `crate::` imports

```rust
use std::{collections::VecDeque, sync::Arc, time::Instant};

use axum::{Json, Router, extract::State};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::models::SystemSample;
```

### Formatting

- Use `cargo fmt` before committing
- 4-space indentation
- Trailing commas in multi-line structs, arrays, and function calls
- Max line length follows rustfmt defaults

### Types and Structs

- Place each derive attribute on its own line for complex structs
- Use `#[must_use]` on public constructors that return new instances
- Use `#[allow(clippy::...)]` sparingly with justification

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemSample {
    pub timestamp_unix_ms: u64,
    pub cpu_usage_percent: f32,
}
```

### Naming Conventions

- `snake_case` for functions, variables, and modules
- `PascalCase` for types, traits, and enums
- `SCREAMING_SNAKE_CASE` for constants
- Module names should be single words when possible

### Error Handling

- Use `Result<T, String>` for fallible system operations with descriptive error messages
- Use `Option<T>` for parsing operations that can fail gracefully
- Convert errors with `.map_err(|error| error.to_string())`
- Use `.ok()` to convert `Result` to `Option` when errors should be silently ignored
- Use `.unwrap_or_default()` or `.unwrap_or_else()` for safe defaults
- Main entry point returns `Result<(), Box<dyn std::error::Error>>`

```rust
pub fn read_meminfo() -> Result<MemInfo, String> {
    let raw = std::fs::read_to_string("/proc/meminfo")
        .map_err(|error| error.to_string())?;
    parse_meminfo(&raw)
}
```

### Async Patterns

- Use tokio runtime for async operations
- Use `#[tokio::main]` in binary entry points
- Use `#[tokio::test]` for async tests
- Use `tokio::sync::RwLock` for async-safe shared state
- Use `Arc` for sharing state across tasks

### Documentation

- Use `///` doc comments for public items
- Include `# Errors` section for fallible functions
- Include `# Panics` section if applicable

```rust
/// Read memory counters from `/proc/meminfo`.
///
/// # Errors
///
/// Returns an error if `/proc/meminfo` cannot be read or parsed.
pub fn read_meminfo() -> Result<MemInfo, String> {
```

### Testing

- Place tests inline in the same module with `#[cfg(test)]`
- Use descriptive test names like `parses_meminfo` or `history_endpoint_clamps_limit`
- Use `.expect()` with descriptive messages in tests
- Use `assert_eq!` and `assert!` macros

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let mem = parse_meminfo("MemTotal: 947952 kB\nMemAvailable: 531140 kB\n")
            .expect("meminfo should parse");
        assert_eq!(mem.total_bytes, 947_952 * 1024);
    }
}
```

### Web Handlers

- Use `impl IntoResponse` for handler return types
- Extract state with `State(state): State<AppState>`
- Extract query params with `Query(query): Query<QueryStruct>`
- Use `Json(response)` for JSON responses

### Project Structure

```
src/
  main.rs      # Binary entrypoint with #[tokio::main]
  lib.rs       # Library root, exports modules, contains run()
  config.rs    # CLI and environment configuration
  collector.rs # Sampling loop and vcgencmd integration
  system.rs    # /proc, /sys, and parser helpers
  state.rs     # Shared runtime state types
  models.rs    # API and internal data models
  web.rs       # HTTP routes and handlers
  dashboard.rs # Embedded HTML/CSS/JS dashboard
```

## Key Dependencies

- `axum` - Web framework
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `tower` - Service utilities
- `rustix` - Low-level system calls

## CI/CD and Releases

- The `ci.yml` GitHub Actions workflow runs formatting (`cargo fmt`), linting (`cargo clippy`), and tests (`cargo test`) on all PRs and pushes to `main`.
- The `release.yml` GitHub Actions workflow runs when a tag matching `v*` is pushed. It cross-compiles the application for `armv6`, `armv7`, and `aarch64` architectures (covering all Raspberry Pi models) and creates a GitHub Release with the resulting tarballs.

## Environment Variables

Configuration can be set via environment variables with `CLOAKYNODE_` prefix:

- `CLOAKYNODE_HOST`
- `CLOAKYNODE_PORT`
- `CLOAKYNODE_INTERVAL_SECONDS`
- `CLOAKYNODE_HISTORY_SIZE`
