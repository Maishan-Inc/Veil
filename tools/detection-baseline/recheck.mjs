/**
 * Score assertion 6 against snapshots that are already committed.
 *
 * `capture.mjs` writes the assertion block at capture time, so a snapshot taken before assertion 6
 * existed carries no verdict for it — and there are three dozen such directories under `snapshots/`,
 * every one of them a measurement of a real engine that nobody has to re-run to audit. This is the
 * cheap half: read the JSON, apply `lib/coherence.mjs`, print the findings.
 *
 * Two things it has to infer, because the older `case` block did not record them:
 *
 * - **the claimed platform** — recovered from `launch.argv`'s `--fingerprint-platform=`, which is the
 *   switch that actually carried the claim to the engine. A snapshot with no such switch claims
 *   nothing, and the verdict is `n/a` rather than a guess.
 * - **the declared panel** — not recoverable at all, so `display-matches-the-declared-panel` simply
 *   does not run on an old snapshot. Pass `--display <w>x<h>@<dpr>` to supply one by hand.
 *
 * ```bash
 * node tools/detection-baseline/recheck.mjs snapshots/151.0.7922.137-1.1001/veil-patched-persona.json
 * node tools/detection-baseline/recheck.mjs --display 1470x956@2 snapshots/tier1-002-user-agent/*.json
 * ```
 *
 * Exits 1 if any snapshot scores `fail`, so it is usable as a gate over the whole directory.
 */

import { readFile } from "node:fs/promises";
import process from "node:process";

import { evaluateCoherence } from "./lib/coherence.mjs";

function parseDisplay(raw) {
  const match = /^(\d+)x(\d+)(?:@([\d.]+))?$/.exec(raw ?? "");
  if (!match) {
    throw new Error(`--display wants <width>x<height>[@<dpr>], got ${JSON.stringify(raw)}`);
  }
  return {
    width: Number(match[1]),
    height: Number(match[2]),
    ...(match[3] ? { devicePixelRatio: Number(match[3]) } : {}),
  };
}

function claimedOsFrom(snapshot) {
  if (snapshot.case?.os) {
    return snapshot.case.os;
  }
  const switchArg = (snapshot.launch?.argv ?? []).find((arg) =>
    arg.startsWith("--fingerprint-platform="),
  );
  return switchArg ? switchArg.slice("--fingerprint-platform=".length) : null;
}

function patchedCodeRunning(snapshot) {
  return (snapshot.launch?.argv ?? []).some(
    (arg) =>
      arg.startsWith("--fingerprint") ||
      arg.startsWith("--timezone=") ||
      arg.startsWith("--disable-spoofing="),
  );
}

function printFindings(label, entries) {
  for (const entry of entries) {
    console.log(`  [${label}] ${entry.rule} (${entry.surface})`);
    console.log(`      claim    ${entry.claim}`);
    console.log(`      observed ${entry.observed}`);
    console.log(
      `      ${entry.drivenBy ? `driven by ${entry.drivenBy}` : "no switch reaches this"}`,
    );
  }
}

/**
 * Which declared machine to score a snapshot against.
 *
 * A snapshot taken after 6.4 carries its own `case.machine`, and that is the claim the capture
 * actually made — so it wins. `--display` exists for the snapshots that predate `case.machine` and
 * can never recover it; letting it *override* a snapshot that has one made `recheck` silently skip
 * `display-matches-the-declared-panel` on exactly the snapshots best able to score it, which left the
 * release gate weaker than the capture it re-checks. Measured on the probe-v4 pair: the capture
 * reported three findings for that rule and `recheck` reported none.
 *
 * A declared panel only means something beside a declared platform, so both paths still require
 * `claimedOs` — applying a panel to a snapshot that claims no OS would invent a claim.
 */
function machineFor(snapshot, display, claimedOs) {
  if (!claimedOs) {
    return null;
  }
  const own = snapshot.case?.machine;
  if (own?.display) {
    return own;
  }
  return display ? { label: "supplied on the command line", display } : null;
}

async function main(argv) {
  const files = [];
  let display = null;
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--display") {
      display = parseDisplay(argv[index + 1]);
      index += 1;
    } else {
      files.push(argv[index]);
    }
  }
  if (files.length === 0) {
    console.error("usage: recheck.mjs [--display <w>x<h>[@<dpr>]] <snapshot.json>...");
    return 2;
  }

  let failed = 0;
  for (const file of files) {
    const snapshot = JSON.parse(await readFile(file, "utf8"));
    const claimedOs = claimedOsFrom(snapshot);
    const result = evaluateCoherence({
      realms: snapshot.realms ?? {},
      caseMeta: {
        ...(snapshot.case ?? {}),
        os: claimedOs,
        machine: machineFor(snapshot, display, claimedOs),
      },
      engine: snapshot.engine,
      launch: snapshot.launch,
      fingerprintSwitchPresent: patchedCodeRunning(snapshot),
    });
    const version = snapshot.engine?.version ?? "unknown";
    const name = snapshot.case?.name ?? "unknown";
    console.log(
      `${result.status.toUpperCase()}  ${version} / ${name} / claims ${claimedOs ?? "—"}`,
    );
    printFindings("defect", result.defects);
    printFindings("floor", result.floors);
    if (result.status === "fail") {
      failed += 1;
    }
  }
  console.log(`\n${files.length} snapshot(s), ${failed} scoring fail`);
  return failed > 0 ? 1 : 0;
}

process.exitCode = await main(process.argv.slice(2));
