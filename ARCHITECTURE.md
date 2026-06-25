# Rover — Architecture Document

> **Status:** Living document. Update as design decisions change.
> **Target audience:** Development agents working on this project across sessions.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [System Architecture](#2-system-architecture)
3. [Crate Layout](#3-crate-layout)
4. [Protobuf Schema](#4-protobuf-schema)
5. [Server Architecture](#5-server-architecture)
6. [Client Architecture](#6-client-architecture)
7. [Transport Abstraction](#7-transport-abstraction)
8. [Auth Flow](#8-auth-flow)
9. [App Lifecycle & Runtime System](#9-app-lifecycle--runtime-system)
10. [State Persistence](#10-state-persistence)
11. [Network & Connectivity](#11-network--connectivity)
12. [Deployment Manifest](#12-deployment-manifest)
13. [Development Roadmap](#13-development-roadmap)
14. [Crate Dependency Graph](#14-crate-dependency-graph)
15. [Key Technical Decisions](#15-key-technical-decisions)

---

## 1. Project Overview

### Purpose

Rover turns an Android phone (running Termux) into a tiny Platform-as-a-Service. A cross-platform desktop application (macOS/Windows/Linux) acts as the management frontend. The system allows deploying, managing, and observing applications on one or more phone "servers."

### Core Principles

- **Self-hosted.** No third-party PaaS dependency. The phone IS the server.
- **Extensible.** Runtimes (Python, Node, Go, Rust) are pluggable. Transport (LAN, relay) is swappable.
- **Simple.** No containers. No Kubernetes. Apps are child processes. One binary on the phone.
- **Polished.** The desktop client should feel like a real product, not a dev tool.

### Technical Pillars

| Pillar | Choice |
|--------|--------|
| Language | Rust (edition 2024, latest stable) |
| Async Runtime | tokio |
| RPC Framework | gRPC via tonic |
| Schema | Protocol Buffers (proto3) |
| GUI Framework | Iced (pure Rust, Elm-like architecture) |
| Server State | SQLite via rusqlite |
| Serialization | serde + serde_json (configs, manifests) |
| Build System | Cargo workspace |

---

## 2. System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     DESKTOP CLIENT (Iced)                     │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌──────────────────┐  │
│  │Dashboard│ │App Manager│ │Log Viewer│ │ Connection Mgr  │  │
│  │ (overview)│ │(deploy,  │ │(stream, │ │(profiles, auth) │  │
│  │         │ │ start,stop)│ │ filter) │ │                  │  │
│  └─────────┘ └──────────┘ └─────────┘ └──────────────────┘  │
│                         │                                     │
│                  ┌──────┴──────┐                              │
│                  │ gRPC Client │  (tonic)                     │
│                  └──────┬──────┘                              │
└─────────────────────────┼────────────────────────────────────┘
                          │
              ┌───────────┴───────────┐
              │   Transport Layer     │
              │   (LAN TCP | Relay)   │
              └───────────┬───────────┘
                          │
┌─────────────────────────┼────────────────────────────────────┐
│           ANDROID PHONE (Termux)                              │
│                  ┌──────┴──────┐                              │
│                  │ gRPC Server │  (tonic)                     │
│                  └──────┬──────┘                              │
│  ┌──────────────────────┼──────────────────────────────────┐ │
│  │              rover-server (roverd)                       │ │
│  │                                                         │ │
│  │  ┌───────────┐ ┌────────────┐ ┌──────────────────────┐ │ │
│  │  │App Runtime│ │State Store │ │  Transport Provider  │ │ │
│  │  │ Manager   │ │ (SQLite)   │ │  (LAN/Relay)         │ │ │
│  │  └───────────┘ └────────────┘ └──────────────────────┘ │ │
│  │         │                                               │ │
│  │  ┌──────┴──────┐                                        │ │
│  │  │  App Proc 1 │  python main.py                        │ │
│  │  │  App Proc 2 │  node server.js                        │ │
│  │  │  App Proc N │  ./my-binary                           │ │
│  │  └─────────────┘                                        │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### Multiple Servers

The desktop client manages **independent** servers (not a logical cluster). Each phone runs its own `roverd` instance. The client stores multiple connection profiles and the user switches between them. There is no cross-device orchestration or scheduling.

---

## 3. Crate Layout

```
rover/                          ← Git repository root
│
├── Cargo.toml                  ← Workspace definition
├── ARCHITECTURE.md             ← This file
├── CHECKLIST.md                ← Feature/implementation checklist
├── README.md                   ← Project README
│
├── proto/                      ← Protobuf definitions (single source of truth)
│   └── rover/
│       └── v1/
│           ├── server.proto    ← Server management RPCs
│           ├── app.proto       ← App deployment & lifecycle RPCs
│           ├── auth.proto      ← Authentication RPCs
│           └── common.proto    ← Shared message types
│
├── crates/
│   ├── rover-proto/            ← Proto compilation + generated code re-export
│   │   ├── Cargo.toml
│   │   └── build.rs            ← tonic-build compiles proto/ → src/
│   │
│   ├── rover-core/             ← Shared types, traits, errors (NO I/O)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manifest.rs     ← AppManifest (TOML parsing)
│   │       ├── app_status.rs   ← AppStatus enum (Running, Stopped, Crashed...)
│   │       ├── runtime.rs      ← Runtime enum (Python, Node, Go, Rust)
│   │       ├── profile.rs      ← ConnectionProfile ( serialized client-side)
│   │       └── error.rs        ← RoverError enum (shared error types)
│   │
│   ├── rover-transport/        ← Transport abstraction trait
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          ← trait TransportServer, trait TransportClient
│   │
│   ├── rover-transport-lan/    ← V1: Raw TCP transport
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          ← LanTransportServer, LanTransportClient
│   │
│   ├── rover-transport-relay/  ← V2: Relay transport (STUB)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs          ← RelayTransport (stubbed, panics with "not implemented")
│   │
│   ├── rover-server/           ← Server binary (roverd)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs         ← Entrypoint: parse flags, start server
│   │       ├── server.rs       ← gRPC server setup
│   │       ├── auth.rs         ← Pairing token logic + API key management
│   │       ├── state.rs        ← SQLite state store (apps, env vars, tokens)
│   │       ├── runtime/        ← Runtime system
│   │       │   ├── mod.rs      ← Runtime trait + registry
│   │       │   ├── python.rs   ← Python runtime implementation
│   │       │   ├── node.rs     ← Node.js runtime (stub)
│   │       │   ├── go.rs       ← Go runtime (stub)
│   │       │   └── rust.rs     ← Rust runtime (stub)
│   │       ├── process.rs      ← Process manager (spawn/kill/monitor)
│   │       ├── deploy.rs       ← Deploy orchestration (build → health check → live)
│   │       └── health.rs       ← Health check loop
│   │
│   └── rover-client/           ← Desktop GUI binary (rover)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs         ← Entrypoint: iced::Application
│           ├── app.rs          ← Top-level App struct (Iced Application impl)
│           ├── theme.rs        ← Dark theme definition
│           ├── message.rs      ← Message enum (all UI events + async responses)
│           ├── state/          ← Application state
│           │   ├── mod.rs
│           │   ├── connection.rs ← Current connection state (disconnected/connected)
│           │   └── profiles.rs   ← Saved connection profiles
│           ├── api/            ← gRPC client layer
│           │   ├── mod.rs
│           │   └── client.rs   ← RoverClient (wraps tonic gRPC client)
│           ├── screens/        ← UI screens (one per "page")
│           │   ├── mod.rs
│           │   ├── dashboard.rs    ← Server overview, metrics, app list
│           │   ├── app_detail.rs   ← Single app view (logs, env vars, controls)
│           │   ├── deploy.rs       ← New deployment form + build log stream
│           │   ├── connections.rs  ← Connection profile manager
│           │   └── terminal.rs     ← Embedded shell (gRPC stream)
│           └── widgets/        ← Reusable UI components
│               ├── mod.rs
│               ├── log_viewer.rs   ← Scrollable, real-time log tail
│               ├── app_card.rs     ← App summary card (status, name, runtime)
│               └── metric_bar.rs   ← CPU/RAM/disk usage bars
```

### Dependency Flow (who depends on who)

```
rover-proto ──────────────────────────────────────────────┐
(no deps except tonic-build)                              │
                                                          │
rover-core ───────────────────────────────────────────────┤
(depends on: nothing or serde/toml only)                  │
                                                          │
rover-transport ──────────────────────────────────────────┤
(depends on: rover-core for error types)                  │
                                                          │
rover-transport-lan ──────────────────────────────────────┤
(depends on: rover-transport)                             │
                                                          │
rover-transport-relay ────────────────────────────────────┤
(depends on: rover-transport)                             │
                                                          │
rover-server ─────────────────────────────────────────────┤
(depends on: rover-proto, rover-core, rover-transport,    │
             rover-transport-lan, rover-transport-relay)  │
                                                          │
rover-client ─────────────────────────────────────────────┘
(depends on: rover-proto, rover-core, rover-transport,
             rover-transport-lan, rover-transport-relay)
```

**`rover-core` has NO dependency on `rover-proto`**. This is deliberate: `rover-core` contains plain Rust types. The server/client map between proto types and core types at the gRPC boundary. This keeps `rover-core` testable without compiling protobufs.

---

## 4. Protobuf Schema

### Directory: `proto/rover/v1/`

#### `common.proto`

```protobuf
syntax = "proto3";
package rover.v1;

// Empty message for RPCs that need no input
message Empty {}

// Pagination for list endpoints
message PageRequest {
  int32 limit = 1;   // max results (default 50)
  int32 offset = 2;  // 0-based
}

message PageResponse {
  int32 total = 1;
  int32 limit = 2;
  int32 offset = 3;
}

// Timestamp in Unix milliseconds
message Timestamp {
  int64 millis = 1;
}

// Server resource metrics
message ServerMetrics {
  double cpu_percent = 1;
  uint64 ram_used_bytes = 2;
  uint64 ram_total_bytes = 3;
  uint64 disk_used_bytes = 4;
  uint64 disk_total_bytes = 5;
  // Future: network_rx_bytes, network_tx_bytes, battery_percent, temperature
}

// Runtime identifier
enum Runtime {
  RUNTIME_UNSPECIFIED = 0;
  RUNTIME_PYTHON = 1;
  RUNTIME_NODE = 2;
  RUNTIME_GO = 3;
  RUNTIME_RUST = 4;
}

// App type
enum AppType {
  APP_TYPE_UNSPECIFIED = 0;
  APP_TYPE_SERVICE = 1;   // long-running, auto-restart on crash
  APP_TYPE_JOB = 2;       // one-shot, runs to completion
}

// App status
enum AppStatus {
  APP_STATUS_UNSPECIFIED = 0;
  APP_STATUS_DEPLOYING = 1;     // Build in progress
  APP_STATUS_STARTING = 2;      // Process starting
  APP_STATUS_RUNNING = 3;       // Healthy
  APP_STATUS_STOPPED = 4;       // Explicitly stopped
  APP_STATUS_CRASHED = 5;       // Exited unexpectedly
  APP_STATUS_FAILED = 6;        // Build or start failed
}
```

#### `auth.proto`

```protobuf
syntax = "proto3";
package rover.v1;

import "rover/v1/common.proto";

// --- Pairing (first-time setup) ---

message PairRequest {
  string pairing_token = 1;  // The token printed by the server on startup
}

message PairResponse {
  string api_key = 1;        // Persist this for future connections
  string server_name = 2;    // Human-readable (e.g., hostname)
  string server_version = 3; // roverd version
}

// --- Authenticated RPCs ---
// The api_key is sent as gRPC metadata: "authorization: Bearer <api_key>"
// If the key is invalid, the server returns UNAUTHENTICATED.

service AuthService {
  // Exchange a one-time pairing token for a persistent API key.
  // This is the ONLY unauthenticated RPC.
  rpc Pair(PairRequest) returns (PairResponse);
}
```

#### `server.proto`

```protobuf
syntax = "proto3";
package rover.v1;

import "rover/v1/common.proto";

// --- Server info ---

message ServerInfo {
  string name = 1;
  string version = 2;
  string os = 3;            // e.g., "Android 14 (aarch64)"
  string hostname = 4;
  uint32 uptime_seconds = 5;
}

message GetInfoRequest {}
message GetMetricsRequest {}
message GetAppListRequest {
  PageRequest page = 1;
}

// --- App summary (in list views) ---

message AppSummary {
  string app_id = 1;
  string name = 2;
  Runtime runtime = 3;
  AppType app_type = 4;
  AppStatus status = 5;
  Timestamp created_at = 6;
  Timestamp updated_at = 7;
}

message AppListResponse {
  repeated AppSummary apps = 1;
  PageResponse page = 2;
}

// --- Server management ---

service ServerService {
  // Get server identity and version
  rpc GetInfo(GetInfoRequest) returns (ServerInfo);

  // Get live resource metrics
  rpc GetMetrics(GetMetricsRequest) returns (ServerMetrics);

  // Stream live metrics (push every N seconds)
  rpc StreamMetrics(GetMetricsRequest) returns (stream ServerMetrics);

  // List all deployed apps
  rpc ListApps(GetAppListRequest) returns (AppListResponse);
}
```

#### `app.proto`

```protobuf
syntax = "proto3";
package rover.v1;

import "rover/v1/common.proto";

// --- Deployment ---

message DeployRequest {
  string name = 1;           // App name (unique)
  Runtime runtime = 2;
  AppType app_type = 3;
  string manifest_toml = 4;  // Full TOML manifest content
  bytes source_archive = 5;  // .tar.gz of the app source code
}

message DeployResponse {
  string app_id = 1;         // UUID assigned by server
}

// Streamed during deploy (build output, step progress)
message DeployEvent {
  oneof event {
    DeployLogLine log = 1;
    DeployProgress progress = 2;
    DeployComplete complete = 3;
    DeployError error = 4;
  }
}

message DeployLogLine {
  string line = 1;
  bool is_stderr = 2;
}

message DeployProgress {
  string stage = 1;          // e.g., "building", "installing", "starting"
  float percent = 2;         // 0.0 - 1.0
}

message DeployComplete {
  string app_id = 1;
}

message DeployError {
  string message = 1;
}

// --- App controls ---

message AppRequest {
  string app_id = 1;
}

message AppDetailResponse {
  string app_id = 1;
  string name = 2;
  Runtime runtime = 3;
  AppType app_type = 4;
  AppStatus status = 5;
  string build_command = 6;
  string run_command = 7;
  map<string, string> env_vars = 8;  // Non-secret env vars
  Timestamp created_at = 9;
  Timestamp updated_at = 10;
  int32 restart_count = 11;
  optional int32 pid = 12;
}

// --- Environment variables ---

message SetEnvRequest {
  string app_id = 1;
  map<string, string> env_vars = 2;  // Key-value pairs to set/update
}

message DeleteEnvRequest {
  string app_id = 1;
  repeated string keys = 2;  // Keys to delete
}

message SetSecretRequest {
  string app_id = 1;
  string key = 2;
  string value = 3;  // Stored encrypted at rest
}

// --- Logs ---

message LogStreamRequest {
  string app_id = 1;
  bool follow = 2;           // true = tail -f, false = return recent and close
  int32 tail_lines = 3;      // Number of recent lines to send first (default 100)
}

message LogEntry {
  Timestamp timestamp = 1;
  string line = 2;
  bool is_stderr = 3;
}

// --- Shell (embedded terminal) ---

message ShellRequest {
  string app_id = 1;         // Optional: spawn shell in app's working directory
  int32 rows = 2;            // Terminal rows
  int32 cols = 3;            // Terminal cols
}

message ShellInput {
  bytes data = 1;            // Raw stdin bytes
}

message ShellOutput {
  bytes data = 1;            // Raw stdout/stderr bytes
}

// --- App service ---

service AppService {
  // Deploy a new app (returns stream of build events)
  rpc Deploy(DeployRequest) returns (stream DeployEvent);

  // Get detailed app info
  rpc GetApp(AppRequest) returns (AppDetailResponse);

  // Start a stopped app
  rpc StartApp(AppRequest) returns (AppDetailResponse);

  // Stop a running app (SIGTERM, then SIGKILL after timeout)
  rpc StopApp(AppRequest) returns (AppDetailResponse);

  // Restart an app (stop + start)
  rpc RestartApp(AppRequest) returns (AppDetailResponse);

  // Delete an app and all its data
  rpc DeleteApp(AppRequest) returns (rover.v1.Empty);

  // Stream app logs
  rpc StreamLogs(LogStreamRequest) returns (stream LogEntry);

  // Set environment variables
  rpc SetEnv(SetEnvRequest) returns (AppDetailResponse);

  // Delete environment variables
  rpc DeleteEnv(DeleteEnvRequest) returns (AppDetailResponse);

  // Set a secret (encrypted at rest)
  rpc SetSecret(SetSecretRequest) returns (rover.v1.Empty);

  // Open an interactive shell session
  rpc Shell(stream ShellInput) returns (stream ShellOutput);
}
```

### gRPC Service Summary

| Service | Purpose | Auth Required? |
|---------|---------|----------------|
| `AuthService.Pair` | Exchange pairing token for API key | No |
| `ServerService.GetInfo` | Server identity + version | Yes |
| `ServerService.GetMetrics` | One-shot resource metrics | Yes |
| `ServerService.StreamMetrics` | Streaming resource metrics | Yes |
| `ServerService.ListApps` | List deployed apps | Yes |
| `AppService.Deploy` | Deploy new app (streaming response) | Yes |
| `AppService.GetApp` | App details | Yes |
| `AppService.StartApp` | Start app | Yes |
| `AppService.StopApp` | Stop app | Yes |
| `AppService.RestartApp` | Restart app | Yes |
| `AppService.DeleteApp` | Delete app | Yes |
| `AppService.StreamLogs` | Stream app logs | Yes |
| `AppService.SetEnv` | Set env vars | Yes |
| `AppService.DeleteEnv` | Delete env vars | Yes |
| `AppService.SetSecret` | Set secret | Yes |
| `AppService.Shell` | Interactive shell (bidirectional stream) | Yes |

---

## 5. Server Architecture

### Entrypoint (`main.rs`)

```
./roverd --mode lan --port 9050 --data-dir ~/.rover
```

1. Parse CLI flags (clap)
2. Load or initialize SQLite state store
3. Initialize runtime registry (register available runtimes)
4. Select transport based on `--mode`
5. Generate pairing token (if none exists)
6. Start health check loop for existing apps
7. Discover and display LAN IP + pairing token
8. Build tonic gRPC server with all services
9. Serve via the selected transport

### Runtime Registry

A `Runtime` trait in `runtime/mod.rs`:

```rust
#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    /// The runtime identifier
    fn runtime(&self) -> Runtime;

    /// Check if the required toolchain is installed
    async fn check_installed(&self) -> Result<bool, RoverError>;

    /// Execute the build command in the app's source directory
    async fn build(&self, app_dir: &Path, command: &str) -> Result<(), RoverError>;

    /// Return the command and args to run the app process
    fn run_command(&self, app_dir: &Path, command: &str) -> (String, Vec<String>);
}
```

Runtime implementations:

| Runtime | Build Check | Build Command | Run Command |
|---------|-------------|---------------|-------------|
| Python | `which python3` | `pip install -r requirements.txt` | `python3 main.py` |
| Node | `which node` | `npm install` | `node index.js` |
| Go | `which go` | `go build -o app .` | `./app` |
| Rust | `which cargo` | `cargo build --release` | `./target/release/app` |

The registry is a `HashMap<Runtime, Box<dyn RuntimeHandler>>`. Adding a new runtime = implementing the trait + registering it.

### Process Manager (`process.rs`)

```rust
pub struct ProcessManager {
    processes: HashMap<String, ManagedProcess>,  // app_id → process
}

pub struct ManagedProcess {
    child: tokio::process::Child,
    pid: u32,
    restart_count: u32,
    max_restarts: u32,  // to prevent crash loops
}
```

Key behaviors:
- **Services**: Auto-restart on crash with exponential backoff (1s, 2s, 4s, ... max 60s). Give up after N consecutive crashes.
- **Jobs**: Run once, capture exit code, don't restart.
- **Stop**: Send SIGTERM, wait 5s, send SIGKILL.
- **Stdin/stdout/stderr**: Captured via tokio pipes for log streaming. Each line is timestamped and persisted.

### State Store (`state.rs`)

SQLite schema:

```sql
CREATE TABLE apps (
    app_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    runtime TEXT NOT NULL,
    app_type TEXT NOT NULL,       -- 'service' or 'job'
    status TEXT NOT NULL,         -- 'deploying', 'running', 'stopped', ...
    build_command TEXT NOT NULL,
    run_command TEXT NOT NULL,
    source_dir TEXT NOT NULL,     -- Path to extracted source
    manifest_toml TEXT NOT NULL,  -- Original manifest
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    restart_count INTEGER DEFAULT 0,
    pid INTEGER
);

CREATE TABLE env_vars (
    app_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    is_secret INTEGER DEFAULT 0,  -- 1 if encrypted at rest
    PRIMARY KEY (app_id, key),
    FOREIGN KEY (app_id) REFERENCES apps(app_id) ON DELETE CASCADE
);

CREATE TABLE auth_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token_hash TEXT NOT NULL UNIQUE,  -- SHA-256 hash of the API key
    created_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE server_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Stores: pairing_token, server_name, etc.

CREATE TABLE logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    line TEXT NOT NULL,
    is_stderr INTEGER DEFAULT 0,
    FOREIGN KEY (app_id) REFERENCES apps(app_id) ON DELETE CASCADE
);

CREATE INDEX idx_logs_app_time ON logs(app_id, timestamp);
```

Log retention: Configurable via `server_config`. Default: keep last 10,000 lines per app, delete older.

### Health Check Loop (`health.rs`)

Runs every 30 seconds:
1. Query all apps with `status = 'running'`
2. Check if their PID is still alive
3. If dead: mark as `crashed`, attempt restart if `service` type
4. Update `restart_count`

---

## 6. Client Architecture

### Framework: Iced

Iced uses an Elm-like architecture: `Application` trait with `State`, `Message`, `view()`, and `update()`.

### App Structure

```rust
pub struct RoverApp {
    // Navigation
    pub screen: Screen,

    // Connection
    pub connection_state: ConnectionState,
    pub profiles: Vec<ConnectionProfile>,

    // Server data (populated when connected)
    pub server_info: Option<ServerInfo>,
    pub apps: Vec<AppSummary>,
    pub metrics: Option<ServerMetrics>,
    pub selected_app_id: Option<String>,
    pub app_detail: Option<AppDetailResponse>,

    // Streaming state
    pub log_entries: Vec<LogEntry>,
    pub deploy_events: Vec<DeployEvent>,
    pub shell_output: Vec<u8>,

    // UI state
    pub theme: Theme,
    pub toasts: Vec<Toast>,
}

pub enum Screen {
    Connections,   // Manage connection profiles
    Dashboard,     // Server overview + app list
    AppDetail,     // Single app view
    Deploy,        // New deployment form
    Terminal,      // Embedded shell
}

pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { client: RoverClient },
    Error { message: String },
}
```

### Message Enum (events)

```rust
pub enum Message {
    // Navigation
    Navigate(Screen),
    SelectApp(String),

    // Connection
    Connect { address: String, pairing_token: String },
    ConnectWithApiKey { address: String, api_key: String },
    Disconnect,
    SaveProfile(ConnectionProfile),
    DeleteProfile(String),
    ConnectionResult(Result<PairResponse, String>),

    // Server
    RefreshServerInfo,
    ServerInfoResult(Result<ServerInfo, String>),
    RefreshMetrics,
    MetricsResult(Result<ServerMetrics, String>),
    RefreshAppList,
    AppListResult(Result<Vec<AppSummary>, String>),

    // App actions
    DeployApp { /* form fields */ },
    DeployEvent(DeployEvent),
    StartApp(String),
    StopApp(String),
    RestartApp(String),
    DeleteApp(String),
    AppActionResult(Result<AppDetailResponse, String>),

    // Logs
    StartLogStream(String),
    StopLogStream,
    LogEntry(LogEntry),

    // Env vars
    SetEnvVar { app_id: String, key: String, value: String },
    DeleteEnvVar { app_id: String, key: String },
    SetSecret { app_id: String, key: String, value: String },

    // Shell
    OpenShell(String),
    CloseShell,
    ShellInput(Vec<u8>),
    ShellOutput(Vec<u8>),

    // UI
    Tick,  // For periodic refresh
    DismissToast(usize),
}
```

### API Layer (`api/client.rs`)

Wraps tonic gRPC client. Each method is async and returns `Result<T, String>`:

```rust
pub struct RoverClient {
    channel: tonic::transport::Channel,
    auth_client: AuthServiceClient<...>,
    server_client: ServerServiceClient<...>,
    app_client: AppServiceClient<...>,
    api_key: String,
}

impl RoverClient {
    pub async fn connect(address: &str) -> Result<Self, String>;
    pub async fn pair(&mut self, token: &str) -> Result<PairResponse, String>;

    // Each gRPC call adds the api_key metadata header
    pub async fn get_info(&mut self) -> Result<ServerInfo, String>;
    pub async fn get_metrics(&mut self) -> Result<ServerMetrics, String>;
    pub async fn stream_metrics(&mut self) -> Result<Streaming<ServerMetrics>, String>;
    pub async fn list_apps(&mut self, limit: i32, offset: i32) -> Result<AppListResponse, String>;
    pub async fn deploy(&mut self, req: DeployRequest) -> Result<Streaming<DeployEvent>, String>;
    pub async fn get_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String>;
    pub async fn start_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String>;
    pub async fn stop_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String>;
    pub async fn restart_app(&mut self, app_id: &str) -> Result<AppDetailResponse, String>;
    pub async fn delete_app(&mut self, app_id: &str) -> Result<(), String>;
    pub async fn stream_logs(&mut self, app_id: &str, follow: bool, tail: i32) -> Result<Streaming<LogEntry>, String>;
    pub async fn set_env(&mut self, app_id: &str, vars: HashMap<String, String>) -> Result<AppDetailResponse, String>;
    pub async fn delete_env(&mut self, app_id: &str, keys: Vec<String>) -> Result<AppDetailResponse, String>;
    pub async fn set_secret(&mut self, app_id: &str, key: &str, value: &str) -> Result<(), String>;
    pub async fn shell(&mut self, app_id: &str) -> Result<(Streaming<ShellOutput>, Sink<ShellInput>), String>;
}
```

### Screens

| Screen | Purpose | Key Widgets |
|--------|---------|-------------|
| **Connections** | Add/edit/remove server profiles. Enter address + pairing token or API key. | Form fields, profile list, Connect button |
| **Dashboard** | Server overview: metrics bar, app list with status badges. Quick actions (start/stop/restart). | `MetricBar`, `AppCard` list, Refresh button |
| **AppDetail** | Single app: status, env vars, secrets form, log viewer, start/stop/restart/delete buttons. | `LogViewer`, env var table, action buttons |
| **Deploy** | Form: app name, runtime selector, build command, run command, type (service/job), source directory picker. Build log stream during deploy. | Form fields, `LogViewer` for build output |
| **Terminal** | Full terminal emulator connected to gRPC Shell stream. | Terminal widget (xterm.js via webview or custom grid) |

### Connection Profiles

Stored as JSON in `~/.config/rover/profiles.json`:

```json
{
  "profiles": [
    {
      "id": "uuid-v4",
      "name": "Galaxy S25 (Home)",
      "address": "192.168.1.42:9050",
      "api_key": "rover-key-abc123...",
      "last_used": "2026-06-25T10:30:00Z"
    }
  ],
  "active_profile_id": "uuid-v4"
}
```

---

## 7. Transport Abstraction

### Trait Definition (`rover-transport`)

```rust
// Server side: creates a listening socket
#[async_trait]
pub trait TransportServer: Send + Sync {
    /// Bind and start listening. Returns the local address.
    async fn bind(&self, port: u16) -> Result<SocketAddr, TransportError>;

    /// Accept incoming connections and pass them to the gRPC server.
    /// This method blocks until the server shuts down.
    async fn serve(
        &self,
        service: tonic::transport::server::Router,
    ) -> Result<(), TransportError>;

    /// Discover and return the local LAN IP(s) for display
    fn local_addresses(&self) -> Vec<SocketAddr>;

    /// Graceful shutdown
    async fn shutdown(&self);
}

// Client side: connects to a server
#[async_trait]
pub trait TransportClient: Send + Sync {
    /// Connect to an address. The address format depends on the transport.
    /// - LAN: "192.168.1.42:9050"
    /// - Relay: "rover-relay.example.com/session/abc123"
    async fn connect(&self, address: &str) -> Result<tonic::transport::Channel, TransportError>;
}
```

### LAN Implementation (`rover-transport-lan`)

**Server**: Binds `TcpListener` to `0.0.0.0:{port}`. Calls `tonic::transport::Server::builder().add_service(...).serve_with_incoming(stream_of_tcp_streams)`.

**Client**: `tonic::transport::Endpoint::from(address).connect().await`.

Literally just raw TCP. No TLS. gRPC over HTTP/2 cleartext (h2c).

### Relay Implementation (`rover-transport-relay`) — Stub

Panics with "relay transport not implemented yet". Trait methods return `Err(TransportError::Unsupported(...))`.

---

## 8. Auth Flow

### Pairing (First-Time Setup)

```
Desktop Client                          Rover Server (Termux)
     │                                        │
     │                                        │ Generate pairing token on startup
     │                                        │ (if no API keys exist in DB)
     │                                        │ Print: "Pairing token: rover-pair-abc123"
     │                                        │
     │  <user copies token from phone>         │
     │                                        │
     │──Pair(pairing_token)──────────────────►│
     │                                        │ Verify token
     │                                        │ Generate persistent API key
     │                                        │ Store hash in SQLite
     │◄──PairResponse(api_key)───────────────│
     │                                        │
     │  Save api_key + address in profile     │
     │                                        │
     │──All subsequent RPCs──────────────────►│
     │  (authorization: Bearer <api_key>)     │ Verify API key hash
```

### API Key Format

Random 32-byte value, base64url-encoded: `rover-key-dGhpcyBpcyBhIHRlc3Qga2V5...`

Stored in SQLite as SHA-256 hash. The raw key is only given to the client once (during pairing). If lost, user deletes `auth_tokens` table rows and re-pairs.

### Token Validation (Server Middleware)

A tonic interceptor (`fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status>`) extracts the `authorization` metadata header, looks up the hash in SQLite, and rejects with `UNAUTHENTICATED` if invalid. The `Pair` RPC is whitelisted to skip this check.

---

## 9. App Lifecycle & Runtime System

### Deploy Flow

```
Client                              Server
  │                                    │
  │──Deploy(manifest, source.tar.gz)──►│
  │                                    │ 1. Validate manifest
  │                                    │ 2. Insert app row (status: deploying)
  │                                    │ 3. Extract source.tar.gz → apps/{app_id}/
  │◄──DeployEvent(log: "...")─────────│ 4. Run build command (stream output)
  │◄──DeployEvent(log: "...")─────────│    ...
  │◄──DeployEvent(progress: "...")────│ 5. Build complete
  │◄──DeployEvent(complete)──────────│ 6. Update status → starting
  │                                    │ 7. Spawn process with env vars
  │                                    │ 8. Health check
  │                                    │ 9. Update status → running
```

### Directory Layout on Server

```
~/.rover/
├── rover.db                    ← SQLite state
├── apps/
│   ├── {app_id_1}/
│   │   ├── source/             ← Extracted source code
│   │   │   ├── main.py
│   │   │   ├── requirements.txt
│   │   │   └── ...
│   │   ├── rover.toml          ← Copy of manifest
│   │   └── data/               ← Persistent app data (SQLite per app, etc.)
│   └── {app_id_2}/
│       └── ...
└── server.toml                 ← Server config (port, mode, etc.)
```

---

## 10. State Persistence

### What Gets Persisted

| Data | Location | Notes |
|------|----------|-------|
| App definitions | SQLite `apps` table | id, name, runtime, status, commands, paths |
| Environment variables | SQLite `env_vars` table | Secrets marked `is_secret=1` |
| Auth tokens | SQLite `auth_tokens` table | SHA-256 hashed |
| Server config | SQLite `server_config` table | pairing token, server name |
| Logs | SQLite `logs` table | Per-app, configurable retention |
| Connection profiles | `~/.config/rover/profiles.json` | Client-side only |

---

## 11. Network & Connectivity

### V1: Local LAN

- Server binds `0.0.0.0:{port}` over raw TCP
- Server discovers and prints its LAN IP(s) on startup
- Client connects via that IP
- Native gRPC over HTTP/2 cleartext (h2c)
- No TLS, no Cloudflare, no external services

### V2: Relay Transport (Future)

- Server connects outbound to a relay server (WebSocket or TCP)
- Client connects to same relay
- Relay proxies gRPC frames between them
- Allows access from anywhere without NAT traversal issues

### IP Discovery

```rust
fn discover_lan_ips() -> Vec<IpAddr> {
    // Iterate network interfaces
    // Filter: not loopback, IPv4 only (for simplicity)
    // Return list of 192.168.x.y / 10.x.y.z addresses
}
```

---

## 12. Deployment Manifest

### TOML Schema (`rover.toml`)

```toml
[app]
name = "my-bot"              # Unique app name
runtime = "python"           # python | node | go | rust
type = "service"             # service | job
version = "1.0.0"           # Optional, informational

[build]
command = "pip install -r requirements.txt"

[run]
command = "python main.py"

[env]
# Non-secret environment variables
DATABASE_URL = "sqlite:///data/app.db"
LOG_LEVEL = "info"
```

The client provides a form that generates this TOML, so the user never writes it manually.

---

## 13. Development Roadmap

### Phase 0: Foundation (Sequential — Everything Depends On This)

**Duration estimate:** 1-2 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 0.1 | Initialize Cargo workspace | Root `Cargo.toml` | `cargo check` passes |
| 0.2 | Create `rover-core` with `RoverError`, `AppStatus`, `Runtime` enums | `rover-core` | `cargo test` passes |
| 0.3 | Create `rover-core::manifest` — `AppManifest` struct + TOML parser | `rover-core` | Parse sample TOML, test validation |
| 0.4 | Create `rover-core::profile` — `ConnectionProfile` struct + JSON ser/de | `rover-core` | Round-trip JSON test |
| 0.5 | Write `.proto` files (all services) | `proto/` | `protoc` compiles without errors |
| 0.6 | Create `rover-proto` with `build.rs` (tonic-build) | `rover-proto` | `cargo build` succeeds |
| 0.7 | Create `rover-transport` — trait definitions | `rover-transport` | `cargo build` succeeds |
| 0.8 | Create `rover-transport-lan` — implement TCP transport | `rover-transport-lan` | Bind + connect test on localhost |
| 0.9 | Create `rover-transport-relay` — stub only | `rover-transport-relay` | `cargo build` succeeds |

**Gate:** All crates compile. Core types and transport are testable.

---

### Phase 1A: Server Core (Parallel with 1B)

**Duration estimate:** 2-3 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 1A.1 | `rover-server` binary scaffold (clap flags, tokio main) | `rover-server` | `./roverd --help` prints usage |
| 1A.2 | SQLite state store (`state.rs`) — schema creation, CRUD for apps | `rover-server` | Integration test: create app, read back |
| 1A.3 | Auth system (`auth.rs`) — pairing token gen, API key hash/store/verify | `rover-server` | Test: pair returns key, verify accepts, bad key rejects |
| 1A.4 | Runtime registry (`runtime/mod.rs`) — trait + Python impl | `rover-server` | Test: `check_installed()` detects python3 |
| 1A.5 | Process manager (`process.rs`) — spawn, monitor PID, SIGTERM, capture stdout | `rover-server` | Test: spawn `python -c "print('hi')"`, read stdout, wait for exit |
| 1A.6 | Health check loop (`health.rs`) — poll PIDs, update status | `rover-server` | Test: kill a process externally, verify status → crashed |
| 1A.7 | Deploy orchestrator (`deploy.rs`) — extract tar.gz, run build, start | `rover-server` | Integration test: deploy a dummy Python app |
| 1A.8 | gRPC service impls — `AuthService`, `ServerService`, `AppService` handlers | `rover-server` | Test: start server, connect with grpcurl |
| 1A.9 | Wire everything in `main.rs` — full startup sequence | `rover-server` | Manual: `./roverd --mode lan`, see pairing token |

**Gate:** Server starts, accepts pairing, deploys a Python app, shows it in app list.

---

### Phase 1B: Client Core (Parallel with 1A)

**Duration estimate:** 2-3 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 1B.1 | `rover-client` binary scaffold — Iced `Application` impl, dark theme | `rover-client` | Window opens with dark background |
| 1B.2 | Navigation system — `Screen` enum, sidebar/tab bar, screen switching | `rover-client` | Click between screens |
| 1B.3 | Connection profiles (`state/profiles.rs`) — load/save JSON, list/edit/delete | `rover-client` | Add profile, restart app, it persists |
| 1B.4 | Connection screen — address + pairing token form, connect button | `rover-client` | Mock: form validates, transitions to Connecting state |
| 1B.5 | `api/client.rs` — `RoverClient` struct, connect + pair methods | `rover-client` | Needs running server (1A.8). Pair and store API key. |
| 1B.6 | Dashboard screen — server info card, app list, metrics bar (static) | `rover-client` | Mock data renders correctly |
| 1B.7 | Wire real data — populate dashboard from gRPC calls | `rover-client` | Connect to real server, see apps + metrics |
| 1B.8 | App detail screen — status, env vars table, action buttons | `rover-client` | Select app in list, see details |
| 1B.9 | Deploy form — app name, runtime selector, build/run commands, source picker | `rover-client` | Fill form, submit → deploy call succeeds |

**Gate:** Client connects, pairs, lists apps, views app detail, deploys new app.

---

### Phase 2A: Streaming & Real-Time (Parallel with 2B)

**Duration estimate:** 1-2 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 2A.1 | `AppService.StreamLogs` server handler — read from SQLite or pipe buffer | `rover-server` | grpcurl streams logs |
| 2A.2 | `AppService.Deploy` streaming — stream build output during deploy | `rover-server` | grpcurl sees build lines as they happen |
| 2A.3 | `ServerService.StreamMetrics` — push metrics every 5s | `rover-server` | grpcurl sees CPU/RAM updates |

**Gate:** All streaming RPCs work.

---

### Phase 2B: Client Streaming UI (Parallel with 2A)

**Duration estimate:** 1-2 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 2B.1 | `LogViewer` widget — scrollable, auto-scroll, line buffering, dark theme coloring | `rover-client` | Renders 1000 lines smoothly |
| 2B.2 | Wire log streaming to `LogViewer` — start/stop stream, follow mode | `rover-client` | See live logs from a running app |
| 2B.3 | Build log in deploy screen — show `DeployEvent` stream during deployment | `rover-client` | Deploy app, see build output live |
| 2B.4 | Live metrics on dashboard — update metric bars every 5s | `rover-client` | CPU/RAM bars update in real time |

**Gate:** Log tailing, build streaming, and live metrics all work in the client.

---

### Phase 3: Advanced Features

**Duration estimate:** 2-3 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 3.1 | `AppService.Shell` server handler — spawn `sh` in app dir, bidirectional stream | `rover-server` | Test with simple echo via grpcurl |
| 3.2 | Terminal widget in client — basic ANSI-capable terminal, stdin forwarding | `rover-client` | Type `ls`, see output |
| 3.3 | Env var manager in client — add/edit/delete env vars and secrets | `rover-client` | Set env var, restart app, it's available |
| 3.4 | Health check status — show crash count, restart count in app detail | `rover-client` | Kill app, see status update to crashed |
| 3.5 | Error handling polish — graceful disconnection, retry, meaningful errors | Both | Pull network cable, client shows "disconnected" |

**Gate:** Full feature set working.

---

### Phase 4: Polish & Release

**Duration estimate:** 1-2 sessions

| Task | Crate(s) | Testing Gate |
|------|----------|--------------|
| 4.1 | Node.js runtime implementation | `rover-server` | Deploy a Node app |
| 4.2 | Icon, about dialog, window title | `rover-client` | Looks like a real app |
| 4.3 | Build/release scripts — cross-compilation for aarch64 (server) + desktop platforms | Root | `cargo build --release` for all targets |
| 4.4 | README with setup instructions (Termux deps, build from source) | Root | Someone can follow and get it running |

---

### Phase 5: Relay Transport (Future)

- Implement `rover-transport-relay` — connect via WebSocket to a relay
- Build a small relay server binary (`rover-relay`) that bridges two connections
- Client: add relay address option to connection profiles

---

### Parallel Development Opportunities

```
Phase 0: ────────────────── (sequential, everything depends on it)
                          │
          ┌───────────────┴───────────────┐
          │                               │
Phase 1A: Server Core              Phase 1B: Client Core
(no dependency on client)         (needs server for integration,
                                   but can mock during dev)
          │                               │
          └───────────────┬───────────────┘
                          │
          ┌───────────────┴───────────────┐
          │                               │
Phase 2A: Server Streaming        Phase 2B: Client Streaming UI
(can be tested with grpcurl)     (needs server for integration)
          │                               │
          └───────────────┬───────────────┘
                          │
Phase 3: Advanced Features (some parallel, some need both sides)
Phase 4: Polish (mostly parallel)
```

Phases 1A and 1B can be developed entirely in parallel by different agents. Same for 2A and 2B. The `rover-core` and `rover-proto` crates serve as the contract between them.

---

## 14. Crate Dependency Graph

```
                         ┌─────────────┐
                         │  rover-core │  (zero external deps for core types)
                         └──────┬──────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                  │
     ┌────────┴────────┐ ┌─────┴──────┐  ┌───────┴────────┐
     │ rover-transport │ │ rover-proto│  │ (future crates)│
     └────────┬────────┘ └─────┬──────┘  └────────────────┘
              │                 │
    ┌─────────┼─────────┐      │
    │         │         │      │
┌───┴───┐ ┌───┴────┐    │      │
│ LAN   │ │ Relay  │    │      │
│Transport│Transport│   │      │
└───┬───┘ └───┬────┘    │      │
    │         │         │      │
    └────┬────┘         │      │
         │              │      │
    ┌────┴──────────────┴──────┴────┐
    │         rover-server           │
    └───────────────────────────────┘
    ┌───────────────────────────────┐
    │         rover-client           │
    └───────────────────────────────┘
```

---

## 15. Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| **gRPC + Protobuf** | Strong typing, code generation, streaming built-in, industry standard. Better than REST for this use case. |
| **tonic** | Mature Rust gRPC library. Async, tokio-native, good protobuf integration. |
| **Iced** | Pure Rust GUI. No JavaScript/webview. Elm architecture maps well to async RPC patterns. Cross-platform. |
| **SQLite (server)** | Embedded, zero-config, reliable. Perfect for single-device state. No Postgres/MySQL overhead. |
| **No containers** | Android/Termux constraints. Simpler process model. Apps are trusted (single user). |
| **tokio** | De facto async runtime. tonic depends on it. Iced's async support works with tokio. |
| **Transport abstraction** | Swap LAN ↔ relay without touching server logic. Future-proof. |
| **`rover-core` separate from `rover-proto`** | Core types are plain Rust, testable without proto compilation. Proto ↔ core mapping at boundary. |
| **TOML manifests** | Human-readable, native Rust support (toml crate), simpler than YAML. |
| **Pairing token auth** | Simple, secure enough for local/relay access. No OAuth, no passwords, no SSH key management. |
| **Monorepo** | Shared proto definitions, core types, transport crates. Easier CI, versioning, cross-crate refactoring. |
