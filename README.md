# r3-UI - Xray Proxy Panel (Rust Edition)

A high-performance Xray proxy panel rewritten in Rust using Axum + SQLx.

## Features

- 🚀 **Fast & Lightweight** - Written in Rust, ~5MB binary size
- 🔐 **Secure Authentication** - Argon2 password hashing, session management
- 📊 **Real-time Monitoring** - WebSocket-based traffic statistics
- ⚙️ **Xray Integration** - Process management, config generation, log streaming
- 🔄 **Traffic Reset** - Daily/weekly/monthly automatic reset
- 📱 **Modern UI** - Ant Design Vue frontend
- 📋 **Subscription Support** - Generate client configs for mobile apps

## Tech Stack

| Component | Technology |
|-----------|------------|
| Web Framework | Axum 0.8 |
| Database | SQLx + SQLite |
| Async Runtime | Tokio |
| Serialization | Serde |
| Sessions | tower-sessions |
| WebSocket | axum::ws |
| Password Hashing | Argon2 |
| Logging | tracing |
| Scheduled Tasks | tokio-cron-scheduler |

## Project Structure

```
r3-UI/
├── Cargo.toml
├── .env                    # Environment configuration
├── src/
│   ├── main.rs             # Application entry point
│   ├── lib.rs              # Library exports
│   ├── app_state.rs        # Global state
│   ├── config/             # Configuration management
│   ├── db/                 # Database operations
│   ├── models/             # Data models
│   ├── handlers/           # API handlers
│   ├── services/           # Business logic
│   ├── middleware/         # HTTP middleware
│   ├── xray/               # Xray integration
│   ├── websocket/          # Real-time communication
│   ├── scheduler/          # Cron jobs
│   └── utils/              # Helper functions
└── web/html/               # Frontend (Vue + Ant Design)
```

## API Endpoints

### Authentication
- `POST /api/login` - Login
- `POST /api/logout` - Logout
- `GET /api/is_logged` - Check auth status

### Inbounds
- `GET /api/panel/api/inbounds` - List all inbounds
- `POST /api/panel/api/inbounds` - Create inbound
- `POST /api/panel/api/inbounds/update` - Update inbound
- `POST /api/panel/api/inbounds/del/:id` - Delete inbound
- `GET /api/panel/api/inbounds/traffic` - Traffic stats

### Xray
- `GET /api/panel/api/xray/status` - Get status
- `POST /api/panel/api/xray/restart` - Restart process
- `GET /api/panel/api/xray/logs` - Get logs

### Settings
- `GET /api/panel/api/setting/all` - Get all settings
- `POST /api/panel/api/setting/update` - Update setting

### WebSocket
- `GET /ws` - WebSocket connection for real-time updates

### Subscription
- `GET /sub/:token` - Get subscription config

## Quick Start

### Prerequisites
- Rust 1.70+
- Xray-core binary

### Build
```bash
cargo build --release
```

### Configure
```bash
cp .env.example .env
# Edit .env with your settings
```

### Run
```bash
./target/release/r3_ui
```

Default credentials: `admin` / `admin`

## Configuration

Edit `.env` file:

```env
HOST=0.0.0.0
PORT=2053
DATABASE_URL=sqlite:data.db?mode=rwc
SESSION_SECRET=your-secret-key
XRAY_BINARY=/usr/local/bin/xray
XRAY_CONFIG=/etc/xray/config.json
RUST_LOG=info
```

## Supported Protocols

- VMess
- VLESS
- Trojan
- Shadowsocks
- SOCKS
- HTTP

## Supported Transports

- TCP
- WebSocket
- HTTP/2
- gRPC
- QUIC

## Security Features

- Argon2id password hashing
- Session-based authentication
- TLS/Reality support
- Sniffing & routing rules

## Development

```bash
# Run in development mode
RUST_LOG=debug cargo run

# Run tests
cargo test

# Check for errors
cargo clippy
```

## License

MIT License

## Credits

- Original 3x-ui project: https://github.com/MHSanaei/3x-ui
- Xray-core: https://github.com/XTLS/Xray-core
- Axum: https://github.com/tokio-rs/axum
