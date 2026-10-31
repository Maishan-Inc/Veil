# detection-baseline

Layer 1 of Veil's engine verification: a machine-readable snapshot of the browser's fingerprint
surface, captured over CDP into JSON so per-release regression detection is a `git diff`.

This directory is the harness. It is **dev-only, read-only and offline** by construction (see
*Rules* below), and nothing in the app or CI depends on it.

## Directory map

```
capture.mjs                 entry point: attach, open the fixture, write the snapshot
compare.mjs                 cross-run comparator: scores the half of assertion 3 no single capture can
recheck.mjs                 scores assertion 6 against snapshots already captured, no re-capture
lib/cdp.mjs                 minimal CDP client, read-only by construction (method + expression allowlists)
lib/fixtures.mjs            the two loopback origins that host the probe
lib/assertions.mjs          the six non-negotiable assertions, scored from a capture
lib/coherence.mjs           assertion 6's rules: does the persona describe a machine that could exist
pages/probe.mjs             the value collector — one module, run in all three realms
pages/{main,frame,worker}.* per-realm drivers
cases/*.json                the machine each snapshot declares, and the fingerprint it launches with
snapshots/<engine>/*.json   baselines captured locally (not committed — see below)
```

## Where the measurements live

The harness and the case definitions are public; **baseline captures are not committed**. Each
maintainer populates `snapshots/` locally (the directory is gitignored), so captures stay on the
machine that made them and no host profile ships in this repository. Everything below still works
exactly as described — the per-release comparison just runs against a local corpus instead of a
committed one.

## The six assertions

One capture scores six non-negotiable assertions (`lib/assertions.mjs`):

| # | Assertion | One line |
|---|---|---|
| 1 | Patched getters return `[native code]` | a JS-level spoof self-reveals via `Function.prototype.toString`; patch level only |
| 2 | Main frame, cross-site iframe and Worker agree | the same value must appear in all three realms, leaf by leaf |
| 3 | Same seed reproduces; different seeds differ | one capture cannot show this — scored across runs with `compare.mjs` |
| 4 | No WebRTC candidate reveals the real IP with a proxy set | a candidate address must be the proxy exit, never the host |
| 5 | `Intl` timezone matches the proxy exit country | the claimed zone must follow the proxy, not the host |
| 6 | The persona describes a machine that could exist | rules in `lib/coherence.mjs` catch self-contradicting claims before users do |

## Running it

The capture tool does not launch a browser. `crates/runtime/tests/detection_baseline.rs` does, through
Veil's own launch path, and hands this tool a debug port:

```bash
cargo test -p veil-runtime --test detection_baseline -- --ignored --exact captures_detection_baseline --nocapture
```

To measure a build that was never installed through Veil — a locally built `Veil-Chromium`, for
instance — set `VEIL_ENGINE_EXE` and `VEIL_ENGINE_VERSION` (the latter names the snapshot directory):

```bash
VEIL_ENGINE_EXE='H:/build/Veil-Chromium/build/src/out/Default/chrome.exe' \
VEIL_ENGINE_VERSION=151.0.7922.137-1.1000 \
  cargo test -p veil-runtime --test detection_baseline -- --ignored --exact captures_detection_baseline --nocapture
```

`capture.mjs` can also be pointed at any already-running engine with `--remote-debugging-port` open:

```bash
node tools/detection-baseline/capture.mjs --cdp http://127.0.0.1:9222 --meta meta.json --out snapshot.json
```

## Comparing two snapshots

`lib/assertions.mjs` reports assertion 3 (`seed determines the fingerprint`) as `not-run` whenever a
seed reached the engine, because one capture cannot show that the same seed reproduces and a different
seed diverges. `compare.mjs` is the other half — it diffs two snapshots leaf by leaf and sorts every
difference into `surface` (under `realms.`, a real measurement), `volatile` (matches the snapshot's own
`expectedToVary`) or `provenance` (the labels around the measurement):

```bash
node tools/detection-baseline/compare.mjs \
  snapshots/tier1-all16-seed7-a/stock-chromium-default.json \
  snapshots/tier1-all16-seed7-b/stock-chromium-default.json --expect same
```

`--expect same|differ` makes it a gate: exit 1 when the verdict contradicts the expectation. `--list`
prints every differing leaf instead of the first eight per bucket.

## Re-scoring assertion 6 over old snapshots

`capture.mjs` writes the assertion block at capture time, so a snapshot taken before assertion 6 existed
carries no verdict for it. `recheck.mjs` applies `lib/coherence.mjs` to an already-captured JSON
instead of re-running anything:

```bash
node tools/detection-baseline/recheck.mjs --display 1470x956@2 \
  snapshots/local/<version>/veil-patched-persona.json
```

It infers the claimed platform from `launch.argv`'s `--fingerprint-platform=` when the older `case` block
records no `os`, and skips the declared-panel rule unless `--display <w>x<h>[@<dpr>]` supplies one. Exit 1
if any snapshot scores `fail`, so it is a gate over a directory of captures.

## Per-release checklist

Run this for every engine bump, and for every patch in Stage 5 — one patch, one build, one baseline
run. A clean `git apply` is not proof: a patch whose hook moved to another call path still applies and
silently does nothing.

- [ ] Install the new engine; verify its SHA-256 against the GitHub asset `digest`. (For a local
      build, point `VEIL_ENGINE_EXE` + `VEIL_ENGINE_VERSION` at it instead.)
- [ ] Capture both cases into the local corpus under the new version directory.
- [ ] **Check the host first.** If the display, GPU, audio devices, locale or timezone changed since
      the previous capture, re-capture the *previous* engine as a control and diff against that.
      Diffing across a host change produces host-driven noise that reads exactly like a regression.
- [ ] Diff the previous version's captures. Every changed leaf is either an intended patch effect, a
      documented `expectedToVary` field, or a regression to explain. There is no fourth category.
- [ ] Read the `assertions` block. Any status that got *worse*, or any `pass` that became `fail`,
      blocks the release.
- [ ] **Assertion 6 must not report `fail`.** `recheck.mjs <version-dir>/*.json` scores it over the
      new directory and exits 1 on any `fail`; a `known-floor` is acceptable.
- [ ] Re-run the capture once and confirm only `expectedToVary` fields move —
      `compare.mjs <first> <second> --expect same` does this mechanically.

## Toolchain decision

CDP evaluation needs a WebSocket client, and Veil adds **no dependency** for it: the harness uses
Node's global `WebSocket`, which is stable from **Node 22.4**. `engines.node` is therefore `>=22.4.0`
and CI pins `node-version: 22`. (Node 20 reached end of life on 2026-04-30; a `ws` devDependency and
a hand-rolled RFC 6455 client were both considered and rejected.) The harness is plain `.mjs` with no
build step and sits outside the pnpm workspace (`apps/*` + `packages/*`), so `pnpm test` and
`pnpm typecheck` cannot pick it up even by accident.

## Rules

- **Read only.** No `Emulation.*`, no `Network.setUserAgentOverride`, no
  `Page.addScriptToEvaluateOnNewDocument`, no property patching. `lib/cdp.mjs` enforces this with an
  allowlist rather than trusting review, and `Runtime.evaluate` is restricted to two fixed
  expressions that read what the fixture page computed for itself. Spoofing from the automation layer
  is the anti-pattern the ungoogled migration exists to remove.
- **Dev-only.** Nothing here is reachable from the app: no Tauri command, no service, no CI job. It is
  outside the pnpm workspace (`apps/*` + `packages/*`), so `pnpm test` and `pnpm typecheck` cannot
  pick it up, and the Rust entry points are `#[ignore]`d.
- **Offline.** The fixture pages are served from loopback, so a capture makes no outbound request.
- **No dependencies.** Plain `.mjs` on Node ≥ 22.4.
