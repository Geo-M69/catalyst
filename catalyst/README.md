# Catalyst

Catalyst is a Tauri desktop app (Vanilla TS frontend + Rust backend commands).

Authentication, Steam linking, session management, and library sync are implemented as Tauri commands in `src-tauri/src/lib.rs`.

## Prerequisites

- Node.js 20+ and npm
- Rust toolchain (`rustup`, `cargo`)
- Tauri system prerequisites

### Windows

- Microsoft Visual Studio C++ Build Tools (MSVC)
- WebView2 Runtime (usually preinstalled on Windows 11)
- If PowerShell blocks `npm` scripts (`npm.ps1` execution policy), use `npm.cmd` commands instead.

### Linux

- Build dependencies required by Tauri/WebKitGTK for your distro

## Local Development

```bash
npm install
npm run tauri dev
```

Windows PowerShell fallback:

```powershell
npm.cmd install
npm.cmd run tauri dev
```

Optional one-session policy bypass:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
```

You can also run frontend-only mode:

```bash
npm run dev
```

## Steam API Key (Optional)

Steam login works without syncing games, but game sync requires `STEAM_API_KEY`.

### PowerShell (Windows)

```powershell
$env:STEAM_API_KEY="your_key_here"
npm run tauri dev
```

### Bash (Linux)

```bash
export STEAM_API_KEY="your_key_here"
npm run tauri dev
```

## Production Build

```bash
npm run tauri build
```

On Windows, this produces an MSI installer by default (see `src-tauri/tauri.conf.json` bundle settings).

## Steam Login Note (Windows)

Steam sign-in uses a loopback callback (`127.0.0.1`) to return auth data to Catalyst.
On first use, Windows Firewall may prompt for Catalyst network permission; allow local/private access so the callback can complete.

## Data Storage

Runtime data is stored in the Tauri app data directory:

- `catalyst.db` (SQLite database)
- `session.token` (persisted local session token)
