# things-api menu bar app

Swift wrapper that bundles the `things-api` Rust binary and provides a native macOS
menu bar UI on top.

```
ThingsAPIMenuBar.app/
├── Contents/
│   ├── Info.plist                  ← LSUIElement=true (no Dock icon)
│   ├── MacOS/
│   │   ├── ThingsAPIMenuBar        ← Swift menu bar app
│   │   └── things-api              ← Rust server binary (child process)
│   └── Resources/
```

The Swift app spawns `things-api` as a child process and watches it via `/health` polling.
Both processes share state by reading/writing the same files in
`~/Library/Application Support/things-api/`:

- `auth_token`   — single-line bearer used to authenticate API calls
- `account.json` — signed-up username, public URL, and Cloudflare Tunnel token

## Building

```sh
make            # build the .app bundle into build/ThingsAPIMenuBar.app
make run        # build and launch it
make clean      # wipe build artifacts
```

The Makefile invokes both `cargo build --release -p things-api` (in the parent crate) and
`swift build -c release`, then assembles the bundle. Requires Xcode command-line tools.

## Distribution

You need an **Apple Developer Program** membership (USD $99/yr) for code signing
and notarization, without which macOS will refuse to launch the `.app`.

```sh
# One-time: store your Apple ID + app-specific password under a keychain profile
xcrun notarytool store-credentials "things-api-notary" \
    --apple-id you@example.com \
    --team-id YOURTEAMID \
    --password "app-specific-password-from-appleid.apple.com"

# Build + sign + notarize
make notarize \
    CODESIGN_IDENTITY="Developer ID Application: Your Name (YOURTEAMID)" \
    NOTARY_PROFILE=things-api-notary
```

After notarization, `build/ThingsAPIMenuBar.app` is fully gatekeeper-friendly.
Distribute via `.dmg` (use `hdiutil create`) on GitHub Releases, or publish a
Homebrew Cask formula.

## Architecture notes

- **MenuBarExtra**: `App.swift` uses SwiftUI's `MenuBarExtra` (macOS 13+). Icon
  reflects the `ServerManager` state machine via SF Symbols.
- **No IPC protocol**: the Swift app reads the same config files the Rust binary
  writes, and probes the Rust server's existing `/health` endpoint. No new
  protocol surface.
- **Signup**: `SignupView` POSTs directly to the control plane via `URLSession`,
  then writes `account.json`. Restarts the server so `cloudflared` picks up the
  new tunnel token.
- **Launch at Login**: `SMAppService.mainApp` registers the bundle as a login
  item. Requires the app to be signed and located in `/Applications`.

## Things 3 permissions

Two prompts that always appear (we cannot pre-authorize):

1. **Automation** — first call to AppleScript triggers System Settings →
   Privacy & Security → Automation. User must allow `things-api` → `Things3`.
2. **Things URLs** — manually toggle "Enable Things URLs" in Things 3 →
   Settings → General. There is no API to flip this.

The app surfaces a guidance modal when AppleScript calls fail with `-1743`
(not authorized).

## Files

| Path | What it does |
|---|---|
| `Package.swift` | Swift Package manifest (macOS 13+, single executable target) |
| `Info.plist` | Bundle metadata; `LSUIElement=true` makes it menu-bar-only |
| `Makefile` | Builds the `.app`, signs it, notarizes it |
| `Sources/.../App.swift` | App entry, `MenuBarExtra` root, window definitions |
| `Sources/.../AccountStore.swift` | Reads/writes `auth_token` + `account.json` |
| `Sources/.../ServerManager.swift` | Spawns + monitors the `things-api` subprocess |
| `Sources/.../Models.swift` | Codable types matching the Rust side + control plane |
| `Sources/.../MenuView.swift` | The dropdown menu items |
| `Sources/.../SignupView.swift` | First-time signup form |
| `Sources/.../SettingsView.swift` | Port, launch-at-login, account, files |
