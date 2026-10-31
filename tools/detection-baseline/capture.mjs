#!/usr/bin/env node
/**
 * Detection baseline capture — layer 1 of Stage 3 (`tools/detection-baseline/README.md`).
 *
 * Attaches to an already-running engine over CDP, opens the loopback fixture, and writes one
 * machine-readable snapshot of the fingerprint surface. Per-release regression detection is then a
 * `git diff` of the committed JSON rather than a human re-reading detection-site scores.
 *
 * It does not launch the browser. `crates/runtime/tests/detection_baseline.rs` does that through
 * Veil's own launch path (`veil_core::to_engine_args` → `veil_bridge::launch`), and hands this tool a
 * debug port. Keeping the two apart is what lets Stage 4 move this directory into `Veil-Chromium`
 * without rewriting the measurement.
 *
 *   node tools/detection-baseline/capture.mjs \
 *     --cdp http://127.0.0.1:<port> --meta <meta.json> --out <snapshot.json>
 *
 * Requires Node >= 22.4 for the stable global `WebSocket` (see the toolchain decision in
 * tools/detection-baseline/README.md). No dependencies.
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { CdpClient, EXPR_RESULT, EXPR_STATUS } from "./lib/cdp.mjs";
import { startFixtures } from "./lib/fixtures.mjs";
import { evaluateAssertions } from "./lib/assertions.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const COLLECT_TIMEOUT_MS = 120000;

/**
 * Fields whose value legitimately changes between two runs of the same engine. The reproducibility
 * claim in `tools/detection-baseline/README.md` is "two consecutive snapshots differ only in these".
 */
const EXPECTED_TO_VARY = [
  { path: "capturedAt", why: "wall-clock time of the run" },
  {
    path: "realms.*.misc.performanceMemory",
    why:
      "JS heap sizes depend on what the page happened to allocate; Chromium quantises them, so the " +
      "observed movement is a whole 262144-byte (256 KiB) step in totalJSHeapSize, in either direction",
  },
  {
    path: "realms.*.misc.storageEstimate",
    why: "quota is derived from free disk space; usage from this throwaway profile",
  },
  {
    path: "realms.*.misc.connection.{downlink,rtt}",
    why: "quantised network estimates, but still estimates",
  },
  {
    path: "realms.*.webrtc.candidates.*.addressSha256",
    why: "Chromium randomises mDNS candidate hostnames per session",
  },
  {
    path: "realms.*.intl.timezoneOffsetNow",
    why: "`new Date()` crosses DST boundaries; the fixed January/July offsets do not",
  },
];

/**
 * The fixture servers bind ephemeral ports, so every origin string would churn the diff on every
 * run for no information. Normalise them the same way the Rust launcher normalises the debug port in
 * the recorded argv.
 */
function redactPorts(value, ports) {
  if (typeof value === "string") {
    return ports.reduce((text, port) => text.split(`:${port}`).join(":<ephemeral>"), value);
  }
  if (Array.isArray(value)) {
    return value.map((item) => redactPorts(item, ports));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, inner]) => [key, redactPorts(inner, ports)]),
    );
  }
  return value;
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i];
    if (!key.startsWith("--")) {
      throw new Error(`unexpected argument: ${key}`);
    }
    args[key.slice(2)] = argv[i + 1];
  }
  for (const required of ["cdp", "meta", "out"]) {
    if (!args[required]) {
      throw new Error(`--${required} is required`);
    }
  }
  return args;
}

async function waitForCollection(client, sessionId) {
  const deadline = Date.now() + COLLECT_TIMEOUT_MS;
  let status = null;
  while (Date.now() < deadline) {
    status = await client.read(EXPR_STATUS, sessionId);
    if (status === "done" || status === "failed") {
      return status;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`fixture never finished collecting (last status: ${status})`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const meta = JSON.parse(await readFile(args.meta, "utf8"));
  const fixtures = await startFixtures(join(HERE, "pages"));
  process.stdout.write(`fixture main   ${fixtures.mainUrl}\n`);
  process.stdout.write(`fixture frame  ${fixtures.frameOrigin}/frame.html\n`);

  let rawRealms;
  let version;
  const client = await CdpClient.connect(args.cdp).then((connected) => {
    version = connected.version;
    return connected.client;
  });
  try {
    const { targetId } = await client.send("Target.createTarget", { url: fixtures.mainUrl });
    const { sessionId } = await client.send("Target.attachToTarget", { targetId, flatten: true });
    const status = await waitForCollection(client, sessionId);
    const payload = await client.read(EXPR_RESULT, sessionId);
    if (!payload) {
      throw new Error(`fixture produced no result (status: ${status})`);
    }
    rawRealms = JSON.parse(payload);
    if (rawRealms.error) {
      throw new Error(`fixture failed: ${rawRealms.error}`);
    }
    for (const realm of ["main", "iframe", "worker"]) {
      const captured = rawRealms[realm];
      if (!captured || captured.unavailable) {
        throw new Error(
          `realm "${realm}" did not report: ${captured ? captured.unavailable : "missing"}`,
        );
      }
    }
    await client.send("Target.closeTarget", { targetId }).catch(() => {});
    // Graceful shutdown so the throwaway user-data-dir is released; the Rust launcher kills the
    // process as a fallback if this does not land.
    await client.send("Browser.close").catch(() => {});
  } finally {
    client.close();
    await fixtures.close();
  }

  const realms = redactPorts(rawRealms, fixtures.ports);
  const snapshot = {
    snapshotVersion: meta.snapshotVersion,
    capturedAt: new Date().toISOString(),
    engine: meta.engine,
    harness: {
      node: process.version,
      probeVersion: realms.main.probeVersion,
      cdpBrowser: version.Browser,
      cdpProtocolVersion: version["Protocol-Version"],
      fixtureModel: "loopback: main on 127.0.0.1, cross-site frame on localhost, module Worker",
    },
    case: meta.case,
    launch: meta.launch,
    fixtureOrigins: redactPorts(
      { main: fixtures.mainOrigin, frame: fixtures.frameOrigin },
      fixtures.ports,
    ),
    realms,
    assertions: evaluateAssertions({
      realms,
      caseMeta: meta.case,
      launch: meta.launch,
      engine: meta.engine,
    }),
    expectedToVary: EXPECTED_TO_VARY,
  };

  await mkdir(dirname(args.out), { recursive: true });
  await writeFile(args.out, `${JSON.stringify(snapshot, null, 2)}\n`, "utf8");
  printSummary(snapshot, args.out);
}

function printSummary(snapshot, outPath) {
  const main = snapshot.realms.main;
  const uaData = main.navigator.userAgentData;
  const rows = [
    [
      "engine",
      `${snapshot.engine.name} ${snapshot.engine.version} (${snapshot.engine.flagDialect})`,
    ],
    ["case", `${snapshot.case.name}, proxy ${snapshot.case.proxyConfigured ? "set" : "none"}`],
    ["navigator.userAgent", main.navigator.userAgent],
    ["navigator.platform", main.navigator.platform],
    ["UA-CH platform", uaData ? `${uaData.platform} ${uaData.highEntropy.platformVersion}` : "n/a"],
    ["screen", `${main.screen.width}x${main.screen.height} @${main.screen.devicePixelRatio}`],
    ["window inner", `${main.screen.innerWidth}x${main.screen.innerHeight}`],
    ["hardwareConcurrency", String(main.navigator.hardwareConcurrency)],
    ["deviceMemory", String(main.navigator.deviceMemory)],
    ["Intl timeZone", main.intl.dateTimeFormat.timeZone],
    ["WebGL renderer", String(main.webgl.unmaskedRenderer)],
    ["canvas toDataURL", String(main.canvas.toDataUrlSha256).slice(0, 16)],
    ["audio render", String(main.audio.render.sha256).slice(0, 16)],
    ["fonts present", `${main.fonts.presentCount}/${main.fonts.probed}`],
  ];
  process.stdout.write("\n");
  for (const [label, value] of rows) {
    process.stdout.write(`${label.padEnd(22)}${value}\n`);
  }
  process.stdout.write("\nassertions\n");
  for (const [name, assertion] of Object.entries(snapshot.assertions)) {
    process.stdout.write(`  ${assertion.status.padEnd(13)}${name}\n`);
  }
  process.stdout.write(`\nsnapshot written: ${outPath}\n`);
}

main().catch((error) => {
  process.stderr.write(`detection-baseline: ${error.stack || error.message}\n`);
  process.exitCode = 1;
});
