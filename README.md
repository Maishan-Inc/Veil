# Veil

**Veil** is a fully offline, local-first, open-source **browser manager** for Windows.

Create and run many isolated browser profiles — each with its own fingerprint, proxy,
cookies, and extensions — without an account, without the cloud, and without sending
your data anywhere. Everything lives on your machine.

> Status (2026-10-17): the **management layer is broadly built** — profiles, proxies, fingerprint
> configuration, extensions, cookies, a loopback automation API, AI-driven automation, and an encrypted
> vault with master password / TOTP / recovery phrase. The **fingerprint capability is live**: the engine
> manifest declares the `veil-patched` flag dialect and points at `Veil-Chromium` releases
> (`151.0.7922.137-1.1001` and up) carrying Veil's patch set — every spoofed surface measured per patch
> rather than assumed from a clean apply. What is missing now is **packaging and signing**: no CI can
> finish a Chromium build yet (engine assets are built locally and uploaded by an upload-only workflow),
> and the Windows installers are unsigned alpha builds.



## Principles

- **Offline & local-first** — every core feature works with no network.
- **Private by default** — no telemetry, no accounts, no cloud binding. Local data is encrypted at rest.
- **Open source** — MIT licensed.
- **Windows-first** — built for Windows 10/11 with a native-feeling, Fluent-inspired UI.
- **Don't reinvent the engine** — Veil *manages* a fingerprint-capable Chromium engine through an
  adapter; it never reimplements fingerprinting or browser-core logic.

## What's online vs offline

| Always offline (core) | Online only (optional, user-initiated) |
| --- | --- |
| Create / edit / launch profiles | Download the browser engine runtime |
| Fingerprint, proxy, cookies, extensions config | Check for updates (can be disabled) |
| Local API / automation | Fetch GitHub Releases metadata |
| Cookie, credential and vault storage | Import an extension from the Chrome Web Store |
| Everything the UI does with stored data | AI automation, when you configure a provider and enable it |

Veil never contacts the network in the background: every row on the right needs an explicit action from
you, and the AI lane is inert until you supply a provider and turn network access on. Veil keeps working
with no internet.


## Tech stack

- **Shell:** [Tauri v2](https://v2.tauri.app/) (Rust) + WebView2
- **UI:** React 18 + TypeScript (strict) + Vite, bespoke Fluent-inspired design system
- **Backend:** Rust workspace — services, SQLCipher-encrypted database, Windows DPAPI, engine bridge
- **Monorepo:** pnpm workspaces (TS) + Cargo workspace (Rust)

```
apps/desktop          Tauri app: React frontend (src/) + Rust backend (src-tauri/)
apps/automation-worker  Optional AI automation worker, driven over a local CDP endpoint
crates/core           Domain types + stable error codes
crates/database       SQLCipher-encrypted local persistence
crates/secure-storage Windows DPAPI + master-password key derivation
crates/bridge         Engine launch adapter (UI never touches Chromium directly)
crates/runtime        Engine runtime provider / installer / registry
packages/shared       Shared TS contracts (mirror Rust types)
packages/i18n         en-US + zh-CN (parity pair) + partial zh-TW / ja-JP / ru-RU
packages/ui           Design tokens + UI primitives
```

## Development

Quick start:

```powershell
pnpm install
pnpm test
pnpm typecheck
cargo test
pnpm desktop:dev
```

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the ground rules and the checks to run before a PR.

The engine's fingerprint surface is measured, not assumed: the harness under
[`tools/detection-baseline/`](tools/detection-baseline/README.md) captures what a page can read and
scores it against the assertions the patch set has to satisfy.

## License

[MIT](LICENSE)
