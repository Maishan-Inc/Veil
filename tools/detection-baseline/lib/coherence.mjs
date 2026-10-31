/**
 * Assertion 6 — a persona must describe a machine that could exist.
 *
 * Assertions 1–5 ask whether the engine spoofs what it claims to spoof. None of them asks whether the
 * result is a machine anyone could buy, and that gap produced exactly the artifact it exists to
 * prevent. All four of these were read out of
 * `snapshots/151.0.7922.137-1.1001/veil-patched-persona.json`, a snapshot the per-release checklist
 * recorded as clean:
 *
 * - an `Apple M3` Metal renderer beside `devicePixelRatio: 1` — no Apple Silicon Mac has ever shipped
 *   a non-Retina display;
 * - a font list carrying `Menlo`, `Monaco`, `Geneva` and `PingFang SC` **and** `Segoe UI`,
 *   `Microsoft Sans Serif` and `Microsoft PhagsPa` — two operating systems' system fonts at once;
 * - three `Microsoft … (zh-CN)` speech voices on a machine claiming `en-US` macOS;
 * - `screen.width` 2560 on a persona whose claimed display is a 13-inch laptop panel.
 *
 * Each rule below takes **no external spec sheet.** It is either arithmetic, or a statement about
 * which OS ships a thing — never "an M3 MacBook Air reports N cores", which is a number this repo
 * cannot measure and which would rot at the next hardware revision.
 *
 * Every finding records whether the contradicting surface is one the patch set **drives**:
 *
 * - driven → `fail`. Something Veil steers produced an impossible value. That is a defect.
 * - not driven → `known-floor`. The surface is the host's because no patch reads a switch for it, so
 *   the fix is a Stage 6 patch rather than a regression. Reporting `fail` on every run would make the
 *   status carry no information — the failure mode assertion 5 already avoids in exactly this way.
 *
 * `tools/detection-baseline/README.md` → *Assertion 6* carries the rule table and the current verdicts.
 */

/**
 * Families shipped by exactly one OS, kept deliberately small. Anything with a plausible second
 * distribution channel is **out**: Office for Mac installs Calibri, Cambria, Candara, Constantia,
 * Corbel, Webdings and Wingdings, so none of those can carry a platform claim even though all seven
 * ship with Windows. `Helvetica` is out for the same reason from the other side — it is present on
 * this Windows host, measured in the `default` snapshot. Every name here is also in
 * `pages/probe.mjs` → `FONT_PROBES`; a family the probe never asks about cannot be evidence.
 */
const OS_ONLY_FONTS = {
  windows: [
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Symbol",
    "Segoe UI Historic",
    "Segoe MDL2 Assets",
    "Segoe Print",
    "Segoe Script",
    "Marlett",
    "Microsoft Sans Serif",
    "Microsoft Himalaya",
    "Microsoft JhengHei",
    "Microsoft New Tai Lue",
    "Microsoft PhagsPa",
    "Microsoft Tai Le",
    "Microsoft Yi Baiti",
    "MingLiU-ExtB",
    "Ebrima",
    "Gadugi",
    "Leelawadee UI",
    "Nirmala UI",
    "Myanmar Text",
    "Mongolian Baiti",
    "Javanese Text",
    "MV Boli",
    "Ink Free",
    "Bahnschrift",
    "HoloLens MDL2 Assets",
    "Sitka",
    "Sylfaen",
    "Gabriola",
  ],
  macos: [
    "Apple Color Emoji",
    "Avenir",
    "Baskerville",
    "Didot",
    "Geneva",
    "Helvetica Neue",
    "Hiragino Sans",
    "Lucida Grande",
    "Menlo",
    "Monaco",
    "Optima",
    "PingFang SC",
  ],
  linux: ["Cantarell", "DejaVu Sans", "DejaVu Serif", "FreeSans", "Liberation Sans", "Ubuntu"],
};

/**
 * Which surfaces the installed engine actually steers, per dialect, and what steers them. This table
 * decides `fail` vs `known-floor`, so it is a third mirror of `veil_core::to_engine_args` alongside
 * `@veil/shared`'s `fingerprintFieldState()` — keep all three in step. A surface absent from a
 * dialect's entry is the host's, and a contradiction on it is a floor rather than a defect.
 */
const DRIVEN_SURFACES = {
  "stock-chromium": {
    window: "--window-size",
  },
  "veil-patched": {
    identity: "002 + 902 derive the UA, UA-CH and navigator.platform from --fingerprint-platform",
    window: "--window-size",
    gpuName: "011 picks the unmasked vendor/renderer pair from a table by seed and platform",
    gpuLimits:
      "963 answers the getParameter limits, their WebGL2 block and the extension list from the ANGLE backend --fingerprint-platform implies, clamped to the live driver",
    fonts: "006 rewrites the enumerable family list by seed and platform",
    cores: "005 reads --fingerprint-hardware-concurrency",
    screen:
      "960 reads --fingerprint-screen-{width,height} and derives avail*, colorDepth, orientation and isExtended from the panel and --fingerprint-platform",
    dpr: "960 reads --fingerprint-device-scale-factor",
    voices:
      "968 presents the claimed platform's speech catalogue, keyed on --fingerprint-platform, with the default voice chosen by the locale switches",
    audio:
      "967 answers AudioContext.baseLatency from the output buffer size the claimed platform's own Chromium backend would choose",
    codecs:
      "969 answers HEVC support from --fingerprint-platform in media/base/supported_types.cc, and the hardware-decode set behind decodingInfo's powerEfficient from the Apple part 011 picked",
    mediaDevices: "969 adds one described entry for a capture kind the host does not have",
    touch:
      "961 reads --fingerprint-max-touch-points and, for a cross-platform claim, answers 0 for macOS and Linux — deriving the pointer and hover media features from the same count in SlowWebPreferenceCache::Load",
    memory:
      "905 reads --fingerprint-device-memory, restricted to the five values ApproximatedDeviceMemory can produce on desktop",
    geolocation:
      "970 answers navigator.geolocation from --fingerprint-location=lat,lng — coordinates the profile declares, delivered only on the paths the permission flow already granted",
    battery:
      "990 presents the claimed platform's battery shape — a phone on battery, a desktop on AC — keyed on --fingerprint-platform",
    plugins:
      "990 lets the plugin block follow the claimed platform: the desktop PDF set, or none at all behind a phone claim",
  },
};

/**
 * What each platform claim commits the identity surfaces to. Certain by construction: these are the
 * strings Chromium itself emits per OS, not a hardware spec.
 */
const PLATFORM_IDENTITY = {
  windows: { platform: "Win32", uaContains: "Windows NT", uaChPlatform: "Windows" },
  macos: { platform: "MacIntel", uaContains: "Macintosh", uaChPlatform: "macOS" },
  linux: { platform: "Linux x86_64", uaContains: "Linux", uaChPlatform: "Linux" },
  android: { platform: "Linux armv81", uaContains: "Android 10; K", uaChPlatform: "Android" },
};

/**
 * The switch that puts each surface under a patch's control. Consulted alongside the dialect table,
 * because `VEIL_BASELINE_EXTRA_ARGS` deliberately injects switches the dialect cannot emit — a Stage 5
 * per-patch capture is exactly that shape — and a surface that was driven in *this run* must not be
 * excused as a floor merely because the manifest could not have driven it.
 *
 * Seed-derived surfaces are absent on purpose: canvas and audio noise, `readPixels` and the font
 * *contents* have no switch of their own, so the dialect table is the only thing that can speak for
 * them.
 */
const SURFACE_SWITCH = {
  identity: "--fingerprint-platform=",
  gpuName: "--fingerprint-platform=",
  gpuLimits: "--fingerprint-platform=",
  fonts: "--fingerprint-platform=",
  voices: "--fingerprint-platform=",
  audio: "--fingerprint-platform=",
  codecs: "--fingerprint-platform=",
  mediaDevices: "--fingerprint-platform=",
  cores: "--fingerprint-hardware-concurrency=",
  window: "--window-size=",
  screen: "--fingerprint-screen-width=",
  dpr: "--fingerprint-device-scale-factor=",
  touch: "--fingerprint-max-touch-points=",
  memory: "--fingerprint-device-memory=",
  geolocation: "--fingerprint-location=",
  battery: "--fingerprint-platform=",
  plugins: "--fingerprint-platform=",
};

/** A finding is one contradiction, with everything needed to act on it. */
function finding(rule, surface, claim, observed, why) {
  return { rule, surface, claim, observed, why };
}

function drivenBy(dialect, surface, argv) {
  // 006 rewrites fonts only for a seeded persona. A seedless `veil-patched` default capture is the
  // host's font set, just like stock Chrome; classifying it as driven turns a host font setting into
  // an engine defect even when both browsers report the same measurements.
  if (surface === "fonts" && !argv.some((arg) => arg.startsWith("--fingerprint="))) {
    return null;
  }
  // Android fonts are a known floor (6.26): 006 hides the host's enumerable families (so
  // `fonts.present` is empty on a phone), but the CSS-generic arm is `972`'s and it carries only
  // macOS and Linux rows — `990` added no android one. `monospace` falling through to the host's
  // standard face is therefore the host's behaviour, not a surface a patch claims to steer; the
  // residual is recorded here rather than scored as a defect.
  const isAndroid = argv.some((arg) => arg === "--fingerprint-platform=android");
  if (surface === "fonts" && isAndroid) {
    return null;
  }
  const byDialect = DRIVEN_SURFACES[dialect]?.[surface];
  if (byDialect) {
    return byDialect;
  }
  const switchName = SURFACE_SWITCH[surface];
  if (switchName && argv.some((arg) => arg.startsWith(switchName))) {
    return `${switchName} is on this command line, so a patch reads it even though \`${dialect}\` does not emit it`;
  }
  return null;
}

/**
 * R1 — the display has to be arithmetically possible. `avail*` is `screen.*` minus the OS chrome, so
 * it can neither exceed it nor be short of it by the height of a second monitor. R2 — a window cannot
 * be larger than the work area it sits in. Neither needs a spec sheet.
 *
 * R2 was the rule that fired the moment `--window-size` was set against a `screen.*` the engine did
 * not own. Since T06 it should be unreachable from the Veil side: `veil_core::validate` rejects a
 * profile whose window does not fit the work area its declared panel leaves, so a finding here now
 * means either an injected switch the validator never saw or a drift between the inset table in
 * `veil_core` and the one in `960`.
 */
function screenShapeRules(screen, claimedOs, findings) {
  if (!screen || screen.unavailable) {
    return;
  }
  const { width, height, availWidth, availHeight, outerWidth, outerHeight } = screen;
  if (availWidth > width || availHeight > height) {
    findings.push(
      finding(
        "screen-work-area-fits-the-panel",
        "screen",
        `availWidth ≤ width and availHeight ≤ height`,
        `${availWidth}×${availHeight} work area on a ${width}×${height} panel`,
        "The work area is the panel minus the OS chrome; it cannot be larger than the panel.",
      ),
    );
  }
  const chrome = height - availHeight;
  if (chrome < 0 || chrome > 200) {
    findings.push(
      finding(
        "screen-os-chrome-is-plausible",
        "screen",
        "0 ≤ height − availHeight ≤ 200",
        `${chrome} px of vertical chrome`,
        "A taskbar, menu bar or dock occupies a bounded strip. Zero is possible only in kiosk " +
          "configurations, and 200 px exceeds every stock desktop shell.",
      ),
    );
  }
  // On android the window *width* is clamped by the desktop host's minimum window width — Chrome
  // will not open a 360–412px phone window on Windows, so the width arm is skipped there. The
  // height arm still holds: a phone window is never taller than its own panel.
  const widthExceeds = claimedOs !== "android" && outerWidth > availWidth;
  if (widthExceeds || outerHeight > availHeight) {
    findings.push(
      finding(
        "window-fits-the-work-area",
        "screen",
        claimedOs === "android"
          ? "outerHeight ≤ availHeight (the width arm is skipped: the desktop host clamps a phone window to its minimum width)"
          : "outerWidth ≤ availWidth and outerHeight ≤ availHeight",
        `${outerWidth}×${outerHeight} window in a ${availWidth}×${availHeight} work area`,
        "A window larger than the work area is what `--window-size` produces when `screen.*` is " +
          "still the host's.",
      ),
    );
  }
}

/**
 * R3 — the persona declares a panel, and the engine either delivers it or it does not.
 *
 * `machine.display` is a **declaration of intent**, not a measured spec of any shipping laptop: its
 * only job is to make "the engine reported something else" detectable at all. It is stated
 * independently of `fingerprint.panel_*` on purpose — the case file carries both and
 * `committed_cases_are_loadable_and_coherent` pins them equal — so this rule compares two statements
 * rather than one statement with itself.
 *
 * A `known-floor` until Stage 6's T05. `000` had declared `--fingerprint-screen-{width,height}` and
 * `--fingerprint-device-scale-factor` for five stages with no patch reading one; `960` reads all
 * three, so a mismatch here is now a defect on a surface Veil steers.
 */
function declaredDisplayRules(display, screen, findings) {
  if (!display || !screen || screen.unavailable) {
    return;
  }
  for (const key of ["width", "height", "devicePixelRatio"]) {
    if (display[key] === undefined || display[key] === screen[key]) {
      continue;
    }
    findings.push(
      finding(
        "display-matches-the-declared-panel",
        key === "devicePixelRatio" ? "dpr" : "screen",
        `screen.${key} = ${display[key]}`,
        `${screen[key]}`,
        "The persona declares this panel; the engine reported the host's.",
      ),
    );
  }
}

/**
 * R3b — the rest of the display block, all of it derived from the panel rather than declared.
 *
 * Each of these is a statement `960` makes for free once it owns `screen.*`, and each is a
 * contradiction the patch itself could introduce. Kept separate from R3 because none of them needs a
 * declared panel: they hold for any single-display machine.
 *
 * - a landscape panel reports a landscape orientation, and a portrait panel a portrait one. Blink's
 *   own rule at rotation 0, so this is arithmetic rather than a spec.
 * - one display is not an extended display, and `isExtended: true` on a spoofed panel is the host's
 *   second monitor showing through.
 * - `pixelDepth` is Blink's alias of `colorDepth`, so they cannot differ; and a desktop panel reports
 *   24 or 30 bits, never 16 or 0.
 */
function derivedDisplayRules(screen, findings) {
  if (!screen || screen.unavailable) {
    return;
  }
  const { width, height, colorDepth, pixelDepth, isExtended, orientation } = screen;
  const expectedOrientation = height > width ? "portrait-primary" : "landscape-primary";
  if (orientation && orientation.type && orientation.type !== expectedOrientation) {
    findings.push(
      finding(
        "screen-orientation-follows-the-panel",
        "screen",
        `orientation.type = ${expectedOrientation} for a ${width}×${height} panel`,
        `${orientation.type}`,
        "A display wider than it is tall is in a landscape orientation. This is Blink's own rule " +
          "for an unrotated screen, so a disagreement means the orientation and the panel came " +
          "from different places.",
      ),
    );
  }
  if (isExtended === true) {
    findings.push(
      finding(
        "one-display-is-not-extended",
        "screen",
        "isExtended = false",
        "true",
        "A persona describes one display. `true` is the host's second monitor reported through a " +
          "spoofed panel, and it is a fact about the real machine.",
      ),
    );
  }
  if (pixelDepth !== undefined && colorDepth !== undefined && pixelDepth !== colorDepth) {
    findings.push(
      finding(
        "pixel-depth-is-the-colour-depth",
        "screen",
        `pixelDepth = colorDepth (${colorDepth})`,
        `${pixelDepth}`,
        "Blink implements `pixelDepth` as an alias of `colorDepth`; two different values cannot " +
          "come out of one getter unless something set them apart.",
      ),
    );
  }
  if (colorDepth !== undefined && colorDepth !== 24 && colorDepth !== 30) {
    findings.push(
      finding(
        "colour-depth-is-a-desktop-value",
        "screen",
        "colorDepth is 24 or 30",
        `${colorDepth}`,
        "Every desktop shell a persona would claim reports 24, and a 30-bit panel is the only " +
          "other value in circulation. 16 and 0 belong to hardware no longer sold.",
      ),
    );
  }
}

/**
 * R3c — the media-query side of the engine has to report the display the `screen` object just did.
 *
 * Probe v6's `misc.mediaQueryAgreement` asks `(device-width:)`, `(device-height:)` and
 * `(resolution:)` about the values `screen.width`, `screen.height` and `devicePixelRatio` returned.
 * All three are `true` on any real browser and on the zero-patch floor, because both sides read one
 * display. A display patch that moves `Screen` and forgets `MediaValues::CalculateDeviceWidth`
 * leaves every other surface in the snapshot untouched and flips one of these to `false` — the only
 * place the half-finished shape is visible, and the reason the probe row exists.
 */
function mediaQueryAgreementRules(misc, findings) {
  const agreement = misc?.mediaQueryAgreement;
  if (!agreement || agreement.unavailable) {
    return;
  }
  const asked = {
    deviceWidthMatchesScreen: ["(device-width:)", "screen.width"],
    deviceHeightMatchesScreen: ["(device-height:)", "screen.height"],
    resolutionMatchesDevicePixelRatio: ["(resolution:)", "window.devicePixelRatio"],
  };
  for (const [key, [query, getter]] of Object.entries(asked)) {
    if (agreement[key] === false) {
      findings.push(
        finding(
          "media-queries-agree-with-the-screen",
          key === "resolutionMatchesDevicePixelRatio" ? "dpr" : "screen",
          `${query} matches the value ${getter} reports`,
          "it does not",
          "CSS and the `screen` object are two code paths for one display. A page needs one line " +
            "of CSS to compare them, so a patch that moves only one of them is worse than a patch " +
            "that moves neither.",
        ),
      );
    }
  }
}

/**
 * R4 — Apple Silicon implies Retina, and this one needs no declaration at all.
 *
 * Every Mac Apple has shipped with an `arm` architecture has a display whose `devicePixelRatio` is 2,
 * built in or driven. `devicePixelRatio: 1` beside UA-CH `architecture: arm` is therefore impossible
 * regardless of which Mac is being claimed — which is what made the `-1.1001` persona snapshot
 * describe a machine that does not exist.
 */
function appleSiliconRetinaRule(claimedOs, navigator, screen, findings) {
  const architecture = navigator?.userAgentData?.highEntropy?.architecture ?? null;
  if (claimedOs !== "macos" || architecture !== "arm" || !screen || screen.unavailable) {
    return;
  }
  if (!(screen.devicePixelRatio >= 2)) {
    findings.push(
      finding(
        "apple-silicon-implies-retina",
        "dpr",
        "devicePixelRatio ≥ 2 for UA-CH architecture `arm`",
        `devicePixelRatio ${screen.devicePixelRatio}`,
        "No Apple Silicon Mac has shipped a non-Retina display, so this pair cannot occur on real " +
          "hardware. Needs no spec sheet and no declared panel.",
      ),
    );
  }
}

/**
 * R5 — the three identity surfaces must all name the claimed OS. Driven by `002` + `902`, so a
 * mismatch here is a defect rather than a floor.
 */
function identityRules(claimedOs, navigator, findings) {
  const expected = PLATFORM_IDENTITY[claimedOs];
  if (!expected || !navigator) {
    return;
  }
  if (navigator.platform !== null && navigator.platform !== expected.platform) {
    findings.push(
      finding(
        "navigator-platform-names-the-claimed-os",
        "identity",
        `navigator.platform = ${expected.platform}`,
        `${navigator.platform}`,
        "The platform string is derived from `--fingerprint-platform` by `002`.",
      ),
    );
  }
  if (navigator.userAgent && !navigator.userAgent.includes(expected.uaContains)) {
    findings.push(
      finding(
        "user-agent-names-the-claimed-os",
        "identity",
        `userAgent contains ${expected.uaContains}`,
        `${navigator.userAgent}`,
        "The UA is derived from the same switch as `navigator.platform`.",
      ),
    );
  }
  const uaChPlatform = navigator.userAgentData?.platform ?? null;
  if (uaChPlatform !== null && uaChPlatform !== expected.uaChPlatform) {
    findings.push(
      finding(
        "ua-ch-platform-names-the-claimed-os",
        "identity",
        `userAgentData.platform = ${expected.uaChPlatform}`,
        `${uaChPlatform}`,
        "UA-CH and the UA string come from one source since `902`; disagreement means one of them " +
          "is being set from somewhere else.",
      ),
    );
  }
}

/**
 * R6 — the graphics backend named in the renderer string only exists on one OS. `Direct3D` / `D3D11`
 * is Windows-only and `Metal` is Apple-only, which is enough to catch a platform swap without any
 * table of GPU models. Driven by `011`.
 */
const GPU_BACKEND_OS = [
  [/direct3d|d3d11/i, "windows", "Direct3D is a Windows-only backend"],
  [/metal/i, "macos", "Metal is an Apple-only backend"],
];

/**
 * R6b — the limits beside the renderer name have to be arithmetically possible.
 *
 * Nine of the numbers a WebGL2 context reports are *computed* by ANGLE from the others, by the same
 * arithmetic on every backend: a "components" limit is four times its "vectors" limit, the combined
 * uniform-block count is the sum of the two stage counts, and a combined component count is
 * `blocks × (blockSize / 4) + components`. Verified in this tree, in both backends Veil claims —
 * `third_party/angle/.../d3d11/renderer11_utils.cpp:1198,1228,1243,1248,1254` and
 * `.../metal/DisplayMtl.mm:733,842,887`.
 *
 * So this rule takes no spec sheet, which is what lets it live here: it never asks what an M3 reports,
 * only whether the set in front of it can come from one implementation. That is precisely the check a
 * partial limits spoof fails — move `MAX_VERTEX_UNIFORM_VECTORS` and leave
 * `MAX_VERTEX_UNIFORM_COMPONENTS` behind and the two disagree by construction, and a page finds it
 * with one division. `963` derives every dependent after clamping the independents for this reason;
 * its counterpart `070` spoofs the stage uniform-block counts while leaving both combined component
 * counts on the driver, which is the same equation left half-updated (6.12).
 *
 * One relation the snapshot cannot carry: `MAX_COMBINED_TEXTURE_IMAGE_UNITS` is the sum of the vertex
 * and fragment texture-unit limits, and the probe records the fragment one only. Adding
 * `MAX_VERTEX_TEXTURE_IMAGE_UNITS` is a probe bump, deliberately not taken in the same task as the
 * measurement it would perturb.
 */
function gpuLimitRules(webgl, findings) {
  const one = webgl?.parameters ?? null;
  const two = webgl?.webgl2Parameters ?? null;
  if (!one || !two || two.unavailable) {
    return;
  }
  const relations = [
    [
      "MAX_VARYING_COMPONENTS",
      two.MAX_VARYING_COMPONENTS,
      4 * one.MAX_VARYING_VECTORS,
      "4 × MAX_VARYING_VECTORS",
    ],
    [
      "MAX_VERTEX_OUTPUT_COMPONENTS",
      two.MAX_VERTEX_OUTPUT_COMPONENTS,
      4 * one.MAX_VARYING_VECTORS,
      "4 × MAX_VARYING_VECTORS",
    ],
    [
      "MAX_FRAGMENT_INPUT_COMPONENTS",
      two.MAX_FRAGMENT_INPUT_COMPONENTS,
      4 * one.MAX_VARYING_VECTORS,
      "4 × MAX_VARYING_VECTORS",
    ],
    [
      "MAX_VERTEX_UNIFORM_COMPONENTS",
      two.MAX_VERTEX_UNIFORM_COMPONENTS,
      4 * one.MAX_VERTEX_UNIFORM_VECTORS,
      "4 × MAX_VERTEX_UNIFORM_VECTORS",
    ],
    [
      "MAX_FRAGMENT_UNIFORM_COMPONENTS",
      two.MAX_FRAGMENT_UNIFORM_COMPONENTS,
      4 * one.MAX_FRAGMENT_UNIFORM_VECTORS,
      "4 × MAX_FRAGMENT_UNIFORM_VECTORS",
    ],
    [
      "MAX_COMBINED_UNIFORM_BLOCKS",
      two.MAX_COMBINED_UNIFORM_BLOCKS,
      two.MAX_VERTEX_UNIFORM_BLOCKS + two.MAX_FRAGMENT_UNIFORM_BLOCKS,
      "MAX_VERTEX_UNIFORM_BLOCKS + MAX_FRAGMENT_UNIFORM_BLOCKS",
    ],
    [
      "MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS",
      two.MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS,
      two.MAX_VERTEX_UNIFORM_BLOCKS * (two.MAX_UNIFORM_BLOCK_SIZE / 4) +
        two.MAX_VERTEX_UNIFORM_COMPONENTS,
      "MAX_VERTEX_UNIFORM_BLOCKS × (MAX_UNIFORM_BLOCK_SIZE / 4) + MAX_VERTEX_UNIFORM_COMPONENTS",
    ],
    [
      "MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS",
      two.MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS,
      two.MAX_FRAGMENT_UNIFORM_BLOCKS * (two.MAX_UNIFORM_BLOCK_SIZE / 4) +
        two.MAX_FRAGMENT_UNIFORM_COMPONENTS,
      "MAX_FRAGMENT_UNIFORM_BLOCKS × (MAX_UNIFORM_BLOCK_SIZE / 4) + MAX_FRAGMENT_UNIFORM_COMPONENTS",
    ],
  ];
  for (const [name, observed, expected, formula] of relations) {
    if (!Number.isFinite(observed) || !Number.isFinite(expected)) {
      continue;
    }
    if (observed !== expected) {
      findings.push(
        finding(
          "gpu-limits-are-arithmetically-consistent",
          "gpuLimits",
          `${name} = ${formula} = ${expected}`,
          `${observed}`,
          "ANGLE computes this limit from the others on every backend, so a set that fails the " +
            "arithmetic cannot come from one implementation — a partial spoof, and divisible out in " +
            "one line.",
        ),
      );
    }
  }
  const bindings = two.MAX_UNIFORM_BUFFER_BINDINGS;
  const blocks = two.MAX_COMBINED_UNIFORM_BLOCKS;
  if (Number.isFinite(bindings) && Number.isFinite(blocks) && bindings < blocks) {
    findings.push(
      finding(
        "gpu-limits-are-arithmetically-consistent",
        "gpuLimits",
        `MAX_UNIFORM_BUFFER_BINDINGS ≥ MAX_COMBINED_UNIFORM_BLOCKS (${blocks})`,
        `${bindings}`,
        "Every uniform block a program can declare needs a binding point to be bound to, so the " +
          "ES3 spec requires this ordering and both backends report the two as equal.",
      ),
    );
  }
}

function gpuRules(claimedOs, webgl, findings) {
  const renderer = webgl?.unmaskedRenderer ?? null;
  if (!renderer || !PLATFORM_IDENTITY[claimedOs]) {
    return;
  }
  for (const [pattern, onlyOn, why] of GPU_BACKEND_OS) {
    if (pattern.test(renderer) && claimedOs !== onlyOn) {
      findings.push(
        finding(
          "gpu-backend-exists-on-the-claimed-os",
          "gpuName",
          `a renderer string reachable on ${claimedOs}`,
          renderer,
          `${why}, so this renderer cannot appear on the claimed platform.`,
        ),
      );
    }
  }
}

/**
 * R7 — a machine runs one operating system, so its enumerable font set belongs to one operating
 * system. Driven by `006` + `906`, and measured firing on the `-1.1001` persona: eight macOS-only
 * families beside three Windows-only ones.
 *
 * Two findings, deliberately separate, and only the first survives without a platform claim. A mixed
 * set is impossible on *any* machine, so it is scored even when nothing was claimed; "carries another
 * OS's families" needs something to compare against and is skipped when the case claims no platform.
 *
 * Both look at names only. The advance widths behind those names are a second surface that a single
 * snapshot cannot score — `font-metrics-belong-to-the-claimed-os` in `notScorableHere`.
 */
function fontRules(claimedOs, fonts, findings) {
  const present = new Set(Array.isArray(fonts?.present) ? fonts.present : []);
  if (present.size === 0) {
    return;
  }
  const byOs = Object.fromEntries(
    Object.entries(OS_ONLY_FONTS).map(([os, families]) => [
      os,
      families.filter((family) => present.has(family)),
    ]),
  );
  const withFamilies = Object.entries(byOs).filter(([, families]) => families.length > 0);
  if (withFamilies.length > 1) {
    findings.push(
      finding(
        "font-set-belongs-to-one-os",
        "fonts",
        "single-OS families from at most one OS",
        withFamilies.map(([os, families]) => `${os}: ${families.join(", ")}`).join(" | "),
        "A machine runs one OS, so it cannot enumerate two OSes' system fonts at once. This is the " +
          "loudest single contradiction a font list can carry.",
      ),
    );
  }
  if (!OS_ONLY_FONTS[claimedOs]) {
    return;
  }
  for (const [os, families] of withFamilies) {
    if (os !== claimedOs) {
      findings.push(
        finding(
          "font-set-follows-the-claimed-os",
          "fonts",
          `no ${os}-only families on a ${claimedOs} machine`,
          families.join(", "),
          "`006` rewrites the list per `--fingerprint-platform`, so a leftover family from another " +
            "OS is a gap in that rewrite rather than a host property.",
        ),
      );
    }
  }
}

/**
 * R8 / R9 — the speech-synthesis catalogue is installed by the OS and by the OS's language packs, so
 * it gives away both the platform and the locale. `Microsoft <name>` voices exist only on Windows;
 * that direction is certain, while "these are macOS voices" is not, because macOS voice names carry
 * no vendor prefix. R9 is the weaker but still certain half: a machine configured for a language ships
 * at least one voice for it. Neither surface has a patch, so both are floors.
 */
const VOICE_VENDOR_OS = [
  [
    /^Microsoft /,
    "windows",
    "the `Microsoft <name>` voices ship with Windows and its language packs",
  ],
];

function voiceTags(voices) {
  if (Array.isArray(voices.langs) && voices.langs.length > 0) {
    return voices.langs;
  }
  // Windows names its voices `Microsoft Huihui - Chinese (Simplified, PRC) (zh-CN)`, so the tag is
  // recoverable from a probe that predates the `langs` field rather than making this rule unscorable
  // on every snapshot already committed.
  return (voices.names ?? []).flatMap((name) => {
    const match = /\(([a-z]{2}(?:-[A-Za-z0-9]{2,8})*)\)\s*$/.exec(name);
    return match ? [match[1]] : [];
  });
}

function voiceRules(claimedOs, locale, misc, findings) {
  const voices = misc?.speechVoices;
  if (!voices || voices.unavailable || (voices.names ?? []).length === 0) {
    return;
  }
  for (const [pattern, onlyOn, why] of VOICE_VENDOR_OS) {
    const matched = voices.names.filter((name) => pattern.test(name));
    // A case that claims no platform cannot be contradicted by a vendor prefix. The locale half
    // below still runs, because it compares against `locale` rather than against the platform.
    if (matched.length > 0 && PLATFORM_IDENTITY[claimedOs] && claimedOs !== onlyOn) {
      findings.push(
        finding(
          "voice-catalogue-belongs-to-the-claimed-os",
          "voices",
          `no ${onlyOn}-only voices on a ${claimedOs} machine`,
          matched.join(", "),
          `${why}, so they cannot be installed on the claimed platform.`,
        ),
      );
    }
  }
  const language = (locale ?? "").split("-")[0].toLowerCase();
  const tags = voiceTags(voices);
  if (language && tags.length > 0 && !tags.some((tag) => tag.toLowerCase().startsWith(language))) {
    findings.push(
      finding(
        "voice-catalogue-covers-the-claimed-locale",
        "voices",
        `at least one \`${language}\` voice for locale ${locale}`,
        tags.join(", "),
        "A machine configured for a language ships a voice for it; a catalogue with none is a " +
          "stronger tell than the language setting it contradicts.",
      ),
    );
  }
}

/**
 * R9b — a layout coordinate is a multiple of 1/64 of a CSS pixel.
 *
 * Chromium stores geometry in `LayoutUnit`, which is fixed-point with 64 subdivisions per px, so every
 * coordinate a real build hands to JS is `n / 64` for some integer `n` — and a 1/64 multiple is a
 * dyadic rational, so `value * 64` is exactly an integer in float64 with no tolerance needed. That
 * makes this the cheapest possible test for an *additive* perturbation of layout values: one
 * expression, no seed knowledge, no spec sheet, and it does not care what the value should have been.
 *
 * Measured firing on every seeded snapshot that carries `014` (5.1l, 6.13): the patch adds a
 * seed-derived ±0.001 px offset to `x` and `y` while leaving `width`, `height` and every
 * inter-element distance bit-identical, so it perturbs the half of the surface fingerprinters ignore
 * and lands the half they do read off the grid. `Number.isInteger(rect.x * 64)` is `true` upstream and
 * `false` there.
 *
 * Scope is deliberately the probe's own fixture. A CSS transform with a non-dyadic scale legitimately
 * produces off-grid rects, so this is a rule about the one untransformed element the probe measures,
 * not about `getClientRects` in general.
 *
 * `clientRects` is deliberately **not** in `DRIVEN_SURFACES`, so a hit here scores as a floor. That is
 * not a softening: since T09 dropped `014`, nothing in the patch set steers this surface, and a
 * dialect table that named a patch which no longer exists is the stale mirror this file's own header
 * warns about. The 57 committed snapshots that fire it are measurements of engines that shipped `014`,
 * with the off-grid value printed beside the rule; whoever perturbs layout again owes the
 * `DRIVEN_SURFACES` entry in the same commit, and then it scores as a defect again (6.13).
 */
const LAYOUT_UNITS_PER_PX = 64;

function clientRectRules(canvas, findings) {
  const rects = canvas?.clientRects ?? null;
  if (!rects || rects.unavailable) {
    return;
  }
  for (const key of ["x", "y", "width", "height"]) {
    const value = rects[key];
    if (typeof value !== "number" || !Number.isFinite(value)) {
      continue;
    }
    if (!Number.isInteger(value * LAYOUT_UNITS_PER_PX)) {
      findings.push(
        finding(
          "client-rects-are-on-the-layout-unit-grid",
          "clientRects",
          `${key} × 64 is an integer, because layout geometry is LayoutUnit`,
          `${value}`,
          "Chromium quantises layout to 1/64 px, so an off-grid coordinate is an additive " +
            "perturbation applied after layout — visible in one expression, with no knowledge of what " +
            "the value should have been.",
        ),
      );
    }
  }
}

/**
 * R6c — a machine claiming Apple Silicon has a WebGPU adapter.
 *
 * Chrome has shipped WebGPU enabled by default on macOS since 113, and its macOS backend is Metal,
 * which every Apple Silicon part supports — so `navigator.gpu.requestAdapter()` resolves to an adapter
 * on every Mac a persona can claim. This is a statement about what Chromium ships per platform, the
 * same class of claim as `PLATFORM_IDENTITY`'s strings, not a hardware spec sheet: the rule never asks
 * what an M3's limits are, only whether the API answers at all.
 *
 * It is macOS-only on purpose. "`navigator.gpu` exists and every adapter is null" is an ordinary
 * Windows or Linux reading — the original GT 7xx host proved this, with a GPU below Dawn's D3D12
 * requirement and stock Chrome behaving identically (6.7) — so the same shape carries no
 * contradiction there. On a macOS claim it is the loudest thing the WebGPU block can say, and it is
 * visible on a host that cannot measure WebGPU at all, which is why this rule could be written before
 * a suitable host was available.
 *
 * A GPU-bearing Windows host exposes the second contradiction T08 must close: WebGL names the Apple
 * Silicon part from `011`, while WebGPU names the host vendor. The availability rule cannot detect
 * that case because requestAdapter() already succeeds. Both findings stay floors until `964` lands.
 */
function webgpuRule(claimedOs, webgl, misc, findings) {
  const webgpu = misc?.webgpu;
  if (!webgpu) {
    return;
  }
  const renderer = webgl?.unmaskedRenderer ?? "";
  // RTX 30-series parts are Ampere; Blackwell is the RTX 50 generation. This narrow Windows
  // cross-check needs no Apple measurements and catches the RTX 30-series claim on a Blackwell host.
  if (
    claimedOs === "windows" &&
    !webgpu.unavailable &&
    /GeForce RTX 30\d\d/.test(renderer) &&
    typeof webgpu.architecture === "string" &&
    webgpu.architecture.toLowerCase() === "blackwell"
  ) {
    findings.push(
      finding(
        "rtx30-webgpu-is-not-blackwell",
        "webgpuIdentity",
        "an RTX 30-series WebGL GPU does not expose a Blackwell WebGPU architecture",
        `WebGL ${renderer}; WebGPU architecture ${webgpu.architecture}`,
        "The two APIs expose one GPU. RTX 30-series parts use Ampere, while Blackwell names the " +
          "RTX 50 generation; this pair combines two physical cards in one browser.",
      ),
    );
  }
  if (claimedOs !== "macos") {
    return;
  }
  if (webgpu.unavailable) {
    findings.push(
      finding(
        "apple-silicon-implies-a-webgpu-adapter",
        "webgpu",
        "navigator.gpu.requestAdapter() resolves to an adapter",
        `${webgpu.unavailable}`,
        "Chrome enables WebGPU by default on macOS and reaches it through Metal, which every Apple " +
          "Silicon part supports, so a Mac that cannot produce an adapter is not a Mac. Ordinary on " +
          "Windows and Linux, where the API is present and the hardware may still be below Dawn's bar.",
      ),
    );
    return;
  }
  const vendor = webgpu.vendor;
  if (
    /ANGLE Metal Renderer: Apple M\d/.test(renderer) &&
    typeof vendor === "string" &&
    vendor &&
    vendor.toLowerCase() !== "apple"
  ) {
    findings.push(
      finding(
        "webgpu-vendor-follows-the-apple-webgl-gpu",
        "webgpuIdentity",
        "WebGPU vendor is apple when WebGL names an Apple Silicon GPU",
        `WebGL ${renderer}; WebGPU vendor ${vendor}`,
        "Both APIs expose the same machine's GPU. An Apple Silicon WebGL renderer beside a " +
          "non-Apple WebGPU adapter names two different devices in one persona.",
      ),
    );
  }
}

/**
 * R12 — `baseLatency` is a whole hardware buffer divided by the sample rate.
 *
 * The spec defines it as the greater of the hardware buffer and the render quantum, over the sample
 * rate, so `baseLatency × sampleRate` is an integer frame count on every real build — and Chromium
 * bounds that count itself: `media/base/limits.h` declares `kMinWebAudioBufferSize` 128 and
 * `kMaxWebAudioBufferSize` 8192. Arithmetic plus two in-tree constants, no device data sheet.
 *
 * It is exactly the test a substituted latency fails when it was chosen as a *duration* rather than as a
 * buffer. The counterpart's persona carries 0.015, which is 661.5 frames at 44.1 kHz — not a buffer any
 * backend can report, and the reason `967` substitutes the frame count and lets Blink's own division
 * produce the latency (6.14).
 *
 * The product is compared against its own rounding rather than with `Number.isInteger`: both factors
 * come back as doubles from the page, and 1e-6 of a frame is four orders of magnitude tighter than the
 * smallest step any backend takes.
 */
const WEB_AUDIO_MIN_BUFFER_FRAMES = 128;
const WEB_AUDIO_MAX_BUFFER_FRAMES = 8192;

function audioLatencyRules(audio, findings) {
  const rate = audio?.sampleRate ?? null;
  const base = audio?.baseLatency ?? null;
  if (!Number.isFinite(rate) || !Number.isFinite(base) || rate <= 0 || base <= 0) {
    return;
  }
  const frames = base * rate;
  const rounded = Math.round(frames);
  if (
    Math.abs(frames - rounded) > 1e-6 ||
    rounded < WEB_AUDIO_MIN_BUFFER_FRAMES ||
    rounded > WEB_AUDIO_MAX_BUFFER_FRAMES
  ) {
    findings.push(
      finding(
        "audio-base-latency-is-a-whole-buffer",
        "audio",
        `baseLatency × sampleRate is a whole number of frames in [${WEB_AUDIO_MIN_BUFFER_FRAMES}, ${WEB_AUDIO_MAX_BUFFER_FRAMES}]`,
        `${frames} frames (baseLatency ${base} at ${rate} Hz)`,
        "`baseLatency` is the hardware buffer over the sample rate, and Chromium bounds that buffer at " +
          "128 and 8192 frames, so a value that is not a whole buffer in that range was picked as a " +
          "duration rather than measured as one.",
      ),
    );
  }
}

/**
 * R13 — a Mac decodes HEVC. R14 — the codec APIs cannot disagree with each other.
 *
 * R13 is the same class of claim as `apple-silicon-implies-a-webgpu-adapter`, and admissible for the same
 * reason: it is a statement about what Chromium ships per platform, not a hardware data sheet. Chromium
 * reaches HEVC on macOS through VideoToolbox (`media/gpu/mac/video_toolbox_video_decoder.cc`), and every
 * Mac Apple has shipped since Kaby Lake answers `VTIsHardwareDecodeSupported` yes — so "HEVC unsupported"
 * on a macOS claim does not describe an unusual Mac, it describes something that is not one. Measured
 * firing on the shipping engine, where the persona claims an M3 and the host cannot decode HEVC at all.
 *
 * R14 needs no platform claim and no external truth: `canPlayType`, `MediaSource.isTypeSupported` and
 * `decodingInfo().supported` are three front doors to `media::IsSupportedVideoType`, so they agree on
 * every real build by construction. A spoof that moves one and forgets another is visible in one line of
 * JS — the same shape `gpu-limits-are-arithmetically-consistent` catches in the WebGL block, and the
 * reason `969` answers in `supported_types.cc` rather than at the three entry points. Its second half is
 * pure arithmetic: nothing decodes efficiently, or smoothly, what it cannot decode.
 */
const HEVC_MIME = 'video/mp4; codecs="hvc1.1.6.L93.B0"';

function codecRules(claimedOs, misc, findings) {
  const canPlay = misc?.codecSupport?.canPlayType ?? null;
  const mse = misc?.codecSupport?.mediaSourceIsTypeSupported ?? null;
  const decoding = misc?.mediaCapabilities ?? null;
  if (!canPlay || canPlay.unavailable) {
    return;
  }
  const hevcCanPlay = canPlay[HEVC_MIME];
  const hevcMse = mse && !mse.unavailable ? mse[HEVC_MIME] : null;
  const hevcDecoding = decoding && !decoding.unavailable ? decoding.hevc : null;

  if (claimedOs === "macos" && hevcCanPlay === "") {
    findings.push(
      finding(
        "hevc-support-follows-the-claimed-os",
        "codecs",
        "HEVC is supported on a macOS claim",
        `canPlayType("${HEVC_MIME}") = ""`,
        "Chromium reaches HEVC on macOS through VideoToolbox, and every Mac Apple has shipped since " +
          "Kaby Lake decodes it in hardware, so an unsupported answer describes a machine that is not a " +
          "Mac. A statement about what Chromium ships per platform, not a data sheet.",
      ),
    );
  }

  const answers = [
    ["canPlayType", hevcCanPlay === undefined ? null : hevcCanPlay !== ""],
    ["MediaSource.isTypeSupported", typeof hevcMse === "boolean" ? hevcMse : null],
    [
      "decodingInfo().supported",
      hevcDecoding && typeof hevcDecoding.supported === "boolean" ? hevcDecoding.supported : null,
    ],
  ].filter(([, value]) => value !== null);
  if (answers.length > 1 && new Set(answers.map(([, value]) => value)).size > 1) {
    findings.push(
      finding(
        "codec-queries-agree-with-each-other",
        "codecs",
        "the three HEVC queries return one answer",
        answers.map(([name, value]) => `${name}: ${value}`).join(", "),
        "`canPlayType`, `MediaSource.isTypeSupported` and `decodingInfo().supported` are three front " +
          "doors to one support check, so they agree on every real build. A disagreement is a spoof that " +
          "moved one of them.",
      ),
    );
  }

  if (decoding && !decoding.unavailable) {
    for (const [family, triple] of Object.entries(decoding)) {
      if (!triple || typeof triple !== "object" || triple.supported !== false) {
        continue;
      }
      if (triple.powerEfficient === true || triple.smooth === true) {
        findings.push(
          finding(
            "codec-queries-agree-with-each-other",
            "codecs",
            `${family}: smooth and powerEfficient are false when supported is false`,
            `supported ${triple.supported}, smooth ${triple.smooth}, powerEfficient ${triple.powerEfficient}`,
            "A codec the engine cannot decode cannot be decoded smoothly or efficiently.",
          ),
        );
      }
    }
  }
}

/**
 * R15 — the macOS `monospace` generic is not the same face as `serif` or `sans-serif`.
 *
 * The macOS generic should resolve to Menlo rather than the proportional default. A Windows
 * default in zh-CN is not a safe control for this rule: stock Chrome 154 and the engine both measured
 * `monospace` and `serif` as 792×72 on the current host. Scoring that as an impossible Windows
 * machine would make a stock browser fail assertion 6, so the rule is scoped to the macOS claim it
 * was written to check.
 *
 * Measured firing on the `persona` case before `972`: `monospace` and `serif` both reported 1009×80,
 * because the host's fixed family under `--lang=en-US` is `Consolas`, a ClearType face no macOS table
 * lists, so `906` hid it and the generic landed on Times New Roman (6.16). It is the narrow form of the
 * rule on purpose: `serif` and `sans-serif` may legitimately coincide — the macOS grd names `Times` for
 * both `serif` and the standard family — and a CJK host can have one face behind two keywords.
 *
 * Scored against `fonts` on a seeded macOS claim, where `906` / `972` own the generic fallback.
 */
function genericFamilyRules(claimedOs, fonts, findings) {
  const baselines = fonts?.baselines ?? null;
  if (!baselines || claimedOs !== "macos") {
    return;
  }
  const mono = baselines.monospace;
  if (!mono || !Number.isFinite(mono.width)) {
    return;
  }
  const size = (value) => (value ? `${value.width}×${value.height}` : null);
  for (const key of ["serif", "sans-serif"]) {
    const other = baselines[key];
    if (!other || !Number.isFinite(other.width)) {
      continue;
    }
    if (size(mono) === size(other)) {
      findings.push(
        finding(
          "monospace-is-not-the-proportional-face",
          "fonts",
          `monospace measures differently from ${key}`,
          `both measure ${size(mono)}`,
          "Chrome ships a distinct monospace default on every desktop platform, so a monospace that " +
            "measures exactly like the proportional face beside it was never resolved — it fell " +
            "through, which is what hiding the host's fixed family without providing the claimed " +
            "platform's does.",
        ),
      );
    }
  }
}

/**
 * R10 — no Mac has a touchscreen, so a macOS claim commits `maxTouchPoints` to 0. Deliberately not
 * extended to Windows or Linux, where touch laptops are ordinary and the rule would be a guess.
 */
function touchRule(claimedOs, navigator, findings) {
  const points = navigator?.maxTouchPoints ?? null;
  if (claimedOs === "macos" && points !== null && points > 0) {
    findings.push(
      finding(
        "macos-reports-no-touch-points",
        "touch",
        "maxTouchPoints = 0",
        `${points}`,
        "Apple has never shipped a Mac with a touchscreen.",
      ),
    );
  }
}

/**
 * R16 — `maxTouchPoints > 0` and `(any-pointer: coarse)` are one fact read twice.
 *
 * Not a table and not a market claim: on all three desktop platforms in this tree both answers come
 * from the same "is a touch digitizer present" test, so they agree on every real build by
 * construction. Windows gates `POINTER_TYPE_COARSE` and `MaxTouchPoints()` on the same
 * `IsTouchDevicePresent()` (`SM_DIGITIZER` with `NID_READY`), Linux gates both on
 * `DeviceDataManager`'s touchscreen list, and macOS returns the constants `0` and `{FINE, HOVER}` from
 * `pointer_device_default.cc`. The same identity shape as `codec-queries-agree-with-each-other`.
 *
 * It is the rule that catches the shape the counterpart ships. clearcote's `150` overrides
 * `NavigatorEvents::maxTouchPoints` alone, so a persona reports 0 touch points while
 * `(any-pointer: coarse)` still matches — a spoof visible in one more line of CSS than the one the
 * page already read, and measured on this host before `961`: 10 and `coarse` together, then 0 and no
 * `coarse` together, on four different command lines (6.18).
 *
 * Scored against `touch`, which `961` drives, so a hit is a defect. Desktop claims only: `android` and
 * `ios` emit no `--fingerprint-platform` at all, so nothing here speaks for them.
 */
function pointerAgreementRule(claimedOs, navigator, misc, findings) {
  const queries = misc?.mediaQueries;
  const points = navigator?.maxTouchPoints ?? null;
  if (!queries || points === null || !PLATFORM_IDENTITY[claimedOs]) {
    return;
  }
  const coarse = queries["(any-pointer: coarse)"];
  if (typeof coarse !== "boolean") {
    return;
  }
  if (coarse !== points > 0) {
    findings.push(
      finding(
        "touch-points-agree-with-the-pointer-media-features",
        "touch",
        "(any-pointer: coarse) matches exactly when maxTouchPoints > 0",
        `maxTouchPoints ${points}, (any-pointer: coarse) ${coarse}`,
        "Both answers come from one digitizer test in Chromium — the same `IsTouchDevicePresent()` on " +
          "Windows, the same device list on Linux, the same two constants on macOS — so a build where " +
          "they disagree has had one of them moved and not the other.",
      ),
    );
  }
}

/**
 * R17 — `navigator.deviceMemory` is one of the five values a desktop Chromium can report, and R18 —
 * the JS heap limit is not larger than the claimed RAM allows.
 *
 * R17 is `approximated_device_memory.cc` read out loud: physical memory is rounded to the nearer
 * neighbouring power of two, divided by 1024, then clamped into `[2, 32]` on desktop. Everything below
 * 2 clamps up and everything above 32 clamps down, so the reportable set is exactly five numbers and a
 * sixth is a value no Chrome produces. `905` falls through to the host rather than clamping a bad
 * switch value into range, which is what makes this checkable rather than unreachable.
 *
 * R18 is the cross-check that makes a claimed RAM size worth anything, and it needs two constants
 * rather than a spec sheet. V8 sets its old generation to
 * `clamp(physical_memory / 2, kDefaultMinHeapSize, kDefaultMaxHeapSize)` — 256 MB and **4 GB** on
 * 64-bit (`v8/src/heap/heap.cc:349-361`, `heap.h:314-316`) — and adds a young generation of at most
 * three times `DefaultMaxSemiSpaceSize`, 32 MiB on desktop (`heap.cc:334-352`, `4834-4852`). So the
 * limit is bounded above by `min(4 GiB, physical / 2) + 96 MiB`, and the widest physical memory a
 * claim of `gb` can stand for is `1.5 × gb` (the top of the rounding band).
 *
 * Only the upper bound is asserted. A limit *smaller* than the claim allows has legitimate causes —
 * `--js-flags`, a low-memory device signal — while a limit *larger* than it allows is the interesting
 * shape: the host's heap beside the persona's RAM. Measured: this 32 GiB host reports 4192 MiB, which
 * is exactly the bound for a claim of 8 GB, because both saturate at V8's 4 GB cap. That is also why
 * the surface was coherent for a claim of 8 or more before anything drove it, and why `905` making 2
 * and 4 reachable is what creates work for T14 (6.18).
 */
const DEVICE_MEMORY_REPORTABLE_GB = [2, 4, 8, 16, 32];
const V8_MAX_OLD_GENERATION_BYTES = 4 * 1024 ** 3;
const V8_YOUNG_GENERATION_ALLOWANCE_BYTES = 3 * 32 * 1024 ** 2;
const DEVICE_MEMORY_ROUNDING_TOP = 1.5;

function deviceMemoryRules(navigator, misc, findings) {
  const gb = navigator?.deviceMemory ?? null;
  if (gb === null || !Number.isFinite(gb)) {
    return;
  }
  if (!DEVICE_MEMORY_REPORTABLE_GB.includes(gb)) {
    findings.push(
      finding(
        "device-memory-is-a-value-chromium-reports",
        "memory",
        `navigator.deviceMemory is one of ${DEVICE_MEMORY_REPORTABLE_GB.join(", ")}`,
        `${gb}`,
        "Chromium rounds physical memory to the nearer power of two and clamps the result into " +
          "[2, 32] on desktop, so no other value is reachable — a sixth number is a substitution that " +
          "skipped the clamp.",
      ),
    );
  }
  const limit = misc?.performanceMemory?.jsHeapSizeLimit ?? null;
  if (limit === null || !Number.isFinite(limit)) {
    return;
  }
  const widestPhysical = gb * DEVICE_MEMORY_ROUNDING_TOP * 1024 ** 3;
  const bound =
    Math.min(V8_MAX_OLD_GENERATION_BYTES, widestPhysical / 2) + V8_YOUNG_GENERATION_ALLOWANCE_BYTES;
  if (limit > bound) {
    findings.push(
      finding(
        "js-heap-limit-follows-the-claimed-memory",
        "memory",
        `performance.memory.jsHeapSizeLimit is at most ${bound} for a ${gb} GB claim`,
        `${limit}`,
        "V8 takes its old generation from half the machine's physical memory, capped at 4 GB, and adds " +
          "at most 96 MiB of young generation. A limit above that bound is the host's heap sitting " +
          "beside the persona's RAM.",
      ),
    );
  }
}

/**
 * R11 — the hardware the persona asked for is the hardware the page sees.
 *
 * Both halves are defects now. `cores` has been driven by `005` since Tier 1, and `deviceMemory`
 * joined it in Stage 6 T15: `005` returned 8 unconditionally, with no switch and no seed involvement,
 * so a mismatch there was the documented floor from 5.1e until `905` gave the field
 * `--fingerprint-device-memory` and `DRIVEN_SURFACES` gained `memory` in the same commit (6.18).
 */
function hardwareRules(fingerprint, navigator, findings) {
  const requested = [
    ["cores", "hardwareConcurrency", fingerprint.hardwareConcurrency],
    ["memory", "deviceMemory", fingerprint.deviceMemory],
  ];
  for (const [surface, key, want] of requested) {
    const got = navigator?.[key] ?? null;
    if (want > 0 && got !== null && got !== want) {
      findings.push(
        finding(
          `${key}-matches-the-request`,
          surface,
          `navigator.${key} = ${want}`,
          `${got}`,
          "The persona asked for this value; the page sees another one.",
        ),
      );
    }
  }
}

/**
 * R19 — the plugin block is the spec'd five-entry PDF set, or nothing at all.
 *
 * Since crbug.com/1164635 and whatwg/html#6738, `navigator.plugins` is not the host's plugin list at
 * all: a build with a PDF viewer exposes the fixed five entries hardcoded in
 * `dom_plugin_array.cc` (`PDF Viewer`, `Chrome PDF Viewer`, `Chromium PDF Viewer`, `Microsoft Edge PDF
 * Viewer`, `WebKit built-in PDF`, each filename `internal-pdf-viewer` carrying `application/pdf` and
 * `text/pdf`) with `pdfViewerEnabled` true, and a build without one exposes the empty list and false.
 * One gate — `DOMPluginArray::IsPdfViewerAvailable()` — answers all three counts, so any mixed shape
 * (a list without a PDF viewer, a PDF viewer with no list, or a length that is neither 0 nor 5) is a
 * substitution applied after the gate, not a browser that could exist.
 *
 * That mixed shape is exactly what clearcote's plugin hunk produces when its mobile branch is applied
 * to a persona that is not mobile — `pdfViewerEnabled` false beside a populated list — which is the
 * reason the hunk was left unported (6.20). Scored as a floor: nothing steers the surface, and T23
 * measured it identical to stock Chrome in every persona configuration this host can run.
 */
const PLUGIN_SET_LENGTH = 5;
const PLUGIN_MIME_TYPE_COUNT = 2;

function pluginRules(navigator, findings) {
  const count = navigator?.pluginCount ?? null;
  const mimeCount = navigator?.mimeTypeCount ?? null;
  const pdf = navigator?.pdfViewerEnabled ?? null;
  if (count === null || mimeCount === null || pdf === null) {
    return;
  }
  const coherent =
    (pdf === true && count === PLUGIN_SET_LENGTH && mimeCount === PLUGIN_MIME_TYPE_COUNT) ||
    (pdf === false && count === 0 && mimeCount === 0);
  if (!coherent) {
    findings.push(
      finding(
        "plugins-are-the-spec-pdf-set-or-none",
        "plugins",
        `pdfViewerEnabled ${pdf} with the matching list — ${PLUGIN_SET_LENGTH} plugins and ${PLUGIN_MIME_TYPE_COUNT} mime types when true, none when false`,
        `plugins ${count}, mimeTypes ${mimeCount}, pdfViewerEnabled ${pdf}`,
        "One gate — whether the build has a PDF viewer — answers all three counts, and the five entries " +
          "it populates are hardcoded in Blink, not read from the host. A mixed shape is a substitution " +
          "applied after that gate.",
      ),
    );
  }
}

/**
 * R20 — `window.chrome` carries exactly the keys Chromium itself installs on a web page.
 *
 * Three of them are unconditional in a chrome/ build: `app` (`kWebAvailableFeatures` installs it with
 * the comment "chrome.app is exposed to all webpages"), and `csi`/`loadTimes`
 * (`loadtimes_extension_bindings.cc`). The fourth, `runtime`, is installed only when an *installed
 * extension* declares the page's URL externally_connectable
 * (`NativeExtensionBindingsSystem::UpdateBindingsForContext`), so stock Chrome on a clean profile
 * reports exactly `["app", "csi", "loadTimes"]` — measured on stock Chrome beside the engine, 6.20.
 * The set is therefore pinned from both sides: the three unconditional keys must be present, and no
 * key outside the four Chromium can install may appear. A page-side fabricator that adds `runtime`
 * unconditionally — the classic JS spoof, since the key's presence encodes installed extensions — and
 * a build missing one of the three are both caught in one expression.
 *
 * Scored as a floor: nothing steers it, and T23 measured it identical to stock Chrome in every
 * persona configuration this host can run (6.20).
 */
const CHROME_KEYS_ALWAYS_INSTALLED = ["app", "csi", "loadTimes"];
const CHROME_KEYS_CHROMIUM_CAN_INSTALL = ["app", "csi", "loadTimes", "runtime"];

function chromeKeysRule(misc, findings) {
  const keys = misc?.chromeObjectKeys;
  if (!Array.isArray(keys)) {
    return;
  }
  const missing = CHROME_KEYS_ALWAYS_INSTALLED.filter((key) => !keys.includes(key));
  const extra = keys.filter((key) => !CHROME_KEYS_CHROMIUM_CAN_INSTALL.includes(key));
  if (missing.length > 0 || extra.length > 0) {
    findings.push(
      finding(
        "chrome-keys-are-the-stock-set",
        "chromeKeys",
        `the three unconditional keys (${CHROME_KEYS_ALWAYS_INSTALLED.join(", ")}) and nothing outside ${CHROME_KEYS_CHROMIUM_CAN_INSTALL.join(", ")}`,
        JSON.stringify(keys),
        "`app`, `csi` and `loadTimes` are installed on every web page of a chrome build, and `runtime` " +
          "only when an installed extension declares the page externally_connectable — so a key outside " +
          "that envelope was fabricated, and a missing unconditional key is a build that is not Chrome.",
      ),
    );
  }
}

/**
 * R21 — a fresh fixture profile has answered no permission prompt.
 *
 * The probe runs in a throwaway profile against its own loopback origin, so nothing has ever granted
 * or denied notifications there: the main frame reads `default`, and the cross-site iframe reads
 * `denied` because Chromium's permission policy denies it inside a frame the embedder did not
 * delegate — stock Chrome reports the same pair (6.20). A `granted` or `denied` in the main frame
 * means the capture is reading a profile with the host's permission state in it, which is the leak
 * the throwaway profile exists to keep out of the baseline.
 *
 * Scored as a floor: nothing steers it, and T23 measured it identical to stock Chrome (6.20).
 */
function notificationPermissionRule(misc, findings) {
  const permission = misc?.notificationPermission;
  if (permission === null || permission === undefined || permission === "default") {
    return;
  }
  findings.push(
    finding(
      "notification-permission-is-the-fresh-profile-default",
      "notifications",
      "Notification.permission = default in a profile that has answered no prompt",
      `${permission}`,
      "The probe's profile is throwaway and its origin is a loopback fixture, so the permission has " +
        "never been answered; anything else is host permission state sitting inside the capture.",
    ),
  );
}

/**
 * R22 — the canvas 2D context's own attribute set carries no device identity.
 *
 * `getContextAttributes()` on a 2D context is a readback of the *page's own* creation dictionary —
 * `BaseRenderingContext2D::getContextAttributes()` returns
 * `ToCanvasRenderingContext2DSettings(CreationAttributes())`, and `CreationAttributes()` is the
 * `CanvasContextCreationAttributesCore` the page's `getContext("2d", {...})` call populated. Measured
 * (6.21): with `alpha:false, colorSpace:"display-p3", desynchronized:true` requested, both stock
 * Chrome and the engine echo exactly that; with nothing requested, both report the same six defaults
 * (`alpha:true, colorSpace:"srgb", colorType:"unorm8", desynchronized:false,
 * toneMapping:{mode:"standard"}, willReadFrequently:false`) — on `default`, on a macOS persona, and
 * under `--lang=de-DE` alike. Same shape as `963`'s `contextAttributes` re-scope: this row was
 * assigned a patch on the assumption it was a device surface, and the measurement says it is a mirror.
 *
 * What the rule pins is the echo relationship, because that is the part a spoof could break: the probe
 * requests `willReadFrequently: true`, so a context that reports `false` there — or an `alpha` /
 * `colorSpace` other than the defaults the probe's request leaves in place — is a dictionary rewritten
 * after creation, not a browser answer. One expression, no spec sheet.
 *
 * `canvasContext` is not in `DRIVEN_SURFACES`: nothing steers it, and T21 measured it identical to
 * stock Chrome in every configuration this host can run (6.21). `965` is not owed — the same verdict
 * T23's behaviour block received — and this rule is what keeps that reading checkable.
 */
function canvasContextAttributesRule(canvas, findings) {
  const attrs = canvas?.contextAttributes;
  if (!attrs || attrs.unavailable) {
    return;
  }
  // The probe's own request, which the readback has to echo, and the defaults its request does not
  // touch. `toneMapping.mode` is the newer member and is recorded as an object.
  const expected = {
    alpha: true,
    colorSpace: "srgb",
    willReadFrequently: true,
  };
  const mismatched = Object.entries(expected).filter(([key, value]) => attrs[key] !== value);
  const toneMappingOk =
    attrs.toneMapping === undefined ||
    attrs.toneMapping === null ||
    attrs.toneMapping?.mode === "standard";
  if (mismatched.length === 0 && toneMappingOk) {
    return;
  }
  const observed =
    mismatched.length > 0
      ? mismatched.map(([key]) => `${key}=${JSON.stringify(attrs[key])}`).join(", ")
      : `toneMapping.mode=${attrs.toneMapping?.mode}`;
  findings.push(
    finding(
      "canvas-context-attributes-echo-the-request",
      "canvasContext",
      `getContextAttributes() echoes what the page requested — the probe asked for willReadFrequently:true and left the rest at the defaults ${JSON.stringify(expected)}`,
      observed,
      "A 2D context's attribute dictionary is the page's own creation request read back " +
        "(base_rendering_context_2d.cc → ToCanvasRenderingContext2DSettings), so it carries no device " +
        "identity — measured identical to stock Chrome on default, persona and de-DE runs (6.21). A " +
        "value that differs from the request is a dictionary rewritten after creation.",
    ),
  );
}

/**
 * R23 — the canvas 2D text-rendering state reads the spec'd defaults the probe leaves it in.
 *
 * The eleven text attributes (`textRendering`, `fontKerning`, `fontStretch`, `fontVariantCaps`,
 * `letterSpacing`, `wordSpacing`, `direction`, `textAlign`, `textBaseline`,
 * `imageSmoothingEnabled`, `imageSmoothingQuality`) are stored per-context state with spec'd
 * defaults, and the probe sets none of them except `textBaseline`, which it sets to the default's own
 * value. Measured (6.21): every one reads its default on stock Chrome and the engine alike —
 * including `direction`, the only member with a conditional in it, which resolves `inherit` to `rtl`
 * on an RTL document on **both** binaries (a DOM computation, not a platform one). The one that is
 * *not* platform-neutral in a real browser is `imageSmoothingQuality`, whose default `low` is what a
 * desktop Chromium reports; a persona does not change rasteriser defaults, and nothing in the patch
 * set steers this state.
 *
 * A patch that ever does drive text rendering (the `965` the plan priced) owes the `DRIVEN_SURFACES`
 * entry in the same commit, per the 6.13 precedent; until then a finding here is a floor that names
 * a state no switch reaches, and the value is a substitution applied to state the page can also set.
 */
const CANVAS_TEXT_DEFAULTS = {
  textRendering: "auto",
  fontKerning: "auto",
  fontStretch: "normal",
  fontVariantCaps: "normal",
  letterSpacing: "0px",
  wordSpacing: "0px",
  direction: "ltr",
  textAlign: "start",
  textBaseline: "alphabetic",
  imageSmoothingEnabled: true,
  imageSmoothingQuality: "low",
};

function canvasTextStateRule(canvas, findings) {
  const text = canvas?.textRendering;
  if (!text || text.unavailable) {
    return;
  }
  const mismatched = Object.entries(CANVAS_TEXT_DEFAULTS).filter(
    ([key, value]) => text[key] !== value,
  );
  if (mismatched.length === 0) {
    return;
  }
  findings.push(
    finding(
      "canvas-text-state-is-the-spec-defaults",
      "canvasText",
      `the eleven text attributes at their spec'd defaults (${Object.keys(CANVAS_TEXT_DEFAULTS).length} keys, probe sets none)`,
      mismatched.map(([key]) => `${key}=${JSON.stringify(text[key])}`).join(", "),
      "These are per-context state with spec'd defaults, not device facts — measured identical to " +
        "stock Chrome on default, persona and de-DE runs, with `direction` resolving `inherit` " +
        "against the document on both binaries (6.21). A non-default value the probe did not set is " +
        "state a patch or injection rewrote, and the page can set the same state itself.",
    ),
  );
}

/**
 * R24 — a granted geolocation read answers the position the command line declared.
 *
 * `970` answers `navigator.geolocation.getCurrentPosition()` from `--fingerprint-location=lat,lng`:
 * the coordinates come back exactly as declared, `accuracy` 100, altitude/heading/speed the
 * not-a-value sentinels a desktop without the sensors reports. The rule scores the one read the
 * engine can be caught lying about: parse the declared value off the launch argv (the same story
 * `drivenBy` consults), and when the fixture's permission state is `granted` — the only world in
 * which a page sees coordinates at all — the position must be the declared one. A
 * `POSITION_UNAVAILABLE` beside a declared position means the switch reached an engine whose patch
 * set does not read it (the stock-dialect lie, 6.9's first instance); coordinates that differ mean
 * the persona disagrees with its own command line.
 *
 * When the permission is `prompt` — the fixture's default, and the honest shape for the committed
 * captures — the position is deliberately unread and the rule has nothing to score: declaring a
 * position does not fabricate a permission grant, and granting one in the harness would be the
 * probe inventing a world no page sees.
 */
function geolocationRule(argv, misc, findings) {
  const declared = argv.find((arg) => arg.startsWith("--fingerprint-location="));
  if (!declared) {
    return;
  }
  const [latitudeText, longitudeText] = declared.slice("--fingerprint-location=".length).split(",");
  const declaredLatitude = Number(latitudeText);
  const declaredLongitude = Number(longitudeText);
  const geolocation = misc?.geolocation;
  if (
    !Number.isFinite(declaredLatitude) ||
    !Number.isFinite(declaredLongitude) ||
    !geolocation ||
    geolocation.unavailable ||
    geolocation.permissionState !== "granted"
  ) {
    return;
  }
  const position = geolocation.position;
  const coherent =
    position &&
    !position.unavailable &&
    position.errorCode === undefined &&
    Math.abs(position.latitude - declaredLatitude) <= 1e-4 &&
    Math.abs(position.longitude - declaredLongitude) <= 1e-4;
  if (!coherent) {
    findings.push(
      finding(
        "geolocation-answers-the-declared-position",
        "geolocation",
        `getCurrentPosition() reports the declared position (${declaredLatitude}, ${declaredLongitude}) once the permission is granted`,
        JSON.stringify(position),
        "`970` answers geolocation from --fingerprint-location, so a granted read that errors or " +
          "differs means the switch reached no patch that reads it, or the persona contradicts its " +
          "own command line.",
      ),
    );
  }
}

/**
 * Veil `990` (T24-a): the android persona's own coherence rules. The android claim arrives on
 * the launch argv (`--fingerprint-platform=android`) rather than through the case file's `os`
 * — the persona case still declares its machine the way it always did — so these read argv the
 * way `geolocationRule` does, and score nothing on a desktop run.
 *
 * Each rule is the shape `990` implements, stated as a machine that could exist:
 * a phone has a touchscreen (961's row count), a portrait panel with the work area equal to
 * the full screen (Android reports no shell inset), a UA carrying the ` Mobile` token beside
 * UA-CH `mobile: true` (one claim, two reads), GLES-shaped WebGL answers from the phone row,
 * an empty voice catalogue (no bound local TTS engine), no PDF-viewer plugin, and a battery
 * on the device rather than a desktop's AC-forever set.
 */
function androidPersonaRules(argv, main, findings) {
  const platformArg = argv.find((arg) => arg.startsWith("--fingerprint-platform="));
  if (!platformArg || platformArg.slice("--fingerprint-platform=".length) !== "android") {
    return;
  }
  const navigator = main.navigator ?? {};
  const screen = main.screen ?? {};
  const misc = main.misc ?? {};

  const touch = navigator.maxTouchPoints;
  if (typeof touch === "number" && touch <= 0) {
    findings.push(
      finding(
        "android-carries-a-touchscreen",
        "touch",
        "maxTouchPoints > 0",
        `${touch}`,
        "`990` answers the phone row's touch count through 961's own hook; a zero on an android " +
          "claim is the desktop digitizer answer leaking through.",
      ),
    );
  }

  if (
    typeof screen.width === "number" &&
    typeof screen.height === "number" &&
    screen.height <= screen.width
  ) {
    findings.push(
      finding(
        "android-panel-is-portrait",
        "screen",
        "screen.height > screen.width on a phone",
        `${screen.width}x${screen.height}`,
        "`990`'s phone rows are all portrait; a landscape panel beside an android claim is a " +
          "desktop monitor wearing a phone's identity.",
      ),
    );
  }

  if (
    typeof screen.width === "number" &&
    typeof screen.availWidth === "number" &&
    (screen.availWidth !== screen.width || screen.availHeight !== screen.height)
  ) {
    findings.push(
      finding(
        "android-work-area-is-the-full-panel",
        "screen",
        "screen.availWidth/Height == screen.width/Height",
        `screen ${screen.width}x${screen.height}, avail ${screen.availWidth}x${screen.availHeight}`,
        "Chrome on a phone reports the work area equal to the full screen (990's display arm); a " +
          "shell inset beside an android claim is the desktop shell leaking through.",
      ),
    );
  }

  const ua = navigator.userAgent ?? "";
  const uaHasMobileToken = /\bMobile\b/.test(ua);
  const uaChMobile = navigator.userAgentData?.mobile;
  if (typeof uaChMobile === "boolean" && uaHasMobileToken !== uaChMobile) {
    findings.push(
      finding(
        "mobile-token-agrees-with-ua-ch",
        "identity",
        'the UA " Mobile" token and UA-CH mobile are the same claim',
        `UA ${uaHasMobileToken ? "has" : "lacks"} Mobile, userAgentData.mobile ${JSON.stringify(uaChMobile)}`,
        "`990` drives both from one android claim; a page cross-checks them in one line, so they " +
          "cannot be allowed to disagree.",
      ),
    );
  }

  const voices = misc.speechVoices;
  if (voices && typeof voices.count === "number" && voices.count > 0) {
    findings.push(
      finding(
        "android-reports-no-local-voices",
        "voices",
        "getVoices() is empty on a phone with no bound local TTS engine",
        `${voices.count} voice(s), first: ${JSON.stringify(voices.names?.[0] ?? null)}`,
        "`990`'s android arm returns an empty catalogue — the host's SAPI roster beside a phone " +
          "claim is exactly the leak this patch set exists to close.",
      ),
    );
  }

  if (navigator.pdfViewerEnabled === true) {
    findings.push(
      finding(
        "android-ships-no-pdf-viewer",
        "plugins",
        "pdfViewerEnabled is false on a phone",
        "true",
        "`990` gates IsPdfViewerAvailable() off for the android claim, which empties plugins, " +
          "mimeTypes and pdfViewerEnabled in one gate (the shape 6.20's rule already scores).",
      ),
    );
  }

  const battery = misc.battery;
  if (battery && typeof battery.charging === "boolean") {
    const dischargingFinite =
      typeof battery.dischargingTime === "number" && Number.isFinite(battery.dischargingTime);
    const coherent = battery.charging === false && dischargingFinite;
    if (!coherent) {
      findings.push(
        finding(
          "android-battery-is-on-the-device",
          "battery",
          "a phone persona reports a discharging battery with a finite estimate",
          `charging ${JSON.stringify(battery.charging)}, dischargingTime ${JSON.stringify(battery.dischargingTime)}`,
          "`990` answers the phone battery from the device, not the host's AC set — the debt " +
            "6.18 recorded when it measured the desktop battery a no-op.",
        ),
      );
    }
  }
}

/**
 * Contradictions this assertion can name but a single snapshot cannot settle. Recorded rather
 * than silently omitted: the WebGL parameter block and extension list are the cross-check that makes
 * a renderer *name* worth anything, and proving they moved needs the unspoofed capture beside the
 * spoofed one — the same two-snapshot shape as assertion 3.
 */
const NOT_SCORABLE_HERE = [
  {
    rule: "gpu-limits-move-with-the-gpu-name",
    surface: "gpuLimits",
    how: "compare.mjs over the `default` and `persona` captures of the same engine: if `webgl.parameters.*` and `webgl.supportedExtensions` are identical while `webgl.unmaskedRenderer` differs, the claimed GPU is contradicted by its own capabilities.",
  },
  {
    // Presence and metrics are two surfaces, and `906` closes only the first. A cross-OS persona
    // reports `Menlo` present by rendering it through a face the *host* owns, so the family list
    // can name one OS while every advance width belongs to another — and on a Windows host that
    // cannot be fixed in the engine: Menlo's widths need Menlo's outlines, Windows ships no
    // metric-compatible clone, and Veil bundles no font files (6.8).
    //
    // Deliberately not a scored rule, and the reason is this file's own charter. Deciding from one
    // snapshot that "Menlo measures like Tahoma rather than like Menlo" requires knowing what Menlo
    // measures — a font-metric spec sheet, which is exactly the kind of external table the header
    // above refuses because it would rot. What *is* decidable is whether the persona's per-family
    // sizes are drawn from the host's own set, and that needs the host capture beside it.
    rule: "font-metrics-belong-to-the-claimed-os",
    surface: "fontMetrics",
    how: "compare.mjs over the `default` and `persona` captures of the same engine, on `fonts.metrics` (probe v5): if a family present only under the persona reports a size that the `default` capture reports for one of the host's own families, that family was rendered through a host face and its advance widths name the host's OS, not the claimed one. Needs probe v5 on both sides — a v4 snapshot has no `fonts.metrics` and is not scorable rather than passing.",
  },
];

/**
 * Score assertion 6 against one capture.
 *
 * `fingerprintSwitchPresent` is passed in rather than recomputed so there is one definition of "the
 * engine is running patched code" shared with assertion 1. Without it a clean result would read `pass`
 * on the `default` case, where the persona is the host and agrees with itself for free.
 */
export function evaluateCoherence({ realms, caseMeta, engine, launch, fingerprintSwitchPresent }) {
  const dialect = engine?.flagDialect ?? "stock-chromium";
  const argv = [...(launch?.argv ?? []), ...(launch?.injectedArgs ?? [])];
  const main = realms.main ?? {};
  const claimedOs = caseMeta.os ?? null;
  const declaredDisplay = caseMeta.machine?.display ?? null;
  const findings = [];

  screenShapeRules(main.screen, claimedOs, findings);
  declaredDisplayRules(declaredDisplay, main.screen, findings);
  derivedDisplayRules(main.screen, findings);
  mediaQueryAgreementRules(main.misc, findings);
  appleSiliconRetinaRule(claimedOs, main.navigator, main.screen, findings);
  identityRules(claimedOs, main.navigator, findings);
  gpuRules(claimedOs, main.webgl, findings);
  gpuLimitRules(main.webgl, findings);
  webgpuRule(claimedOs, main.webgl, main.misc, findings);
  clientRectRules(main.canvas, findings);
  fontRules(claimedOs, main.fonts, findings);
  genericFamilyRules(claimedOs, main.fonts, findings);
  voiceRules(claimedOs, caseMeta.fingerprint?.locale ?? "", main.misc, findings);
  audioLatencyRules(main.audio, findings);
  codecRules(claimedOs, main.misc, findings);
  touchRule(claimedOs, main.navigator, findings);
  pointerAgreementRule(claimedOs, main.navigator, main.misc, findings);
  deviceMemoryRules(main.navigator, main.misc, findings);
  hardwareRules(caseMeta.fingerprint ?? {}, main.navigator, findings);
  pluginRules(main.navigator, findings);
  chromeKeysRule(main.misc, findings);
  notificationPermissionRule(main.misc, findings);
  canvasContextAttributesRule(main.canvas, findings);
  canvasTextStateRule(main.canvas, findings);
  geolocationRule(argv, main.misc, findings);
  androidPersonaRules(argv, main, findings);

  const scored = findings.map((entry) => ({
    ...entry,
    drivenBy: drivenBy(dialect, entry.surface, argv),
  }));
  const defects = scored.filter((entry) => entry.drivenBy !== null);
  const floors = scored.filter((entry) => entry.drivenBy === null);

  let status;
  let reason;
  if (defects.length > 0) {
    status = "fail";
    reason =
      "A surface the patch set drives reported a value no real machine can produce. Every entry in " +
      "`defects` names the patch responsible, so each one is actionable without further measurement.";
  } else if (floors.length > 0) {
    status = "known-floor";
    reason =
      "Every contradiction is on a surface no switch reaches under this dialect, so the persona is " +
      "as coherent as the current engine can make it. These are the Stage 6 targets, in the order " +
      "they appear: a floor recorded here is a patch that has not been written, not a regression.";
  } else if (claimedOs === null) {
    // Not a pass. Most rules compare an observation against the platform the persona claims, and a
    // case with no `os` gives them nothing to compare against — the platform-independent ones ran
    // and found nothing, which is a much weaker result than `pass` and must not be spelled the same.
    status = "n/a";
    reason =
      "The case declares no platform, so only the platform-independent rules ran: the arithmetic on " +
      "`screen.*` and the single-OS font check. Add `os` to the case file to score the rest.";
  } else {
    status = fingerprintSwitchPresent ? "pass" : "vacuous-pass";
    reason = fingerprintSwitchPresent
      ? "Every rule held against a persona the engine actually spoofed: the presented machine could " +
        "exist."
      : "No fingerprint switch reached the engine, so the persona is the host and agrees with itself " +
        "for free. A control, not a test.";
  }

  return {
    status,
    reason,
    claimedOs,
    declaredMachine: caseMeta.machine?.label ?? null,
    dialect,
    defects,
    floors,
    notScorableHere: NOT_SCORABLE_HERE,
  };
}
