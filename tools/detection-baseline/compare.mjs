#!/usr/bin/env node
/**
 * Cross-run snapshot comparator — the half of assertion 3 that no single capture can score.
 *
 * `lib/assertions.mjs` deliberately reports `3-seed-determines-the-fingerprint` as `not-run` whenever
 * `--fingerprint=` reached the engine: one snapshot cannot establish that the same seed reproduces and
 * a different seed diverges. That needs three captures, and therefore a tool that lives outside the
 * capture. This is it.
 *
 *   node tools/detection-baseline/compare.mjs <a.json> <b.json> [--expect same|differ] [--list]
 *
 * Every differing leaf lands in exactly one of three buckets, so nothing is silently dropped:
 *
 * - **surface** — under `realms.`, not listed in the snapshot's own `expectedToVary`. The measurement.
 *   `--expect same` requires this to be empty; `--expect differ` requires it not to be.
 * - **volatile** — matches an `expectedToVary` entry. Movement here is documented run-to-run noise
 *   (`tools/detection-baseline/README.md` → Reproducibility), not a finding.
 * - **provenance** — the labels around the measurement: `engine`, `case`, `launch`, `assertions`,
 *   `harness`, `fixtureOrigins`. Two runs at different seeds *must* differ here; it proves the two
 *   files are the comparison the caller thinks they are.
 *
 * Exit code is 1 when `--expect` is given and the verdict contradicts it, so this is usable as a gate
 * and not only as a report. No dependencies, Node >= 22.4, same as the capture.
 */

import { readFile } from "node:fs/promises";

const COMPARABLE = new Set(["string", "number", "boolean"]);
const PROVENANCE = /^(snapshotVersion|engine|harness|case|launch|fixtureOrigins|assertions)(\.|$)/;

/** Same leaf rule as `lib/assertions.mjs`: a leaf is null or a primitive; arrays index by position. */
function flatten(value, prefix = "", out = new Map()) {
  if (value === null || COMPARABLE.has(typeof value)) {
    out.set(prefix, value);
    return out;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => flatten(item, `${prefix}.${index}`, out));
    return out;
  }
  if (typeof value === "object") {
    for (const [key, inner] of Object.entries(value)) {
      flatten(inner, prefix ? `${prefix}.${key}` : key, out);
    }
  }
  return out;
}

/**
 * The snapshot's `expectedToVary` paths are globs, not literals: `*` stands for one path segment,
 * `{a,b}` for an alternation, and an entry matches its own subtree (`…misc.performanceMemory` covers
 * `…performanceMemory.totalJSHeapSize`). Compiled from the snapshot rather than hard-coded here, so
 * the exclusion list has exactly one home — the capture that wrote the file.
 */
function compileVolatile(entries) {
  return entries.map(({ path, why }) => {
    const source = path
      .replace(/[.+^$()|[\]\\]/g, "\\$&")
      .replace(
        /\{([^}]*)\}/g,
        (_, list) =>
          `(?:${list
            .split(",")
            .map((s) => s.trim())
            .join("|")})`,
      )
      .replace(/\*/g, "[^.]+");
    return { path, why, pattern: new RegExp(`^${source}(?:\\..*)?$`) };
  });
}

function groupOf(path) {
  // realms.<realm>.<group>.<rest>
  const parts = path.split(".");
  return parts.length >= 3 ? parts[2] : path;
}

function compare(a, b) {
  const volatile = compileVolatile(a.expectedToVary ?? []);
  const left = flatten(a);
  const right = flatten(b);
  const surface = [];
  const churn = [];
  const provenance = [];
  const structural = [];
  let comparedRealms = 0;
  for (const [path, leftValue] of left) {
    const inRealms = path.startsWith("realms.");
    if (!right.has(path)) {
      structural.push({ path, a: leftValue, b: "<absent>", inRealms });
      continue;
    }
    if (inRealms) {
      comparedRealms += 1;
    }
    const rightValue = right.get(path);
    if (leftValue === rightValue) {
      continue;
    }
    const hit = volatile.find(({ pattern }) => pattern.test(path));
    if (hit) {
      churn.push({ path, a: leftValue, b: rightValue, why: hit.why });
    } else if (PROVENANCE.test(path)) {
      provenance.push({ path, a: leftValue, b: rightValue });
    } else {
      surface.push({ path, a: leftValue, b: rightValue });
    }
  }
  for (const [path, rightValue] of right) {
    if (!left.has(path)) {
      structural.push({ path, a: "<absent>", b: rightValue, inRealms: path.startsWith("realms.") });
    }
  }
  return { surface, churn, provenance, structural, comparedRealms };
}

function short(value, width = 44) {
  const text = value === null ? "null" : String(value);
  return text.length > width ? `${text.slice(0, width - 1)}…` : text;
}

function label(snapshot) {
  const seed = (snapshot.launch.injectedArgs ?? []).join(" ") || "<none injected>";
  return (
    `${snapshot.engine.version} / ${snapshot.case.name}\n` +
    `    injected ${seed}\n` +
    `    exe sha256 ${snapshot.engine.executableSha256.slice(0, 16)}…  captured ${snapshot.capturedAt}`
  );
}

function tally(rows) {
  const counts = new Map();
  for (const row of rows) {
    const key = groupOf(row.path);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()].sort((x, y) => y[1] - x[1] || x[0].localeCompare(y[0]));
}

function printRows(title, rows, limit) {
  if (rows.length === 0) {
    return;
  }
  process.stdout.write(`\n${title}\n`);
  for (const row of rows.slice(0, limit)) {
    process.stdout.write(`  ${row.path}\n      a ${short(row.a)}\n      b ${short(row.b)}\n`);
  }
  if (rows.length > limit) {
    process.stdout.write(`  … ${rows.length - limit} more (pass --list to print all)\n`);
  }
}

async function main() {
  const argv = process.argv.slice(2);
  const list = argv.includes("--list");
  const expectIndex = argv.indexOf("--expect");
  const expect = expectIndex === -1 ? null : argv[expectIndex + 1];
  // `--expect same` puts a bare word on the line that is not a file; drop it by position rather than by
  // value, so a snapshot literally named `same.json` still works. Guarded on `expectIndex === -1`,
  // because `-1 + 1` is 0 and would silently swallow the first file when `--expect` is absent.
  const expectValueIndex = expectIndex === -1 ? -1 : expectIndex + 1;
  const files = argv.filter((arg, index) => !arg.startsWith("--") && index !== expectValueIndex);
  if (files.length !== 2 || (expect !== null && !["same", "differ"].includes(expect))) {
    throw new Error("usage: compare.mjs <a.json> <b.json> [--expect same|differ] [--list]");
  }
  const [a, b] = await Promise.all(files.map(async (f) => JSON.parse(await readFile(f, "utf8"))));
  const { surface, churn, provenance, structural, comparedRealms } = compare(a, b);

  const structuralRealms = structural.filter((row) => row.inRealms);
  const moved = surface.length + structuralRealms.length;
  process.stdout.write(`a  ${label(a)}\nb  ${label(b)}\n\n`);
  process.stdout.write(`realms leaves compared   ${comparedRealms}\n`);
  process.stdout.write(`surface differences      ${surface.length}\n`);
  process.stdout.write(`  structural, in realms  ${structuralRealms.length}\n`);
  process.stdout.write(`volatile (expectedToVary) ${churn.length}\n`);
  process.stdout.write(`provenance differences   ${provenance.length}\n`);

  if (moved > 0) {
    process.stdout.write("\nsurface movement by group\n");
    for (const [group, count] of tally([...surface, ...structuralRealms])) {
      process.stdout.write(`  ${group.padEnd(14)}${count}\n`);
    }
  }
  const limit = list ? Number.POSITIVE_INFINITY : 8;
  printRows("surface differences", surface, limit);
  printRows("structural differences (leaf on one side only)", structural, limit);
  printRows("volatile differences", churn, limit);
  printRows("provenance differences", provenance, limit);

  const verdict = moved === 0 ? "same" : "differ";
  process.stdout.write(`\nverdict: surface ${verdict.toUpperCase()}`);
  if (expect) {
    const ok = verdict === expect;
    process.stdout.write(`   expected ${expect} — ${ok ? "OK" : "MISMATCH"}\n`);
    if (!ok) {
      process.exitCode = 1;
    }
  } else {
    process.stdout.write("\n");
  }
}

main().catch((error) => {
  process.stderr.write(`compare: ${error.stack || error.message}\n`);
  process.exitCode = 1;
});
