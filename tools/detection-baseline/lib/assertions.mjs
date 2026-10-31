/**
 * The six non-negotiable assertions of the engine surface, evaluated
 * against a capture.
 *
 * Scoring them honestly on a stock engine matters more than scoring them well. Three of the five
 * original assertions are not yet meaningful — nothing is patched, `--fingerprint` is not even
 * emitted, and `TZ` is ignored on Windows — so they are recorded as `vacuous`, `n/a` and
 * `known-floor` with the reason attached. A green tick here that does not correspond to a real
 * guarantee would make the whole baseline worthless.
 *
 * Assertion 6 was added later, and for a measured reason: 1–5 all passed or floored on a snapshot
 * that described an Apple M3 Mac with `devicePixelRatio: 1`, two operating systems' fonts and Chinese
 * Windows speech voices. "Every surface moved" is not the same property as "the result is a machine
 * that exists", and only the second one survives a detector. See `lib/coherence.mjs`.
 */

import { evaluateCoherence } from "./coherence.mjs";

/**
 * Paths excluded from the cross-realm equality check, with the reason each one is legitimately
 * per-realm. Anything not listed here is expected to be identical in all three realms.
 */
const CROSS_REALM_EXCLUSIONS = [
  [/^realm$/, "the realm label itself"],
  [/^origin$/, "the frame is deliberately served from a second origin"],
  [
    /^screen\.(inner|outer)(Width|Height)$/,
    "window geometry is per-frame by definition: an iframe's box is not the tab's",
  ],
  [
    /^canvas\.clientRects\.(x|y)$/,
    "layout position depends on the containing document, not on the fingerprint",
  ],
  [/^misc\.performanceMemory\./, "JS heap sizes differ per realm and per run"],
  [/^misc\.storageEstimate\.usage$/, "bytes written by this throwaway profile"],
  [
    /^webrtc\.candidates\.\d+\.addressSha256$/,
    "Chromium randomises mDNS candidate hostnames per session and origin",
  ],
  // The three below were *measured* divergences on stock ungoogled-chromium 151 (main vs a
  // cross-site iframe), investigated, and found to be deliberate Chromium policy for cross-origin
  // frames rather than fingerprint-surface divergence. Excluding them keeps assertion 2 sharp; the
  // investigation is written up in tools/detection-baseline/README.md.
  [
    /^webgl\.contextAttributes\.powerPreference$/,
    "Chromium requests the low-power GPU for cross-origin iframes; the renderer, parameters and " +
      "readPixels hash are identical, so the GPU itself does not change",
  ],
  [
    /^misc\.connection\.(downlink|rtt)$/,
    "Network Information estimates drift between reads, in the same realm as much as across realms",
  ],
  [
    /^misc\.notificationPermission$/,
    "Chromium denies the Notification permission in cross-origin frames by policy",
  ],
  // Both added with probe v4 (Stage 6 T02) and both *measured* on the shipping engine before being
  // excluded, same standard as the three above.
  [
    /^misc\.(sensors\.permissions|geolocation\.permissionState)/,
    "Permissions Policy: a cross-origin frame with no `allow=` for geolocation or a sensor reports " +
      "`denied` where the main frame reports `prompt`. Policy, not fingerprint — the same reason " +
      "`misc.notificationPermission` is excluded",
  ],
  [
    /^misc\.mediaCapabilities\.[a-z0-9]+\.powerEfficient$/,
    "Measured on stock 151: `powerEfficient` is `false` in the main frame and cross-site iframe and " +
      "`true` in the Worker, for all four supported codec families, while `supported` and `smooth` " +
      "agree everywhere. A Worker has no attached rendering surface to judge power efficiency " +
      "against. It is a real divergence a page could read, but it is Chromium's and pre-dates any " +
      "Veil patch — see tools/detection-baseline/README.md, and T11 has to decide whether `969` pins it in " +
      "every realm (more consistent than Chrome, itself a tell) or reproduces the split",
  ],
];

const COMPARABLE = new Set(["string", "number", "boolean"]);

/**
 * The noise leaves `966` (per-origin deterministic farbling) makes a function of the *page's
 * origin*: the same profile must render different values on different sites, while every site is
 * stable across restarts. The main frame and the Worker share the fixture's origin
 * (`127.0.0.1`), so these leaves are still compared — and must agree — between them. The
 * cross-site iframe (`localhost`) is a second origin by design, so when farbling is active its
 * noise leaves are skipped in the equality walk and scored by the explicit decorrelation check
 * below instead: identical canvas bytes across the two fixture origins would mean 966 is not
 * mixing, which is the one failure this assertion exists to catch. With no seed on the command
 * line nothing is perturbed, farbling is inactive, and all three realms are held to equality as
 * before.
 */
const FARBLING_PATHS = [
  /^canvas\.toDataUrlSha256$/,
  /^canvas\.toDataUrlLength$/,
  /^canvas\.getImageDataSha256$/,
  /^canvas\.measureText\./,
  /^webgl\.readPixelsSha256$/,
  /^audio\.render\.sha256$/,
  /^audio\.render\.absSum4500to5000$/,
];

function farbled(path) {
  return FARBLING_PATHS.some((pattern) => pattern.test(path));
}

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

function excluded(path) {
  return CROSS_REALM_EXCLUSIONS.some(([pattern]) => pattern.test(path));
}

function compareRealms(baseName, base, otherName, other, { skipFarbled = false } = {}) {
  const left = flatten(base);
  const right = flatten(other);
  const divergent = [];
  const nullOnlyIn = [];
  const skippedFarbled = [];
  let compared = 0;
  for (const [path, leftValue] of left) {
    if (!right.has(path) || excluded(path)) {
      continue;
    }
    if (skipFarbled && farbled(path)) {
      // Deliberately unscored here: the cross-origin decorrelation check owns these leaves.
      skippedFarbled.push(path);
      continue;
    }
    const rightValue = right.get(path);
    // A `null` here means the realm has no such API (a Worker has no `toDataURL`), not that the
    // value diverged. Recorded separately so it stays visible without failing the assertion.
    if (leftValue === null || rightValue === null) {
      if (leftValue !== rightValue) {
        nullOnlyIn.push({ path, [baseName]: leftValue, [otherName]: rightValue });
      }
      continue;
    }
    compared += 1;
    if (leftValue !== rightValue) {
      divergent.push({ path, [baseName]: leftValue, [otherName]: rightValue });
    }
  }
  return { pair: `${baseName}/${otherName}`, compared, divergent, nullOnlyIn, skippedFarbled };
}

function nativeGetterControl(realms) {
  const perRealm = {};
  const nonNative = [];
  for (const [name, realm] of Object.entries(realms)) {
    const natives = realm && realm.natives;
    if (!natives || natives.unavailable) {
      perRealm[name] = { checked: 0, absent: 0 };
      continue;
    }
    let checked = 0;
    let absent = 0;
    for (const [label, entry] of Object.entries(natives)) {
      if (entry.absent) {
        absent += 1;
        continue;
      }
      checked += 1;
      if (!entry.native) {
        nonNative.push({ realm: name, target: label, source: entry.source });
      }
    }
    perRealm[name] = { checked, absent };
  }
  return { perRealm, nonNative };
}

export function evaluateAssertions({ realms, caseMeta, launch, engine }) {
  const main = realms.main ?? {};
  const control = nativeGetterControl(realms);

  // Farbling activity, from the command line rather than the case file — `VEIL_BASELINE_EXTRA_ARGS`
  // can drive a per-patch capture the case never declared, and the same-origin/cross-origin split
  // below has to follow what the engine actually received. `966` mixes only when a seed is present
  // and the surface has not opted out; `016` reads the `canvas` key, so one opt-out key covers the
  // canvas pair plus readPixels, and the audio render noise is keyed separately.
  const seedSwitchPresent = launch.argv.some((arg) => arg.startsWith("--fingerprint="));
  const optOut = launch.argv.find((arg) => arg.startsWith("--disable-spoofing="));
  const optOutValue = optOut ? optOut.slice("--disable-spoofing=".length) : "";
  const canvasFarblingActive = seedSwitchPresent && !optOutValue.includes("canvas");
  const audioFarblingActive = seedSwitchPresent && !optOutValue.includes("audio");

  const mainVsIframe = realms.iframe
    ? compareRealms("main", main, "iframe", realms.iframe, {
        skipFarbled: canvasFarblingActive || audioFarblingActive,
      })
    : null;
  // The Worker shares the main frame's origin, so its noise leaves stay in the equality walk.
  const mainVsWorker = realms.worker ? compareRealms("main", main, "worker", realms.worker) : null;
  const crossRealm = [mainVsIframe, mainVsWorker].filter(Boolean);
  const divergentTotal = crossRealm.reduce((sum, pair) => sum + pair.divergent.length, 0);

  // The decorrelation half: under an active farbling seed the cross-site iframe's noise leaves
  // must NOT match the main frame's. Both realms draw from the same two servers every run, so
  // identical hashes are a mixing failure, not a fixture artefact. Checked on the leaves that
  // exist in both realms (the Worker has no OfflineAudioContext; toDataURL is main-only).
  const farblingChecks = [];
  if (canvasFarblingActive && realms.iframe) {
    farblingChecks.push(
      {
        leaf: "canvas.getImageDataSha256",
        mustDiffer: main.canvas?.getImageDataSha256 !== realms.iframe.canvas?.getImageDataSha256,
      },
      {
        leaf: "webgl.readPixelsSha256",
        mustDiffer: main.webgl?.readPixelsSha256 !== realms.iframe.webgl?.readPixelsSha256,
      },
    );
  }
  if (audioFarblingActive && realms.iframe) {
    farblingChecks.push({
      leaf: "audio.render.sha256",
      mustDiffer: main.audio?.render?.sha256 !== realms.iframe.audio?.render?.sha256,
    });
  }
  const decorrelationHolds = farblingChecks.every((check) => check.mustDiffer);

  const seedRequested = caseMeta.fingerprint.seed ?? 0;
  const seedSwitch = launch.argv.find((arg) => arg.startsWith("--fingerprint="));
  const seedSwitchEmitted = seedSwitch !== undefined;
  const seedSwitchValue = seedSwitch ? seedSwitch.slice("--fingerprint=".length) : null;
  /**
   * Any switch only a patched engine implements. `--fingerprint=` is the master switch for eleven of
   * the sixteen Tier 1 patches, but `--timezone=` and `--fingerprint-*` drive the others on their
   * own, so keying off the seed alone would call a timezone-only run unpatched.
   */
  const fingerprintSwitchPresent = launch.argv.some(
    (arg) =>
      arg.startsWith("--fingerprint") ||
      arg.startsWith("--timezone=") ||
      arg.startsWith("--disable-spoofing="),
  );

  const webrtc = main.webrtc ?? {};
  const leaks = webrtc.revealsPrivateAddress === true || webrtc.revealsGlobalAddress === true;

  const requestedZone = caseMeta.fingerprint.timezone ?? "";
  const reportedZone = main.intl?.dateTimeFormat?.timeZone ?? null;

  return {
    "1-patched-getters-are-native": {
      // Vacuous only while no patched code path is active. Once a fingerprint switch reaches the
      // engine at least one of these getters is running patched code, and "still reads
      // `[native code]`" becomes the real assertion — the property a JS-level spoof cannot have,
      // and the reason this migration exists. Leaving the label at `vacuous-pass` after that point
      // would understate a result the snapshot has actually earned.
      status:
        control.nonNative.length > 0 ? "fail" : fingerprintSwitchPresent ? "pass" : "vacuous-pass",
      reason:
        control.nonNative.length > 0
          ? "At least one getter no longer reads `[native code]`, which is exactly how a spoof " +
            "self-reveals through `Function.prototype.toString`."
          : fingerprintSwitchPresent
            ? "A real test: a fingerprint switch reached the engine, so patched code is running, and " +
              "every listed getter still reads `[native code]`."
            : "No fingerprint switch reached the engine, so nothing is patched and this is a control " +
              "rather than a test: every listed getter must still read `[native code]` today.",
      ...control,
    },
    "2-values-identical-across-realms": {
      // 966 scopes this per origin (its own commit, per the 6.13 precedent for
      // assertion-affecting changes): same-origin realms must agree on every leaf including the
      // noise surfaces, the cross-site iframe's noise leaves are owned by the farbling check
      // below, and everything else still must be identical in all three realms. A farbled seed
      // that fails to decorrelate the two fixture origins fails here, exactly as a same-origin
      // divergence does.
      status: divergentTotal === 0 && decorrelationHolds ? "pass" : "fail",
      reason:
        "Same-origin realms (main frame and Worker) must agree on every leaf; the cross-site " +
        "iframe is its own origin, and under `966`'s per-origin farbling its noise leaves must " +
        "differ from the main frame's — identical hashes across the two fixture origins would " +
        "mean the seed is not mixing, and divergence elsewhere is still a genuine finding.",
      pairs: crossRealm,
      farbling: {
        canvasActive: canvasFarblingActive,
        audioActive: audioFarblingActive,
        checks: farblingChecks,
      },
      exclusions: CROSS_REALM_EXCLUSIONS.map(([pattern, why]) => ({
        path: pattern.source,
        why,
      })),
    },
    "3-seed-determines-the-fingerprint": {
      // Two distinguishable situations, and neither is a pass. Hard-coding `n/a` with "the switch
      // is not emitted" was true only while nothing could put `--fingerprint=` on the command line;
      // once Stage 5 starts injecting it via `VEIL_BASELINE_EXTRA_ARGS`, that reason would be a
      // false statement sitting inside the artifact.
      status: seedSwitchEmitted ? "not-run" : "n/a",
      reason: seedSwitchEmitted
        ? "`--fingerprint=` reached the engine, but this assertion is not scorable from one " +
          "snapshot: it needs a second capture at the same seed (values must match) and a third at " +
          "a different seed (values must differ). Compare `seedSwitchValue` across those captures."
        : "No `--fingerprint=` switch on the command line, so there is nothing to reproduce. " +
          "`to_engine_args` correctly emits none under `stock-chromium`; a patched engine is driven " +
          "either by flipping the dialect or, during Stage 5 verification, by " +
          "`VEIL_BASELINE_EXTRA_ARGS`.",
      seedRequested,
      seedSwitchEmitted,
      seedSwitchValue,
    },
    "4-no-webrtc-candidate-reveals-the-real-ip": {
      status: !caseMeta.proxyConfigured ? "not-run" : leaks ? "fail" : "pass",
      reason: caseMeta.proxyConfigured
        ? "Measured, not assumed — and the thing measured is not the flag. The switch the `chrome` " +
          "binary reads is `--webrtc-ip-handling-policy`; the `--force-` spelling Veil emitted until " +
          "Stage 6's T03 is a `content/` switch that only content_shell and headless read, so it had " +
          "no effect at all. What holds this shut on the shipping engine is ungoogled-chromium's " +
          "default pref (`disable_non_proxied_udp`), so a `pass` here means *no UDP candidate was " +
          "gathered at all*, not that a gathered one was sanitised. Relaxing the policy to `default` " +
          "scores `fail` on this host, which is what makes the `pass` a test rather than an artefact " +
          "of the environment."
        : "No proxy was configured for this run, so there is no real IP to hide. Re-run with " +
          "VEIL_BASELINE_PROXY set to score this assertion.",
      candidateCount: webrtc.candidateCount ?? null,
      sdpCandidateLineCount: webrtc.sdpCandidateLineCount ?? null,
      addressClasses: webrtc.classes ?? null,
      revealsPrivateAddress: webrtc.revealsPrivateAddress ?? null,
      revealsGlobalAddress: webrtc.revealsGlobalAddress ?? null,
    },
    "5-intl-timezone-matches-the-proxy-exit": {
      status:
        requestedZone === "" ? "not-run" : reportedZone === requestedZone ? "pass" : "known-floor",
      reason:
        requestedZone === ""
          ? "This case requests no timezone, so there is nothing to compare."
          : "Expected floor, not a finding: Stage 1.6 measured that Windows Chromium ignores `TZ`, " +
            "which is the only channel `stock-chromium` has. Closed by Tier 1 patch `018-timezone`.",
      requestedZone: requestedZone === "" ? null : requestedZone,
      reportedZone,
      timezoneChannel: launch.env.TZ ? "TZ env var" : "none",
    },
    // Sixth because it was added sixth, not because it ranks last: a detector scores contradictions,
    // so a persona that fails this one is worse off than a persona that fails 3. It is scored from the
    // same capture as the rest and shares assertion 1's definition of "patched code is running".
    "6-the-persona-describes-a-machine-that-could-exist": evaluateCoherence({
      realms,
      caseMeta,
      engine,
      launch,
      fingerprintSwitchPresent,
    }),
  };
}
