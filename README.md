# Rover

> Turn an Android phone into a tiny PaaS, managed from a native desktop app.

Rover lets you deploy, manage, and observe applications running on Android phones
(via Termux) using a cross-platform desktop GUI. Think of it as a personal,
self-hosted alternative to Heroku or Fly.io — but the "cloud" is the phone in
your pocket.

## Architecture

```
Desktop Client (Iced) ──gRPC──► Android Phone (Termux) running roverd
     macOS / Windows / Linux        aarch64, no root required
```

- **Server (`roverd`)**: Runs on Termux. Manages app processes, stores state in SQLite, exposes gRPC API.
- **Client (`rover`)**: Native desktop app (Iced). Connects to one or more servers, deploys apps, streams logs.
- **Protocol**: gRPC with Protobuf. Auth via pairing token → persistent API key.
- **Transport**: Local LAN (V1), relay (V2 planned).

## Project Structure

```
rover/
├── proto/                        Protobuf definitions
├── crates/
│   ├── rover-core/               Shared types, manifest parser, profiles
│   ├── rover-proto/              Compiled protobuf code
│   ├── rover-transport/          Transport trait definitions
│   ├── rover-transport-lan/      V1: Raw TCP transport
│   ├── rover-transport-relay/    V2: Relay transport (stub)
│   ├── rover-server/             Server daemon (roverd)
│   └── rover-client/             Desktop GUI (rover)
├── ARCHITECTURE.md               Full architecture documentation
├── CHECKLIST.md                  Development checklist
└── README.md                     This file
```

## Prerequisites

### Server (Android/Termux)

Install Termux from F-Droid (Google Play version is outdated), then:

```bash
pkg update && pkg upgrade -y
pkg install python3 openssl
```

### Desktop Client

- Rust toolchain (latest stable)
- Platform-specific build dependencies for Iced (see [Iced requirements](https://github.com/iced-rs/iced))

## Quick Start

### One-Command Install (Android/Termux)

From your Android phone running Termux:

```bash
curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/refs/heads/main/scripts/install.sh | bash
```

This installs dependencies, clones the repo, builds the server, and launches it — all in one command.

To use a custom port:

```bash
curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/refs/heads/main/scripts/install.sh | bash -s -- 8080
```

### Update & Relaunch (Android/Termux)

When you've pushed new code and want to rebuild and restart:

```bash
curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/refs/heads/main/scripts/update.sh | bash
```

This pulls the latest commit, rebuilds, and relaunches.

### 1. Start the Server

On your Android phone (Termux):

```bash
./roverd --mode lan --port 9050
```

The server will print:
```
[Rover] Pairing token: rover-pair-abc123def456
[Rover]   -> Available at: http://192.168.1.42:9050
[Rover] gRPC server listening on 0.0.0.0:9050
```

### 2. Connect the Desktop Client

Launch the desktop app. On the Connections screen:
1. Enter `192.168.1.42:9050` as the address
2. Enter the pairing token
3. Click Connect

### 3. Deploy an App

1. Navigate to the Deploy screen
2. Fill in the form (name, runtime, commands)
3. Select your app's source directory
4. Click Deploy

The build output streams in real-time. Once deployed, manage it from the Dashboard.

## Development

See `ARCHITECTURE.md` for the full design document and roadmap.
See `CHECKLIST.md` for detailed implementation tracking.

```bash
# Build everything
cargo build

# Run tests
cargo test

# Build server only
cargo build -p rover-server

# Build client only
cargo build -p rover-client
```

## License

MIT
