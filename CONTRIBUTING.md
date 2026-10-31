# Contributing to Veil

Thanks for your interest! Veil is an offline, local-first, open-source browser manager.

## Ground rules

- Read [`README.md`](README.md) first: it states the scope, the offline contract, and the layout.
- Respect the priority order: **Security > Stability > Compatibility > Performance > Features**.
- Keep the **offline contract**: core features must work with no network.
- Never reimplement fingerprinting/browser-core; wrap the engine via `crates/bridge`.
- All user-facing strings go through `@veil/i18n` (keep `en-US` and `zh-CN` in parity).
- Keep business logic out of the View.

## Setup

See the quick start in [`README.md`](README.md).

## Reporting bugs

Open an issue with reproduction steps and versions, review screenshots before attaching them, and
never upload the Veil data directory or secrets. The Logs page shows AI activity; a general
diagnostic bundle is not available yet.

## Before you open a PR

```powershell
pnpm typecheck
pnpm test
cargo test
cargo clippy --all-targets --all-features
pnpm format:check
```

- Add tests for new behavior.
- Keep doc comments and this file current when you change behavior or architecture.
- Keep commits focused and messages descriptive (Conventional Commits style preferred,
  e.g. `feat: add proxy test command`).

## Originality

Do not copy icons, images, color palettes, fonts, assets, or source code from other products. Veil has
its own visual identity.

## License

By contributing you agree your contributions are licensed under the [MIT License](LICENSE).
