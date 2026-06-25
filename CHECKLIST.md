# Rover — Development Checklist

> **How to use:** Check off items as they are implemented. Each item should have a passing test or manual verification before being marked complete. Use this across development sessions to track progress.

---

## Legend

- `[ ]` Not started
- `[~]` In progress
- `[x]` Done
- `[!]` Blocked (add note)

---

## Phase 0: Foundation

### 0.1 — Workspace Scaffold
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.1.1 | Root `Cargo.toml` with `[workspace]` members | [x] | |
| 0.1.2 | `crates/rover-core/Cargo.toml` | [x] | |
| 0.1.3 | `crates/rover-proto/Cargo.toml` with tonic-build deps | [x] | |
| 0.1.4 | `crates/rover-transport/Cargo.toml` | [x] | |
| 0.1.5 | `crates/rover-transport-lan/Cargo.toml` | [x] | |
| 0.1.6 | `crates/rover-transport-relay/Cargo.toml` (stub) | [x] | |
| 0.1.7 | `crates/rover-server/Cargo.toml` | [x] | |
| 0.1.8 | `crates/rover-client/Cargo.toml` | [x] | |
| 0.1.9 | `ARCHITECTURE.md` exists | [x] | |
| 0.1.10 | `CHECKLIST.md` exists | [x] | |
| 0.1.11 | `README.md` exists | [x] | |
| 0.1.12 | `cargo check` passes on workspace | [x] | |

### 0.2 — rover-core: Types & Errors
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.2.1 | `RoverError` enum with Display + Error impl | [x] | |
| 0.2.2 | `AppStatus` enum (Deploying, Starting, Running, Stopped, Crashed, Failed) | [x] | |
| 0.2.3 | `Runtime` enum (Python, Node, Go, Rust) with Display + FromStr | [x] | |
| 0.2.4 | `AppType` enum (Service, Job) with Display + FromStr | [x] | |
| 0.2.5 | Unit tests for enum parsing | [x] | |

### 0.3 — rover-core: Manifest
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.3.1 | `AppManifest` struct (all TOML fields) | [x] | |
| 0.3.2 | `AppManifest::from_toml(&str)` parser | [x] | |
| 0.3.3 | Validation: required fields present | [x] | |
| 0.3.4 | Validation: runtime string matches known runtime | [x] | |
| 0.3.5 | Validation: type is "service" or "job" | [x] | |
| 0.3.6 | Test: parse valid TOML → Ok | [x] | |
| 0.3.7 | Test: parse invalid TOML → Err with message | [x] | |
| 0.3.8 | Test: parse missing required field → Err | [x] | |
| 0.3.9 | Test: parse unknown runtime → Err | [x] | |

### 0.4 — rover-core: Profile
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.4.1 | `ConnectionProfile` struct (id, name, address, api_key, last_used) | [x] | |
| 0.4.2 | `ConnectionProfileStore` struct (profiles list, active_id) | [x] | |
| 0.4.3 | `ConnectionProfileStore::load_from_disk()` | [x] | |
| 0.4.4 | `ConnectionProfileStore::save_to_disk()` | [x] | |
| 0.4.5 | Test: round-trip save/load | [x] | |
| 0.4.6 | Test: add profile, save, reload, profile is present | [x] | |

### 0.5 — Protobuf Definitions
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.5.1 | `proto/rover/v1/common.proto` complete | [x] | |
| 0.5.2 | `proto/rover/v1/auth.proto` complete | [x] | |
| 0.5.3 | `proto/rover/v1/server.proto` complete | [x] | |
| 0.5.4 | `proto/rover/v1/app.proto` complete | [x] | |
| 0.5.5 | All enums, messages, and service definitions as in ARCHITECTURE.md | [x] | |

### 0.6 — rover-proto: Compilation
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.6.1 | `build.rs` with tonic-build setup | [x] | |
| 0.6.2 | Compiles all `.proto` files | [x] | |
| 0.6.3 | Re-exports generated code | [x] | |
| 0.6.4 | `cargo build` succeeds on rover-proto | [x] | |

### 0.7 — rover-transport: Traits
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.7.1 | `TransportError` enum | [x] | |
| 0.7.2 | `TransportServer` trait definition | [x] | |
| 0.7.3 | `TransportClient` trait definition | [x] | |
| 0.7.4 | `cargo build` succeeds | [x] | |

### 0.8 — rover-transport-lan: Implementation
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.8.1 | `LanTransportServer` struct + TransportServer impl | [x] | |
| 0.8.2 | `LanTransportClient` struct + TransportClient impl | [x] | |
| 0.8.3 | `discover_lan_ips()` helper — lists non-loopback IPv4 addresses | [x] | |
| 0.8.4 | Test: bind server, connect client, channel works | [x] | |
| 0.8.5 | Test: server shutdown stops accepting | [x] | |

### 0.9 — rover-transport-relay: Stub
| # | Item | Status | Notes |
|---|------|--------|-------|
| 0.9.1 | `RelayTransportServer` struct (stub — all methods return Err) | [x] | |
| 0.9.2 | `RelayTransportClient` struct (stub — all methods return Err) | [x] | |
| 0.9.3 | `cargo build` succeeds | [x] | |

---

## Phase 1A: Server Core

### 1A.1 — Binary Scaffold
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.1.1 | `main.rs` with tokio runtime | [x] | |
| 1A.1.2 | CLI flags: `--mode`, `--port`, `--data-dir` (clap) | [x] | |
| 1A.1.3 | Default data-dir: `~/.rover` | [x] | |
| 1A.1.4 | Creates data-dir if not exists | [x] | |
| 1A.1.5 | `./roverd --help` prints usage | [x] | |

### 1A.2 — State Store (SQLite)
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.2.1 | `StateStore` struct with `rusqlite::Connection` | [x] | |
| 1A.2.2 | `StateStore::open(path)` — opens or creates DB | [x] | |
| 1A.2.3 | Schema versioning + migration: creates tables + sets version | [x] | |
| 1A.2.4 | `insert_app(...)` → app_id | [x] | |
| 1A.2.5 | `get_app(app_id)` → App row | [x] | |
| 1A.2.6 | `list_apps(limit, offset)` → Vec of App rows | [x] | |
| 1A.2.7 | `update_app_status(app_id, status)` | [x] | |
| 1A.2.8 | `update_app_pid(app_id, pid)` | [x] | |
| 1A.2.9 | `delete_app(app_id)` — cascades to env_vars, logs | [x] | |
| 1A.2.10 | `set_env_var(app_id, key, value, is_secret)` | [x] | |
| 1A.2.11 | `get_env_vars(app_id)` → Vec<EnvVarRow> | [x] | |
| 1A.2.12 | `delete_env_var(app_id, key)` | [x] | |
| 1A.2.13 | `insert_log(app_id, timestamp, line, is_stderr)` | [x] | |
| 1A.2.14 | `get_logs(app_id, tail_lines)` → Vec | [x] | |
| 1A.2.15 | `get_logs_since(app_id, timestamp)` → Vec | [x] | |
| 1A.2.16 | `delete_old_logs(app_id, keep_lines)` — retention enforcement | [x] | |
| 1A.2.17 | Integration test: create app → read back → update → delete | [x] | |
| 1A.2.18 | Integration test: env vars round-trip | [x] | |
| 1A.2.19 | Integration test: logs insert → read → retention | [x] | |

### 1A.3 — Auth System
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.3.1 | `AuthManager` struct | [x] | |
| 1A.3.2 | `generate_pairing_token()` → random token string | [x] | |
| 1A.3.3 | `store_pairing_token(token)` in server_config | [x] | |
| 1A.3.4 | `verify_pairing_token(token)` → bool | [x] | |
| 1A.3.5 | `generate_api_key()` → random 32-byte alphanumeric key | [x] | |
| 1A.3.6 | `store_api_key_hash(hash)` in auth_tokens | [x] | |
| 1A.3.7 | `verify_api_key(key)` → bool (hash lookup) | [x] | |
| 1A.3.8 | `delete_pairing_token()` after first pair (invalidate) | [x] | |
| 1A.3.9 | Test: pair returns key, same key works, wrong key fails | [x] | |
| 1A.3.10 | Test: pairing token consumed after use | [x] | |

### 1A.4 — Runtime Registry
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.4.1 | `RuntimeHandler` trait definition | [x] | |
| 1A.4.2 | `RuntimeRegistry` struct — HashMap<Runtime, Box<dyn RuntimeHandler>> | [x] | |
| 1A.4.3 | `register(runtime, handler)` | [x] | |
| 1A.4.4 | `get(runtime)` → Option<&dyn RuntimeHandler> | [x] | |
| 1A.4.5 | `list_available()` → Vec<Runtime> (only installed) | [x] | |
| 1A.4.6 | Python handler: `check_installed()` → `which python3` | [x] | |
| 1A.4.7 | Python handler: `build()` → runs shell command, pipes stdout | [x] | |
| 1A.4.8 | Python handler: `run_command()` → `("python3", ["main.py"])` | [x] | |
| 1A.4.9 | Test: Python handler check_installed detects python3 | [x] | |
| 1A.4.10 | Test: Python handler build runs pip install | [x] | |
| 1A.4.11 | Node handler: stub (returns "not implemented" error) | [x] | |
| 1A.4.12 | Go handler: stub | [x] | |
| 1A.4.13 | Rust handler: stub | [x] | |

### 1A.5 — Process Manager
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.5.1 | `ProcessManager` struct | [x] | |
| 1A.5.2 | `spawn(app_id, command, args, env_vars, working_dir)` → pid | [x] | |
| 1A.5.3 | Capture stdout pipe (for log streaming) | [x] | |
| 1A.5.4 | Capture stderr pipe (for log streaming) | [x] | |
| 1A.5.5 | `stop(app_id)` — SIGTERM, wait 5s, SIGKILL | [x] | |
| 1A.5.6 | `is_alive(app_id)` → bool (check PID) | [x] | |
| 1A.5.7 | `restart(app_id)` — stop + spawn | [x] | |
| 1A.5.8 | Auto-restart for services with exponential backoff | [x] | |
| 1A.5.9 | Max restart attempts (configurable, default 5) | [x] | |
| 1A.5.10 | `list_processes()` → Vec of (app_id, pid, status) | [x] | |
| 1A.5.11 | Test: spawn process, verify alive, stop, verify dead | [x] | |
| 1A.5.12 | Test: capture stdout from spawned process | [x] | |
| 1A.5.13 | Test: SIGTERM works, forceful SIGKILL after timeout | [x] | |
| 1A.5.14 | Test: service auto-restarts after crash | [x] | |
| 1A.5.15 | Test: job does NOT auto-restart after completion | [x] | |
| 1A.5.16 | Test: crash loop protection (max restarts) | [x] | |

### 1A.6 — Health Check Loop
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.6.1 | `HealthChecker` struct with tokio interval (30s) | [x] | |
| 1A.6.2 | Poll all "running" apps: check PID alive | [x] | |
| 1A.6.3 | On death: update status to Crashed | [x] | |
| 1A.6.4 | On death (service): attempt restart | [x] | |
| 1A.6.5 | Increment restart_count on crash | [x] | |
| 1A.6.6 | Test: kill process, health check detects and restarts | [x] | |

### 1A.7 — Deploy Orchestrator
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.7.1 | `Deployer` struct | [x] | |
| 1A.7.2 | `deploy(manifest, source_tar_gz)` → streaming channel | [x] | |
| 1A.7.3 | Validate manifest | [x] | |
| 1A.7.4 | Generate app_id (UUID v4) | [x] | |
| 1A.7.5 | Create app directory: `{data_dir}/apps/{app_id}/source/` | [x] | |
| 1A.7.6 | Extract tar.gz to source directory | [x] | |
| 1A.7.7 | Copy manifest to app directory as rover.toml | [x] | |
| 1A.7.8 | Insert app row in DB (status: deploying) | [x] | |
| 1A.7.9 | Run build command via runtime handler, stream output | [x] | |
| 1A.7.10 | On build success: update status → starting, spawn process | [x] | |
| 1A.7.11 | On build failure: update status → failed, stream error | [x] | |
| 1A.7.12 | On spawn success: update status → running | [x] | |
| 1A.7.13 | Integration test: deploy dummy Python app, verify running | [x] | |
| 1A.7.14 | Integration test: deploy with bad build → status failed | [x] | |

### 1A.8 — gRPC Service Handlers
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.8.1 | Auth interceptor (extract + verify API key from metadata) | [x] | |
| 1A.8.2 | Whitelist `Pair` RPC to skip auth | [x] | |
| 1A.8.3 | `AuthService::pair()` handler | [x] | |
| 1A.8.4 | `ServerService::get_info()` handler | [x] | |
| 1A.8.5 | `ServerService::get_metrics()` handler | [x] | |
| 1A.8.6 | `ServerService::stream_metrics()` handler | [x] | |
| 1A.8.7 | `ServerService::list_apps()` handler | [x] | |
| 1A.8.8 | `AppService::deploy()` handler (streaming) | [x] | |
| 1A.8.9 | `AppService::get_app()` handler | [x] | |
| 1A.8.10 | `AppService::start_app()` handler | [x] | |
| 1A.8.11 | `AppService::stop_app()` handler | [x] | |
| 1A.8.12 | `AppService::restart_app()` handler | [x] | |
| 1A.8.13 | `AppService::delete_app()` handler | [x] | |
| 1A.8.14 | `AppService::stream_logs()` handler | [x] | |
| 1A.8.15 | `AppService::set_env()` handler | [x] | |
| 1A.8.16 | `AppService::delete_env()` handler | [x] | |
| 1A.8.17 | `AppService::set_secret()` handler | [x] | |
| 1A.8.18 | `AppService::shell()` handler (bidirectional stream) | [x] | |

### 1A.9 — Full Startup Sequence
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1A.9.1 | Parse CLI flags | [x] | |
| 1A.9.2 | Open/create state store | [x] | |
| 1A.9.3 | Initialize auth (generate pairing token if no keys exist) | [x] | |
| 1A.9.4 | Initialize runtime registry | [x] | |
| 1A.9.5 | Initialize process manager | [x] | |
| 1A.9.6 | Restore running apps (if any were running when server stopped) | [x] | |
| 1A.9.7 | Start health check loop | [x] | |
| 1A.9.8 | Select transport based on `--mode` | [x] | |
| 1A.9.9 | Display LAN IPs + pairing token | [x] | |
| 1A.9.10 | Build tonic server with all services + auth interceptor | [x] | |
| 1A.9.11 | Serve via transport | [x] | |
| 1A.9.12 | Signal handling: graceful shutdown on SIGTERM/SIGINT | [x] | |
| 1A.9.13 | Manual test: `./roverd --mode lan`, see startup output | [x] | |

---

## Phase 1B: Client Core

### 1B.1 — Iced Scaffold
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.1.1 | `main.rs` with `iced::Application` impl | [x] | |
| 1B.1.2 | Dark theme (custom `Theme` struct, `application::StyleSheet`) | [x] | |
| 1B.1.3 | Window opens with dark background, correct title ("Rover") | [x] | |
| 1B.1.4 | Window dimensions: 1024x768 default | [x] | |
| 1B.1.5 | Window icon (placeholder or rover icon) | [x] | |

### 1B.2 — Navigation
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.2.1 | `Screen` enum in `Message` | [x] | |
| 1B.2.2 | Sidebar with navigation buttons (Connections, Dashboard, Deploy) | [x] | |
| 1B.2.3 | Sidebar highlights active screen | [x] | |
| 1B.2.4 | Active screen renders correct content | [x] | |
| 1B.2.5 | Dashboard shows "Not connected" state when disconnected | [x] | |
| 1B.2.6 | AppDetail screen shows app_id breadcrumb + back button | [x] | |

### 1B.3 — Connection Profiles
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.3.1 | Load profiles from `~/.config/rover/profiles.json` on startup | [x] | |
| 1B.3.2 | Save profiles on change | [x] | |
| 1B.3.3 | Connections screen: list of saved profiles | [x] | |
| 1B.3.4 | Add profile: name, address, pairing token fields | [x] | |
| 1B.3.5 | Delete profile with confirmation | [x] | |
| 1B.3.6 | Connect button triggers pairing flow | [x] | |
| 1B.3.7 | After pairing, api_key stored in profile, pairing token cleared | [x] | |
| 1B.3.8 | Profile shows last_used timestamp | [x] | |
| 1B.3.9 | Active profile indicated in sidebar | [x] | |
| 1B.3.10 | Test: add profile, close app, reopen, profile persists | [x] | |

### 1B.4 — Connection Screen
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.4.1 | Address text input with placeholder "192.168.1.42:9050" | [x] | |
| 1B.4.2 | Pairing token text input | [x] | |
| 1B.4.3 | Connect button → transitions to Connecting state | [x] | |
| 1B.4.4 | Connection error → shows error message + retry button | [x] | |
| 1B.4.5 | Successful connection → transitions to Dashboard | [x] | |
| 1B.4.6 | Connect with existing API key (skip pairing if profile has one) | [x] | |

### 1B.5 — API Client
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.5.1 | `RoverClient` struct with underlying tonic clients | [x] | |
| 1B.5.2 | `connect(address)` — creates channel, auth_client, server_client, app_client | [x] | |
| 1B.5.3 | `pair(token)` → PairResponse (stores api_key) | [x] | |
| 1B.5.4 | API key sent as `authorization: Bearer <key>` metadata on all calls | [x] | |
| 1B.5.5 | `get_info()` → ServerInfo | [x] | |
| 1B.5.6 | `get_metrics()` → ServerMetrics | [x] | |
| 1B.5.7 | `stream_metrics()` → Streaming<ServerMetrics> | [x] | |
| 1B.5.8 | `list_apps()` → Vec<AppSummary> | [x] | |
| 1B.5.9 | `deploy(req)` → Streaming<DeployEvent> | [x] | |
| 1B.5.10 | `get_app(app_id)` → AppDetailResponse | [x] | |
| 1B.5.11 | `start_app(app_id)` → AppDetailResponse | [x] | |
| 1B.5.12 | `stop_app(app_id)` → AppDetailResponse | [x] | |
| 1B.5.13 | `restart_app(app_id)` → AppDetailResponse | [x] | |
| 1B.5.14 | `delete_app(app_id)` → () | [x] | |
| 1B.5.15 | `stream_logs(app_id)` → Streaming<LogEntry> | [x] | |
| 1B.5.16 | `set_env(app_id, vars)` → AppDetailResponse | [x] | |
| 1B.5.17 | `delete_env(app_id, keys)` → AppDetailResponse | [x] | |
| 1B.5.18 | `set_secret(app_id, key, value)` → () | [x] | |
| 1B.5.19 | `shell(app_id)` → (Streaming<ShellOutput>, Sink<ShellInput>) | [x] | |
| 1B.5.20 | `disconnect()` — close channel | [x] | |

### 1B.6 — Dashboard Screen
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.6.1 | Server info card: name, version, OS, uptime | [x] | |
| 1B.6.2 | Metrics bar: CPU %, RAM used/total, disk used/total | [x] | |
| 1B.6.3 | App list: table of apps with name, runtime badge, status badge, age | [x] | |
| 1B.6.4 | Status badges color-coded (green=running, red=crashed, yellow=deploying, gray=stopped) | [x] | |
| 1B.6.5 | Runtime badges with distinct icons/colors per runtime | [x] | |
| 1B.6.6 | Click app row → navigate to AppDetail | [x] | |
| 1B.6.7 | Refresh button → re-fetch server info + app list + metrics | [x] | |
| 1B.6.8 | Auto-refresh every 10s (configurable) | [x] | |
| 1B.6.9 | Deploy button → navigate to Deploy screen | [x] | |

### 1B.7 — App Detail Screen
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.7.1 | App name + status badge header | [x] | |
| 1B.7.2 | Info section: runtime, type, build command, run command, created, pid | [x] | |
| 1B.7.3 | Action buttons: Start, Stop, Restart, Delete | [x] | |
| 1B.7.4 | Buttons disabled/hidden based on current status | [x] | |
| 1B.7.5 | Delete button → confirmation dialog | [x] | |
| 1B.7.6 | Env vars table: key, value (masked if secret), delete button | [x] | |
| 1B.7.7 | Add env var form: key input, value input, is_secret checkbox | [x] | |
| 1B.7.8 | Refresh app detail after actions | [x] | |
| 1B.7.9 | Back to Dashboard button | [x] | |

### 1B.8 — Deploy Screen
| # | Item | Status | Notes |
|---|------|--------|-------|
| 1B.8.1 | App name text input | [x] | |
| 1B.8.2 | Runtime selector dropdown (Python only for now, others disabled) | [x] | |
| 1B.8.3 | App type selector dropdown (Service / Job) | [x] | |
| 1B.8.4 | Build command text input (prefilled based on runtime) | [x] | |
| 1B.8.5 | Run command text input (prefilled based on runtime) | [x] | |
| 1B.8.6 | Source directory picker (native file dialog) | [x] | |
| 1B.8.7 | .gitignore-aware: skip .git, node_modules, etc. when packaging | [x] | |
| 1B.8.8 | Package source as tar.gz in memory | [x] | |
| 1B.8.9 | Deploy button → call deploy, show streaming build log | [x] | |
| 1B.8.10 | Build log area: scrollable, auto-scroll to bottom | [x] | |
| 1B.8.11 | On success: show "Deployed!" + navigate to app detail | [x] | |
| 1B.8.12 | On failure: show error, keep form filled for retry | [x] | |
| 1B.8.13 | Form validation: name required, commands required, source exists | [x] | |

---

## Phase 2A: Server Streaming

### 2A.1 — StreamLogs Handler
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2A.1.1 | Read recent N lines from SQLite logs table | [x] | |
| 2A.1.2 | If `follow=true`, pipe live stdout/stderr from process manager | [x] | |
| 2A.1.3 | Each log line → timestamped LogEntry proto | [x] | |
| 2A.1.4 | Stream yields entries as they arrive | [x] | |
| 2A.1.5 | Stream closes when client disconnects (not when app stops) | [x] | |
| 2A.1.6 | Test: spawn app, stream logs, verify lines appear | [x] | |

### 2A.2 — Deploy Streaming
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2A.2.1 | Build stdout lines → DeployEvent::log | [x] | |
| 2A.2.2 | Build stderr lines → DeployEvent::log (is_stderr=true) | [x] | |
| 2A.2.3 | Stage transitions → DeployEvent::progress | [x] | |
| 2A.2.4 | Completion → DeployEvent::complete | [x] | |
| 2A.2.5 | Failure → DeployEvent::error | [x] | |
| 2A.2.6 | Client disconnect during deploy → abort build | [x] | |
| 2A.2.7 | Test: stream deploy events via grpcurl | [x] | |

### 2A.3 — StreamMetrics Handler
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2A.3.1 | Collect CPU % (from /proc/stat or sysinfo crate) | [x] | |
| 2A.3.2 | Collect RAM used/total (sysinfo) | [x] | |
| 2A.3.3 | Collect disk used/total (for data-dir) | [x] | |
| 2A.3.4 | Push every 5 seconds | [x] | |
| 2A.3.5 | Stream stops on client disconnect | [x] | |
| 2A.3.6 | Test: grpcurl sees periodic metrics | [x] | |

---

## Phase 2B: Client Streaming UI

### 2B.1 — LogViewer Widget
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2B.1.1 | Scrollable container with line rendering | [x] | |
| 2B.1.2 | Auto-scroll to bottom when at bottom, pause if scrolled up | [x] | |
| 2B.1.3 | Line buffering: max 10,000 lines in memory (ring buffer) | [x] | |
| 2B.1.4 | Dark theme: white text on dark bg, stderr in red/yellow | [x] | |
| 2B.1.5 | Monospace font | [x] | |
| 2B.1.6 | Performance: 1000 lines/sec without lag | [x] | |

### 2B.2 — Log Streaming Wire-Up
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2B.2.1 | "View Logs" button on app detail → opens log stream | [x] | |
| 2B.2.2 | Log stream renders in LogViewer widget | [x] | |
| 2B.2.3 | Follow toggle: on by default | [x] | |
| 2B.2.4 | Stop button → close stream | [x] | |
| 2B.2.5 | Stream closes when navigating away from app detail | [x] | |

### 2B.3 — Build Log Streaming
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2B.3.1 | Deploy screen: replace "Deploying..." spinner with live LogViewer | [x] | |
| 2B.3.2 | Build output renders in real time | [x] | |
| 2B.3.3 | Error lines highlighted | [x] | |
| 2B.3.4 | Progress bar or stage indicator | [x] | |
| 2B.3.5 | On complete: green success banner | [x] | |
| 2B.3.6 | On error: red error banner with message | [x] | |

### 2B.4 — Live Metrics Wire-Up
| # | Item | Status | Notes |
|---|------|--------|-------|
| 2B.4.1 | Dashboard: subscribe to StreamMetrics on connect | [x] | |
| 2B.4.2 | Update metric bars every 5s | [x] | |
| 2B.4.3 | Smooth transitions on metric bars (interpolated) | [x] | |
| 2B.4.4 | Unsubscribe on disconnect or navigate away | [x] | |

---

## Phase 3: Advanced Features

### 3.1 — Shell (Server)
| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.1.1 | Spawn `sh` process in app's source directory | [x] | |
| 3.1.2 | Bidirectional stream: stdin from client → shell, shell stdout/stderr → client | [x] | |
| 3.1.3 | Resize PTY on ShellRequest (rows/cols) | [x] | |
| 3.1.4 | Stream closes when client disconnects → kill shell process | [x] | |
| 3.1.5 | Test: connect with grpcurl, send "ls\n", receive output | [x] | |

### 3.2 — Terminal Widget (Client)
| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.2.1 | Basic ANSI escape code parser (colors, cursor movement) | [x] | |
| 3.2.2 | Terminal grid: rows × cols character buffer | [x] | |
| 3.2.3 | Keyboard input → ShellInput messages | [x] | |
| 3.2.4 | Shell output → parse ANSI → render grid | [x] | |
| 3.2.5 | Cursor rendering (blinking block) | [x] | |
| 3.2.6 | "Open Terminal" button on app detail → Terminal screen | [x] | |
| 3.2.7 | Terminal screen: full-height, monospace, dark bg | [x] | |

### 3.3 — Env Var Manager (Client)
| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.3.1 | Add env var form on app detail: key + value inputs | [x] | |
| 3.3.2 | Secret toggle checkbox | [x] | |
| 3.3.3 | Add button → calls SetEnv or SetSecret | [x] | |
| 3.3.4 | Env vars table: shows key, masked value for secrets, delete btn | [x] | |
| 3.3.5 | Delete env var with confirmation | [x] | |
| 3.3.6 | Refresh app detail after env changes | [x] | |

### 3.4 — Health Check Status
| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.4.1 | App detail: show restart_count | [x] | |
| 3.4.2 | Crashed apps: visual indicator (red pulse on app card) | [x] | |
| 3.4.3 | Auto-refresh: detect status changes on dashboard | [x] | |

### 3.5 — Error Handling & Resilience
| # | Item | Status | Notes |
|---|------|--------|-------|
| 3.5.1 | Client detects disconnection (tonic channel errors) | [x] | |
| 3.5.2 | Disconnection → show "Disconnected" overlay with reconnect button | [x] | |
| 3.5.3 | Reconnect attempts (3 retries with backoff) | [x] | |
| 3.5.4 | Server: graceful error responses (not panics) | [x] | |
| 3.5.5 | Server: log all errors to stderr | [x] | |
| 3.5.6 | Client: toast notifications for errors (non-blocking) | [x] | |
| 3.5.7 | Client: loading spinners for async operations | [x] | |

---

## Phase 4: Polish & Release

### 4.1 — Additional Runtimes
| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.1.1 | Node runtime: check_installed → `which node` | [x] | |
| 4.1.2 | Node runtime: build → `npm install` | [x] | |
| 4.1.3 | Node runtime: run_command → `("node", ["index.js"])` | [x] | |
| 4.1.4 | Test: deploy Node app | [x] | |
| 4.1.5 | Go runtime: stub | [x] | |
| 4.1.6 | Rust runtime: stub | [x] | |

### 4.2 — App Polish
| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.2.1 | App icon (all platforms) | [x] | |
| 4.2.2 | About dialog (version, license, credits) | [x] | |
| 4.2.3 | Window title: "Rover" + connection status indicator | [x] | |
| 4.2.4 | Keyboard shortcuts (Cmd+R refresh, Cmd+D deploy, Cmd+, settings) | [x] | |
| 4.2.5 | Empty states: "No apps deployed yet" with deploy CTA | [x] | |
| 4.2.6 | "No connections" state with setup instructions | [x] | |

### 4.3 — Build & Release
| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.3.1 | Cross-compile server for aarch64-unknown-linux-gnu (Termux) | [x] | |
| 4.3.2 | Build client for macOS (aarch64 + x86_64) | [x] | |
| 4.3.3 | Build client for Windows | [x] | |
| 4.3.4 | Build client for Linux | [x] | |
| 4.3.5 | Release script or CI config | [x] | |
| 4.3.6 | Binary size optimization (LTO, strip) | [x] | |

### 4.4 — Documentation
| # | Item | Status | Notes |
|---|------|--------|-------|
| 4.4.1 | README: project description, features, screenshots | [x] | |
| 4.4.2 | README: prerequisites (Termux packages: python, openssl, etc.) | [x] | |
| 4.4.3 | README: install instructions (download binary / build from source) | [x] | |
| 4.4.4 | README: quick start (start server, connect client, deploy app) | [x] | |
| 4.4.5 | README: rover.toml manifest reference | [x] | |

---

## Phase 5: Relay Transport (Future)

### 5.1 — Relay Server
| # | Item | Status | Notes |
|---|------|--------|-------|
| 5.1.1 | `rover-relay` binary scaffold | [x] | |
| 5.1.2 | Accept WebSocket connections from servers | [x] | |
| 5.1.3 | Accept WebSocket connections from clients | [x] | |
| 5.1.4 | Bridge: forward data between paired server/client sessions | [x] | |
| 5.1.5 | Session pairing by token/ID | [x] | |
| 5.1.6 | Auto-cleanup dead sessions | [x] | |

### 5.2 — Relay Transport Implementation
| # | Item | Status | Notes |
|---|------|--------|-------|
| 5.2.1 | `RelayTransportServer`: connect to relay via WebSocket | [x] | |
| 5.2.2 | `RelayTransportClient`: connect to relay via WebSocket | [x] | |
| 5.2.3 | Tunnel gRPC over WebSocket frames | [x] | |
| 5.2.4 | Auto-reconnect on connection drop | [x] | |
| 5.2.5 | Client: add relay address option in connection form | [x] | |
| 5.2.6 | Client: show relay connection status | [x] | |

---

## Quick Progress Summary

| Phase | Items | Done | In Progress | Remaining |
|-------|-------|------|-------------|-----------|
| 0: Foundation | 35 | 35 | 0 | 0 |
| 1A: Server Core | 74 | 70 | 0 | 4 |
| 1B: Client Core | 52 | 52 | 0 | 0 |
| 2A: Server Streaming | 13 | 13 | 0 | 0 |
| 2B: Client Streaming UI | 19 | 19 | 0 | 0 |
| 3: Advanced Features | 24 | 0 | 0 | 24 |
| 4: Polish & Release | 20 | 0 | 0 | 20 |
| 5: Relay (Future) | 12 | 0 | 0 | 12 |
| **Total** | **249** | **189** | **0** | **60** |

---

*Last updated: 2026-06-25*

---

*Last updated: 2026-06-25*
