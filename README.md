# Catalyst

**Catalyst is a desktop library companion for Steam.** It brings a Steam library, local-install status, collections, game details, and common game-management actions into one focused Tauri application.

Built with a TypeScript/Vite frontend and a Rust backend, Catalyst keeps its local state in SQLite and talks to Steam services only for the features that need them.

## What it does

- Sign in with Steam and synchronize a game library.
- Scan local Steam installations and show installed games and download activity.
- Browse game metadata, reviews, DLC, achievements, trading cards, screenshots, and friends activity when available.
- Organize games with favorites and custom collections, including Steam collection import.
- Start games and perform Steam-backed actions such as install, uninstall, verify, back up, and open installed files.
- Manage per-game settings including language, compatibility tools, privacy settings, artwork, and installation details.

## Prerequisites

- [Node.js](https://nodejs.org/) 20 or later and npm
- The [Rust toolchain](https://www.rust-lang.org/tools/install), including `cargo`
- The platform prerequisites required by [Tauri v2](https://v2.tauri.app/start/prerequisites/)

Platform notes:

- **Windows:** Install Microsoft C++ Build Tools (MSVC). WebView2 is normally already present on Windows 11.
- **Linux:** Install your distribution's WebKitGTK/Tauri build dependencies before building.
- **macOS:** Follow Tauri's macOS prerequisites and use Xcode Command Line Tools.

## Get started

From the repository root, enter the application directory and install its dependencies:

```bash
cd catalyst
npm install
npm run tauri dev
```

The development command starts Vite and launches the native Catalyst window. To run only the browser frontend:

```bash
npm run dev
```

On Windows PowerShell systems where script execution blocks npm shims, use `npm.cmd`:

```powershell
npm.cmd install
npm.cmd run tauri dev
```

## Steam configuration

Steam sign-in does not require configuration, but set `STEAM_API_KEY` in the shell session that starts Catalyst to enable Steam Web API-backed library synchronization and richer Steam data.

Local installation detection is enabled by default. These optional environment variables help with non-standard Steam installations:

| Variable | Purpose |
| --- | --- |
| `STEAM_LOCAL_INSTALL_DETECTION` | Enables or disables local-install detection (`true` by default). |
| `STEAM_ROOT_OVERRIDE` | Uses a specific Steam installation directory. |
| `STEAM_SETTINGS_DEBUG_LOGGING` | Enables diagnostic logging for Steam settings behavior. |

Steam authentication returns through a local loopback callback. On Windows, allow local/private network access if Windows Firewall asks for permission.

## Build and verification

Create a production bundle:

```bash
npm run tauri build
```

The current Tauri bundle configuration targets a Windows MSI installer. The project also provides repeatable checks for the frontend, Rust backend, command surface, security policy, and architectural boundaries:

```bash
npm run phase0:guardrails
```

For narrower checks, use `npm run build`, `npm run smoke`, `npm run guard:architecture`, or `npm run inventory:commands:check`.

## Architecture

```text
catalyst/
├── src/                 TypeScript UI, stores, components, and IPC client
├── src-tauri/
│   └── src/
│       ├── domain/      Core game and collection models
│       ├── application/ Use cases, services, contracts, and ports
│       ├── infrastructure/ Steam, SQLite, filesystem, and runtime adapters
│       └── interface/   Tauri command handlers
├── scripts/             Architecture, CSP, and command-inventory checks
├── docs/adr/            Architecture decision records
└── tools/               Supporting developer tooling
```

The frontend invokes a typed Tauri IPC boundary. Rust command handlers delegate to application services, whose ports are implemented by Steam, SQLite, and runtime adapters. The command inventory is generated at [`catalyst/docs/command-inventory.md`](catalyst/docs/command-inventory.md) and checked for drift in CI-style local verification.

Runtime data is stored in Catalyst's Tauri app-data directory:

- `catalyst.db` — local SQLite database
- `session.token` — persisted local session token

## Contributing

Contributions are welcome. Before opening a pull request:

1. Keep changes scoped and preserve the boundaries documented in [`catalyst/docs/adr`](catalyst/docs/adr).
2. Run `npm run phase0:guardrails` from `catalyst/`.
3. If you add, remove, or register a Tauri command, run `npm run inventory:commands` and include the generated inventory update.
4. Describe user-visible behavior and the verification performed in the pull request.

Please do not commit local `.env` files, generated build output, or application data.

## License

Catalyst is available under the [MIT License](LICENSE).
