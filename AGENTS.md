# AGENTS.md — Master Prompt for Rover Development

> **Who this is for:** Any AI coding agent (or human developer) working on the Rover project in any session. Read this first, every time. It establishes the rules of the road.

---

## 0. First Steps (Do This Every Session)

1. **Read `ARCHITECTURE.md`** — Understand the system before touching code. Pay attention to the crate dependency graph in §3 and §14.
2. **Read `CHECKLIST.md`** — Know what's done and what's next. Find your task.
3. **Run `cargo check` and `cargo test`** — Confirm the repo is in a clean state before you start. If it isn't, note it and fix forward.
4. **Check `git status` / `git log`** — See if another agent left uncommitted work or WIP.

---

## 1. Task Assignment

Tasks come from `CHECKLIST.md`, organized by phase. When you begin a task:

- **Mark it `[~]`** (in progress) in `CHECKLIST.md`.
- **Don't skip phases.** Phase 0 → 1A/1B → 2A/2B → 3 → 4 → 5. Foundation first.

You may work across crates as needed — a single feature often touches both server and client. The contract between them is `rover-proto` + `rover-core`.

---

## 2. Code Practices

### Rust Style

- **Edition 2024** — All crates use `edition = "2024"`.
- **`rustfmt`** — Format all code with `cargo fmt` before committing.
- **`clippy`** — Run `cargo clippy --workspace` and resolve all warnings. No `#[allow(clippy::...)]` without a comment explaining why.
- **No `unwrap()` in production code** — Use `?`, `.map_err()`, or `anyhow::Context`. `unwrap()` is acceptable only in tests and `fn main`.
- **No `println!()`** — Use `tracing::info!()`, `tracing::warn!()`, `tracing::error!()`. Server uses `tracing-subscriber` with env filter. Client uses the same.
- **No panics** — All errors must be handled gracefully. Return `Result` or `anyhow::Result`.
- **`anyhow::Result` for binaries, `thiserror` for libraries** — `rover-core`, `rover-transport`, `rover-proto` should define typed errors with `thiserror`. Binaries (`roverd`, `rover`) can use `anyhow::Result`.

### Naming Conventions

- **Files**: `snake_case.rs`
- **Types**: `PascalCase` (structs, enums, traits)
- **Functions/methods**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`, one module per conceptual unit
- **gRPC services**: `PascalCase` matching the `.proto` service names
- **gRPC messages**: `PascalCase` matching the `.proto` message names

### Module Structure

- Each module gets its own `.rs` file or `mod.rs` directory.
- `lib.rs` re-exports the public API. Don't put implementation in `lib.rs`.
- Use `pub(crate)` for internal visibility. Use `pub` only for the crate's public API.
- Server modules: `auth.rs`, `state.rs`, `process.rs`, `health.rs`, `deploy.rs`, `runtime/`
- Client modules: `api/`, `screens/`, `widgets/`, `state/`

### Imports

- Group imports: `std` first, then external crates, then `crate::` internal.
- Don't use `use super::*` or glob imports inside modules.
- Prefer explicit imports over wildcard (`use crate::foo::Bar` over `use crate::foo::*`).

---

## 3. Dependency Management

- **All dependencies are workspace-level.** Add new deps to the root `Cargo.toml` under `[workspace.dependencies]`, then reference them in crate `Cargo.toml` files with `workspace = true`.
- **Don't add a dep you don't need.** Check if the functionality already exists in an existing dep.
- **Prefer crates in the existing ecosystem.** We use `tokio`, `tonic`, `rusqlite`, `clap`, `serde`, `iced`. Don't introduce competing alternatives (e.g., don't add `axum` alongside `tonic`, don't add `gtk-rs` alongside `iced`).
- **If you MUST add a new dep**, justify it in a code comment or commit message.

---

## 4. Testing

### When to Write Tests

- **Every new `pub fn` in `rover-core`** must have at least one unit test.
- **Every gRPC service handler** in `rover-server` must have an integration test (start server, make RPC, verify response).
- **Every new screen/widget in `rover-client`** — manual testing is acceptable. Unit tests for pure logic (state transitions, form validation).
- **Bug fixes** — Add a regression test.

### How to Run Tests

```bash
cargo test                     # All tests
cargo test -p rover-core       # Specific crate
cargo test -- --nocapture      # Show println/tracing output
```

### Test Naming

- Inside `#[cfg(test)] mod tests { ... }`: `fn test_<what_you_are_testing>()`
- Integration tests in `tests/` directory: `<scenario>.rs`

---

## 5. The `rover-proto` Contract

The `.proto` files in `proto/rover/v1/` are the single source of truth for the API contract.

### Rules for Changing Protos

1. **This is a shared resource.** If you change a `.proto` file, you must verify that BOTH server and client still compile.
2. **Don't break existing RPCs.** Add new fields/messages/RPCs, don't remove or rename existing ones unless the task explicitly calls for it.
3. **Regenerate**: `cargo build -p rover-proto` after any `.proto` change.
4. **Mapping**: `rover-core` types should NOT depend on proto types. Keep the mapping at the boundary — convert between proto types and core types in the gRPC handler layer.

### Adding a New RPC

1. Define the message and service in the appropriate `.proto` file.
2. Run `cargo build -p rover-proto` to regenerate.
3. Implement the handler in `rover-server` (add to the relevant service impl).
4. Add the client method in `rover-client/src/api/client.rs`.
5. Add a test that exercises the new RPC end-to-end.

---

## 6. The `rover-core` Contract

`rover-core` is the shared library used by both server and client. The rule is simple:

**`rover-core` must not depend on `rover-proto`, `rover-server`, or `rover-client`.**

Why? Because `rover-proto` depends on `tonic`/`prost` which are heavy build dependencies (they invoke `protoc` and generate code). If `rover-core` depended on `rover-proto`, every unit test in core would trigger a full protobuf compilation. Keeping them separate keeps builds fast and tests independent.

`rover-core` MAY use I/O if it makes sense — there's no blanket ban. The point is to avoid dragging in the entire gRPC/protobuf stack just to define a shared enum. If core needs to read a file or talk to SQLite, that's fine. Just don't make it depend on `tonic`.

Keep `rover-core` focused. If something is only used by the server, put it in `rover-server`. If only by the client, put it in `rover-client`.

---

## 7. The State Store (`rover-server/src/state.rs`)

SQLite schema must be explicitly versioned and migrated. No append-only guesswork.

### Migration Rules

- **Versioned schema**: Store a `schema_version` integer in `server_config`. On startup, check it.
- **Migration functions**: Write a function per version bump (e.g., `migrate_v1_to_v2()`) that runs the needed `ALTER TABLE` / `CREATE TABLE` statements.
- **Every migration must be tested.** Add a test that creates a DB at the old version, runs the migration, and verifies the new schema.
- **Dropping columns is allowed** if the data is no longer needed — just do it in a migration function, not silently.
- **All state store methods** receive `&self` (the inner `Mutex<Connection>` handles locking).

---

## 8. Process Management (`rover-server/src/process.rs`)

- **Child processes** are spawned via `tokio::process::Command`.
- **SIGTERM first**, then SIGKILL after 5 seconds for graceful shutdown.
- **stdout/stderr pipes** must be captured for log streaming.
- **Services auto-restart** with exponential backoff (1s, 2s, 4s, 8s, max 60s).
- **Jobs run once** and capture the exit code. No restart.
- **Crash loop protection**: Max 5 consecutive restarts, then mark as `Crashed`.

---

## 9. Auth Flow

- **Pairing**: The server prints a one-time pairing token on startup. The client sends it via `AuthService::Pair`. The server returns a persistent API key. The token is consumed (can't be reused).
- **API key**: Sent as gRPC metadata: `authorization: Bearer <api_key>`. Validated via an interceptor that hashes and checks against SQLite.
- **`Pair` RPC is unauthenticated.** All other RPCs require the API key.
- **No passwords, no OAuth, no SSH keys.** Just the pairing token → API key flow.

---

## 10. Transport Abstraction

- The server selects transport at startup via `--mode` flag.
- The `TransportServer`/`TransportClient` traits live in `rover-transport`.
- **V1**: `rover-transport-lan` (raw TCP, no TLS, h2c).
- **V2**: `rover-transport-relay` (stub — returns `NotImplemented`).
- Don't hardcode transport specifics in the server or client logic. Use the traits.

---

## 11. Client Architecture (Iced 0.13)

Iced 0.13 uses a **functional builder pattern**, not a trait:

```rust
iced::application(title_fn, update_fn, view_fn)
    .theme(theme_fn)
    .window_size(size)
    .run_with(init_fn)
```

- **`RoverApp`** is a plain struct (not an `impl Application`).
- **`Message`** is an enum in `message.rs` — every UI event and async response.
- **`update()`** receives `&mut RoverApp` and `Message`, returns `Task<Message>`.
- **`view()`** receives `&RoverApp`, returns `Element<Message>`.
- **Async operations**: Spawned via `Task::perform(future, |result| Message::Variant(result))`.
- **Streaming** (logs, metrics): Use `Subscription` for periodic polls, or `Task::run` for gRPC streaming.

---

## 12. Working with the Checklist

### Starting a Task

1. Find the next unchecked `[ ]` item in your assigned phase.
2. Change it to `[~]` (in progress).
3. Implement it.
4. Write tests and verify they pass.
5. Change it to `[x]` (done).
6. Commit with a message referencing the checklist item number (e.g., `feat(server): implement 1A.2 - SQLite state store`).

### Reporting Progress

At the end of your session, update `CHECKLIST.md` with whatever you completed. The next agent picks up from there.

### Stuck or Blocked?

If a task depends on something not yet implemented:
- Mark it `[!]` with a note explaining what's blocking it.
- Move on to a different task.

---

## 13. Commit Conventions

```
<type>(<scope>): <short description>

<optional body>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
Scopes: `core`, `proto`, `transport`, `server`, `client`, `docs`, `workspace`

Examples:
- `feat(server): implement pairing token auth flow`
- `fix(core): reject manifests with empty app names`
- `test(server): add integration test for deploy flow`
- `docs(workspace): update ARCHITECTURE.md with new crate layout`

No need to be obsessive — but make it clear what changed.

---

## 14. Running the Binaries

### Server (`roverd`)

```bash
cargo run -p rover-server -- --mode lan --port 9050
```

Starts the gRPC server on the specified port. Prints LAN IPs and the pairing token.

### Client (`rover`)

```bash
cargo run -p rover-client
```

Opens the Iced desktop GUI.

---

## 15. Target Platform Notes

### Server Target: `aarch64-unknown-linux-gnu` (Android/Termux)

- Termux provides a Linux userland on Android.
- No root required.
- The server is a statically linked binary where possible.
- SQLite uses `bundled` feature (no system libsqlite3 needed on the target).

### Client Targets: macOS, Windows, Linux

- Iced cross-compiles to all three.
- macOS: Tested on aarch64 (Apple Silicon).
- Native file dialogs via `rfd`.

---

## 16. Philosophy

1. **Make it work, make it right, make it fast** — in that order.
2. **Simple over clever.** Prefer straightforward solutions. A flat function is better than a clever macro. A plain struct is better than a generic tower of traits. Complexity must earn its keep.
3. **Document as you go.** Every module gets a doc comment explaining what it does and why it exists. Every public function gets a doc comment.
4. **Ship incrementally.** Phase 0 compiles and has passing tests. Phase 1A adds deploy. Phase 1B adds the UI. Each phase is a usable increment.
5. **Don't break the build.** If `cargo check` was passing before you started, it must pass when you're done.
6. **Respect the abstraction boundaries.** Don't put server logic in the client crate. Don't put UI logic in core. The crate dependency graph in `ARCHITECTURE.md` §3 is the law.

---

*End of AGENTS.md — now go read `ARCHITECTURE.md` and `CHECKLIST.md` and get to work.*
