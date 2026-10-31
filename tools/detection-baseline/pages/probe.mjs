/**
 * The read-only fingerprint probe. One module, three realms: it is imported by the main frame, by
 * a cross-site iframe, and by a module Worker, so all three read the *same* code and any divergence
 * is the engine's, not the harness's.
 *
 * Rules this file lives by:
 *
 * - It only **reads**. Nothing here patches, overrides, or defines a property. The engine is the
 *   only thing allowed to change a fingerprint surface.
 * - Every input is fixed. Canvas geometry, draw ops, text, fonts, shader source, oscillator
 *   settings and the font probe list are all constants, because a snapshot is only diffable if the
 *   input never moves.
 * - Expensive or binary readbacks are hashed, not stored: the tracked value is "did it change".
 * - Values that cannot exist in a realm are recorded as `{ unavailable: "..." }` rather than
 *   omitted, so a surface never silently disappears from a later snapshot.
 */

/**
 * Bump when the probe's *behaviour* changes, so a snapshot says which instrument produced it.
 *
 * - 1: initial (2026-08-17).
 * - 2: `getClientRects()` waits for the realm to acquire a layout box before measuring (2026-08-20).
 *   Healthy realms answer on the first call, so no value a version-1 snapshot recorded changes.
 * - 3: `misc.speechVoices.langs` added (2026-08-24), for assertion 6's locale-coverage rule. Purely
 *   additive — every value a version-2 snapshot recorded is unchanged — but `probeVersion` itself
 *   lives under `realms.`, so a v2↔v3 `compare.mjs` run reports it as a surface difference per realm.
 *   That is the same one-off cost the 1→2 bump had; compare v3 against v3.
 * - 4: Stage 6 T02 — the nine surfaces the inventory listed as *not captured*, so each one has a
 *   "before" value its patch can be measured against (2026-08-24). Also purely additive:
 *   `webgl.webgl2Parameters`, `webgl.extensionParameters`, `webgl.shaderPrecisionFormats`,
 *   `canvas.contextAttributes`, `canvas.textRendering`, `audio.baseLatency`, `audio.outputLatency`,
 *   `misc.gamepads`, `misc.sensors`, `misc.geolocation`, `misc.mediaCapabilities`,
 *   `misc.codecSupport`. No existing value moves, and again `probeVersion` is under `realms.` —
 *   compare v4 against v4.
 *
 *   Two of the eleven read a **permission-gated** API, and both obey the rule the `queryLocalFonts`
 *   comment below states: an unattended run must never be able to hang on a permission prompt.
 *   `misc.geolocation` reports capability plus `permissions.query()` state and reads coordinates
 *   *only* when the state is already `granted`, so a default capture records `prompt` and prompts
 *   nobody. `misc.sensors` reports constructor presence and permission state without ever calling
 *   `start()`. Neither grants itself anything: granting geolocation in the harness would make every
 *   future snapshot describe a world no ordinary page sees.
 * - 5: Stage 6 T04 — `fonts.metrics`, one measured size per probed family (2026-08-26). Purely
 *   additive and free: the probe already took this measurement to decide presence and threw the
 *   number away, so v4 and v5 agree on `present`, `presentCount` and `baselines` leaf for leaf.
 *   It exists because presence and metrics are two different surfaces and only one of them was
 *   captured: `006` reports a claimed-platform family present by rendering it through a **host**
 *   face, so a font list can be made single-OS while every advance width still belongs to the host.
 *   Three generic baselines cannot see that; 85 per-family sizes can.
 * - 6: Stage 6 T05 — three `(resolution:)` queries and `misc.mediaQueryAgreement` (2026-08-27). The
 *   display block names `(resolution: …)` as one of its eight surfaces and the probe had no value for
 *   it, so that surface was *not captured* and a patch on it would have been unverifiable. The two
 *   additions do different jobs on purpose: the three fixed queries are the surface — they flip when
 *   the spoofed `devicePixelRatio` moves — while `mediaQueryAgreement` is an invariant that is `true`
 *   on any real browser before and after, because it asks the media-query side about the value
 *   `screen.*` / `devicePixelRatio` just reported. A patch that moves `screen.width` and forgets
 *   `MediaValues::CalculateDeviceWidth` leaves the first set unchanged and flips the second to
 *   `false`, which is the half-finished shape assertion 6 cannot see from either surface alone.
 *
 * Version 7 (T19) adds no read. It corrects one: `classifyAddress` called a 198.18.0.0/15 TUN gateway
 * `ipv4-public`, so the same address now reports `ipv4-benchmark` and CGNAT space reports
 * `ipv4-shared`. The version moves anyway, because a snapshot diff has to be able to tell a changed
 * *classification* from a changed *address*.
 *
 * Version 8 (T22) adds `misc.keyboardLayoutMap` -- the full code -> key map, sorted, beside the size
 * `971` leaves untouched. The size alone cannot measure `971`: the patch substitutes per-locale key
 * *values* while the key *set* stays the host's, so a German QWERTZ map and the host's US one both
 * report 48 on this build host (measured, 6.22). Additive; compare v8 against v8.
 *
 * Version 9 (T08) records the sorted WebGPU feature names and every exposed adapter limit. The
 * previous `featureCount` could not show which capability moved, and captured no limits at all.
 * Additive on a host without a WebGPU adapter; compare v9 against v9 on an adapter-bearing host.
 */
export const PROBE_VERSION = 9;

const CANVAS_WIDTH = 260;
const CANVAS_HEIGHT = 88;
const CANVAS_TEXT = "Veil baseline — 0123456789 \u{1F510}";
const MEASURE_TEXT = "Cwm fjordbank glyphs vext quiz — 0123456789";
const MEASURE_FONT = "27px 'Segoe UI', Arial, sans-serif";
const RECT_TARGET_ID = "veil-rect-target";
/**
 * How long to wait for the measured element to acquire a layout box. A cross-site iframe whose
 * rendering lifecycle Chromium has throttled has no layout tree yet, and `getClientRects()` then
 * returns an empty list for an element that plainly has a box — measured once in roughly nine
 * `persona` captures, where it fails assertion 2 with the iframe reporting `count: 0` against the
 * main frame's `1`. That is a false regression, and Stage 5 diffs sixteen patches against this
 * baseline. A healthy realm answers on the first call, so this costs nothing when layout is ready.
 */
const RECT_LAYOUT_TIMEOUT_MS = 2000;
const RECT_LAYOUT_POLL_MS = 50;
const READ_PIXELS_SIZE = 64;
const AUDIO_SAMPLE_RATE = 44100;
const AUDIO_FRAMES = AUDIO_SAMPLE_RATE * 3;
const HIGH_ENTROPY_HINTS = [
  "platform",
  "platformVersion",
  "architecture",
  "bitness",
  "model",
  "uaFullVersion",
  "fullVersionList",
  "wow64",
  "formFactors",
];
const MEDIA_QUERIES = [
  "(prefers-color-scheme: dark)",
  "(prefers-reduced-motion: reduce)",
  "(hover: hover)",
  "(any-hover: hover)",
  "(pointer: fine)",
  "(any-pointer: coarse)",
  "(dynamic-range: high)",
  "(forced-colors: active)",
  "(display-mode: browser)",
  // Probe v6. `(resolution:)` is block B's eighth surface and had no captured value before this,
  // which made `960` unverifiable on it. Three fixed queries rather than one: `1dppx` and `2dppx`
  // pin the two values a desktop persona would ever claim, and the `min-` query catches the
  // fractional Windows scale factors (1.25 / 1.5 / 1.75) that neither exact query would see.
  "(resolution: 1dppx)",
  "(resolution: 2dppx)",
  "(min-resolution: 1.5dppx)",
];

/**
 * How long to wait for a coordinate fix. Bounded for the same reason `RECT_LAYOUT_TIMEOUT_MS` is: a
 * capture runs unattended, and a surface that can block forever is worse than one recorded as absent.
 */
const GEOLOCATION_TIMEOUT_MS = 4000;

/**
 * The four generic sensors block C names, plus the two legacy device-motion event constructors. All
 * six are absent on an ordinary Windows desktop and present on a phone, so the *set* is a platform
 * tell in one read — which is what makes it worth a "before" value even though this host has no
 * sensor hardware to report.
 */
const SENSOR_PROBES = [
  ["accelerometer", "Accelerometer"],
  ["gyroscope", "Gyroscope"],
  ["magnetometer", "Magnetometer"],
  ["ambientLightSensor", "AmbientLightSensor"],
  ["deviceMotionEvent", "DeviceMotionEvent"],
  ["deviceOrientationEvent", "DeviceOrientationEvent"],
];

/**
 * Permission names queried without ever triggering a prompt. `permissions.query()` reports the
 * current state only; it is `navigator.geolocation.getCurrentPosition()` and `Sensor.start()` that
 * prompt, and neither is called unless the state is already `granted`.
 */
const PERMISSION_PROBES = [
  "geolocation",
  "accelerometer",
  "gyroscope",
  "magnetometer",
  "ambient-light-sensor",
];

/**
 * Fixed `decodingInfo()` configurations, one per codec family. Hardware decode support differs by OS
 * and by GPU, so these answers must end up coherent with the spoofed platform and renderer — the
 * cross-check T11 exists to close. Every field is pinned: a bitrate or a framerate that moved would
 * make the answers incomparable between snapshots.
 */
const MEDIA_DECODING_CONFIGS = [
  ["h264", 'video/mp4; codecs="avc1.42E01E"'],
  ["vp8", 'video/webm; codecs="vp8"'],
  ["vp9", 'video/webm; codecs="vp09.00.10.08"'],
  ["av1", 'video/mp4; codecs="av01.0.04M.08"'],
  ["hevc", 'video/mp4; codecs="hvc1.1.6.L93.B0"'],
];
const MEDIA_DECODING_VIDEO = { width: 1920, height: 1080, bitrate: 4000000, framerate: 30 };

/**
 * MIME strings for the two synchronous codec queries. `canPlayType` and
 * `MediaSource.isTypeSupported` disagree in useful ways — the first is about playback, the second
 * about Media Source Extensions — so both are recorded rather than one standing in for the other.
 */
const CODEC_PROBES = [
  'video/mp4; codecs="avc1.42E01E"',
  'video/mp4; codecs="avc1.640028"',
  'video/mp4; codecs="hvc1.1.6.L93.B0"',
  'video/mp4; codecs="av01.0.04M.08"',
  'video/webm; codecs="vp8"',
  'video/webm; codecs="vp9"',
  'video/ogg; codecs="theora"',
  "audio/mpeg",
  'audio/mp4; codecs="mp4a.40.2"',
  'audio/webm; codecs="opus"',
  'audio/ogg; codecs="vorbis"',
  "audio/flac",
  'audio/wav; codecs="1"',
];

/**
 * Fonts worth probing: the Windows 10/11 core and Office sets (what this engine should report),
 * plus macOS and Linux families (what it must NOT report, and what a future `--fonts` patch will
 * have to fake coherently).
 */
const FONT_PROBES = [
  "Agency FB",
  "Arial",
  "Arial Black",
  "Arial Narrow",
  "Bahnschrift",
  "Bookman Old Style",
  "Calibri",
  "Cambria",
  "Candara",
  "Comic Sans MS",
  "Consolas",
  "Constantia",
  "Corbel",
  "Courier New",
  "Ebrima",
  "Franklin Gothic Medium",
  "Gabriola",
  "Gadugi",
  "Garamond",
  "Georgia",
  "HoloLens MDL2 Assets",
  "Impact",
  "Ink Free",
  "Javanese Text",
  "Leelawadee UI",
  "Lucida Console",
  "Lucida Sans Unicode",
  "MS Gothic",
  "MV Boli",
  "Malgun Gothic",
  "Marlett",
  "Microsoft Himalaya",
  "Microsoft JhengHei",
  "Microsoft New Tai Lue",
  "Microsoft PhagsPa",
  "Microsoft Sans Serif",
  "Microsoft Tai Le",
  "Microsoft YaHei",
  "Microsoft Yi Baiti",
  "MingLiU-ExtB",
  "Mongolian Baiti",
  "Myanmar Text",
  "Nirmala UI",
  "Palatino Linotype",
  "Rockwell",
  "Segoe MDL2 Assets",
  "Segoe Print",
  "Segoe Script",
  "Segoe UI",
  "Segoe UI Emoji",
  "Segoe UI Historic",
  "Segoe UI Symbol",
  "SimSun",
  "Sitka",
  "Sylfaen",
  "Symbol",
  "Tahoma",
  "Times New Roman",
  "Trebuchet MS",
  "Verdana",
  "Webdings",
  "Wingdings",
  "Yu Gothic",
  "Apple Color Emoji",
  "Avenir",
  "Baskerville",
  "Didot",
  "Geneva",
  "Helvetica",
  "Helvetica Neue",
  "Hiragino Sans",
  "Lucida Grande",
  "Menlo",
  "Monaco",
  "Optima",
  "PingFang SC",
  "Cantarell",
  "DejaVu Sans",
  "DejaVu Serif",
  "FreeSans",
  "Liberation Sans",
  "Noto Sans",
  "Noto Sans CJK SC",
  "Source Han Sans",
  "Ubuntu",
];

/**
 * Collect every surface this realm can see. Never throws: a surface that blows up is recorded as
 * `{ unavailable }` so one broken read cannot cost the whole snapshot.
 */
export async function collect(realm) {
  const hasDocument = typeof document !== "undefined";
  const result = {
    probeVersion: PROBE_VERSION,
    realm,
    origin: typeof location !== "undefined" ? location.origin : null,
    isSecureContext: typeof isSecureContext === "boolean" ? isSecureContext : null,
    crossOriginIsolated: typeof crossOriginIsolated === "boolean" ? crossOriginIsolated : null,
    navigator: await safe(() => navigatorSurface()),
    screen: hasDocument ? await safe(() => screenSurface()) : unavailable("no window in a Worker"),
    intl: await safe(() => intlSurface()),
    canvas: await safe(() => canvasSurface(hasDocument)),
    webgl: await safe(() => webglSurface(hasDocument)),
    audio: hasDocument
      ? await safe(() => audioSurface())
      : unavailable("OfflineAudioContext is Window-only"),
    fonts: hasDocument ? await safe(() => fontSurface()) : unavailable("font probing needs a DOM"),
    misc: await safe(() => miscSurface(hasDocument)),
    webrtc:
      typeof RTCPeerConnection === "function"
        ? await safe(() => webrtcSurface())
        : unavailable("RTCPeerConnection is Window-only"),
    natives: await safe(() => nativeSourceSurface()),
  };
  return sanitize(result);
}

async function safe(fn) {
  try {
    return await fn();
  } catch (error) {
    return unavailable(`${error && error.name ? error.name : "Error"}: ${error && error.message}`);
  }
}

function unavailable(reason) {
  return { unavailable: reason };
}

async function navigatorSurface() {
  const n = navigator;
  const surface = {
    userAgent: n.userAgent,
    appVersion: valueOrNull(n.appVersion),
    platform: valueOrNull(n.platform),
    vendor: valueOrNull(n.vendor),
    product: valueOrNull(n.product),
    language: valueOrNull(n.language),
    languages: Array.isArray(n.languages) ? [...n.languages] : null,
    hardwareConcurrency: valueOrNull(n.hardwareConcurrency),
    deviceMemory: valueOrNull(n.deviceMemory),
    webdriver: "webdriver" in n ? n.webdriver : null,
    maxTouchPoints: valueOrNull(n.maxTouchPoints),
    cookieEnabled: valueOrNull(n.cookieEnabled),
    doNotTrack: valueOrNull(n.doNotTrack),
    pdfViewerEnabled: valueOrNull(n.pdfViewerEnabled),
    pluginCount: n.plugins ? n.plugins.length : null,
    mimeTypeCount: n.mimeTypes ? n.mimeTypes.length : null,
    userAgentData: null,
  };
  if (n.userAgentData) {
    surface.userAgentData = {
      brands: n.userAgentData.brands.map((entry) => ({
        brand: entry.brand,
        version: entry.version,
      })),
      mobile: n.userAgentData.mobile,
      platform: n.userAgentData.platform,
      // Requested one hint at a time on purpose: an unrecognised hint rejects the whole call, so
      // asking individually records exactly which hints this engine implements.
      highEntropy: await highEntropyHints(n.userAgentData),
    };
  }
  return surface;
}

async function highEntropyHints(uaData) {
  const hints = {};
  for (const hint of HIGH_ENTROPY_HINTS) {
    try {
      const values = await uaData.getHighEntropyValues([hint]);
      hints[hint] = hint in values ? values[hint] : null;
    } catch (error) {
      hints[hint] = { unavailable: error && error.name ? error.name : "Error" };
    }
  }
  return hints;
}

function screenSurface() {
  const s = screen;
  return {
    width: s.width,
    height: s.height,
    availWidth: s.availWidth,
    availHeight: s.availHeight,
    availLeft: valueOrNull(s.availLeft),
    availTop: valueOrNull(s.availTop),
    colorDepth: s.colorDepth,
    pixelDepth: s.pixelDepth,
    isExtended: valueOrNull(s.isExtended),
    orientation: s.orientation ? { type: s.orientation.type, angle: s.orientation.angle } : null,
    devicePixelRatio: window.devicePixelRatio,
    // Window geometry is per-realm by definition — an iframe's inner size is the frame's box, not
    // the tab's. Excluded from the cross-realm equality check for that reason.
    outerWidth: window.outerWidth,
    outerHeight: window.outerHeight,
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
  };
}

/**
 * Does the media-query side of the engine agree with the `screen` / `window` side? Probe v6.
 *
 * Every entry is `true` on any real browser and on the zero-patch floor, so this is an **invariant**
 * rather than a fingerprint: `screen.width`, `screen.height` and `devicePixelRatio` come from
 * `Screen` / `LocalDOMWindow`, and `(device-width:)`, `(device-height:)` and `(resolution:)` come
 * from `MediaValues` — two code paths for one fact about the display. A display patch that moves the
 * first and forgets the second leaves `misc.mediaQueries` byte-identical and flips these to `false`,
 * which is the only place in the snapshot where that half-finished shape is visible.
 *
 * Built from the live values instead of from fixed strings for exactly that reason: the question is
 * "do the two paths agree", which cannot be asked by a query whose value is hard-coded.
 */
function mediaQueryAgreement() {
  const ask = (query) => window.matchMedia(query).matches;
  return {
    deviceWidthMatchesScreen: ask(`(device-width: ${screen.width}px)`),
    deviceHeightMatchesScreen: ask(`(device-height: ${screen.height}px)`),
    resolutionMatchesDevicePixelRatio: ask(`(resolution: ${window.devicePixelRatio}dppx)`),
  };
}

function intlSurface() {
  // A fixed instant as well as "now": `new Date()` moves across a DST boundary, so the fixed
  // January/July offsets are the diffable pair and `now` is documented as expected-to-vary.
  const january = new Date(Date.UTC(2026, 0, 15, 12, 0, 0));
  const july = new Date(Date.UTC(2026, 6, 15, 12, 0, 0));
  return {
    dateTimeFormat: Intl.DateTimeFormat().resolvedOptions(),
    numberFormatLocale: Intl.NumberFormat().resolvedOptions().locale,
    collatorLocale: Intl.Collator().resolvedOptions().locale,
    timezoneOffsetNow: new Date().getTimezoneOffset(),
    timezoneOffsetJanuary: january.getTimezoneOffset(),
    timezoneOffsetJuly: july.getTimezoneOffset(),
    longZoneNameJanuary: new Intl.DateTimeFormat("en-US", { timeZoneName: "long" }).format(january),
    longZoneNameJuly: new Intl.DateTimeFormat("en-US", { timeZoneName: "long" }).format(july),
  };
}

function drawCanvasScene(ctx) {
  ctx.fillStyle = "#f0f6ff";
  ctx.fillRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
  const gradient = ctx.createLinearGradient(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
  gradient.addColorStop(0, "#7c3aed");
  gradient.addColorStop(0.5, "#0ea5e9");
  gradient.addColorStop(1, "#f59e0b");
  ctx.fillStyle = gradient;
  ctx.fillRect(8, 8, CANVAS_WIDTH - 16, 22);
  // Overlapping alpha circles under `multiply` are the classic canvas-fingerprint recipe: the
  // result depends on the rasteriser, not just on the inputs.
  ctx.globalCompositeOperation = "multiply";
  for (const [x, colour] of [
    [56, "rgba(239,68,68,0.65)"],
    [96, "rgba(34,197,94,0.65)"],
    [136, "rgba(59,130,246,0.65)"],
  ]) {
    ctx.beginPath();
    ctx.arc(x, 46, 22, 0, Math.PI * 2, true);
    ctx.closePath();
    ctx.fillStyle = colour;
    ctx.fill();
  }
  ctx.globalCompositeOperation = "source-over";
  ctx.textBaseline = "alphabetic";
  ctx.font = "18px 'Segoe UI', Arial, sans-serif";
  ctx.fillStyle = "#111827";
  ctx.fillText(CANVAS_TEXT, 6, 66);
  ctx.strokeStyle = "rgba(17,24,39,0.6)";
  ctx.strokeText(CANVAS_TEXT, 6, 84);
}

async function canvasSurface(hasDocument) {
  const canvas = hasDocument
    ? Object.assign(document.createElement("canvas"), {
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
      })
    : new OffscreenCanvas(CANVAS_WIDTH, CANVAS_HEIGHT);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    return unavailable("no 2d context");
  }
  drawCanvasScene(ctx);
  const imageData = ctx.getImageData(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
  if (hasDocument) {
    await waitForRectLayout();
  }
  const surface = {
    width: CANVAS_WIDTH,
    height: CANVAS_HEIGHT,
    // The 2D context's own attribute set, alongside WebGL's. `alpha`/`colorSpace`/`desynchronized`
    // are the values a page reads back after asking for a context, and `willReadFrequently` echoes
    // what this probe requested — recorded so a patch that rewrites the dictionary is visible.
    contextAttributes: contextAttributesOf(ctx),
    textRendering: textRenderingOf(ctx),
    // `toDataURL` is HTMLCanvasElement-only; a Worker's OffscreenCanvas has no equivalent that is
    // guaranteed synchronous, so the Worker realm records the pixel hash alone.
    toDataUrlSha256: hasDocument ? await sha256(canvas.toDataURL()) : null,
    toDataUrlLength: hasDocument ? canvas.toDataURL().length : null,
    getImageDataSha256: await sha256(new Uint8Array(imageData.data.buffer)),
    measureText: measureTextMetrics(ctx),
    clientRects: hasDocument ? clientRectsMetrics() : unavailable("getClientRects needs a DOM"),
  };
  return surface;
}

function contextAttributesOf(ctx) {
  if (typeof ctx.getContextAttributes !== "function") {
    return unavailable("getContextAttributes is absent on this 2D context");
  }
  const attributes = ctx.getContextAttributes();
  return attributes ? plain(attributes) : null;
}

/**
 * The 2D context's text-rendering state, the other half of block D's "context attributes / text
 * rendering mode" row. Every one of these changes how glyphs rasterise, so they belong beside
 * `measureText` — a patch that alters text rendering without moving them would be reporting one thing
 * and drawing another. All are read at their defaults except `textBaseline`, which `drawCanvasScene`
 * sets explicitly to `alphabetic` (the same value the default carries).
 */
const TEXT_RENDERING_KEYS = [
  "textRendering",
  "fontKerning",
  "fontStretch",
  "fontVariantCaps",
  "letterSpacing",
  "wordSpacing",
  "direction",
  "textAlign",
  "textBaseline",
  "imageSmoothingEnabled",
  "imageSmoothingQuality",
];

function textRenderingOf(ctx) {
  const out = {};
  for (const key of TEXT_RENDERING_KEYS) {
    out[key] = key in ctx ? valueOrNull(ctx[key]) : null;
  }
  return out;
}

function measureTextMetrics(ctx) {
  ctx.font = MEASURE_FONT;
  const metrics = ctx.measureText(MEASURE_TEXT);
  const keys = [
    "width",
    "actualBoundingBoxLeft",
    "actualBoundingBoxRight",
    "actualBoundingBoxAscent",
    "actualBoundingBoxDescent",
    "fontBoundingBoxAscent",
    "fontBoundingBoxDescent",
    "emHeightAscent",
    "emHeightDescent",
    "hangingBaseline",
    "alphabeticBaseline",
    "ideographicBaseline",
  ];
  const out = { text: MEASURE_TEXT, font: MEASURE_FONT };
  for (const key of keys) {
    out[key] = key in metrics ? metrics[key] : null;
  }
  return out;
}

function clientRectsMetrics() {
  const target = document.getElementById(RECT_TARGET_ID);
  if (!target) {
    return unavailable(`#${RECT_TARGET_ID} missing from the fixture page`);
  }
  const rects = target.getClientRects();
  const first = rects[0];
  return {
    count: rects.length,
    // `x`/`y`/`top`/`left` depend on where the element sits in its containing document. The fixture
    // pins the container at 0,0 in every realm so they should still match, but they are excluded
    // from the cross-realm equality set because layout position is not a fingerprint surface.
    x: first ? first.x : null,
    y: first ? first.y : null,
    width: first ? first.width : null,
    height: first ? first.height : null,
  };
}

/**
 * Resolve once the measured element has a layout box, or when the wait runs out. Never throws and
 * never blocks past `RECT_LAYOUT_TIMEOUT_MS`: if the box never appears the empty result is recorded
 * exactly as before, so the failure stays visible instead of being papered over.
 */
async function waitForRectLayout() {
  const target = document.getElementById(RECT_TARGET_ID);
  if (!target) {
    return;
  }
  const deadline = Date.now() + RECT_LAYOUT_TIMEOUT_MS;
  while (target.getClientRects().length === 0 && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, RECT_LAYOUT_POLL_MS));
  }
}

const WEBGL_PARAMS = [
  "VENDOR",
  "RENDERER",
  "VERSION",
  "SHADING_LANGUAGE_VERSION",
  "MAX_TEXTURE_SIZE",
  "MAX_CUBE_MAP_TEXTURE_SIZE",
  "MAX_RENDERBUFFER_SIZE",
  "MAX_VERTEX_ATTRIBS",
  "MAX_VERTEX_UNIFORM_VECTORS",
  "MAX_FRAGMENT_UNIFORM_VECTORS",
  "MAX_VARYING_VECTORS",
  "MAX_TEXTURE_IMAGE_UNITS",
  "MAX_COMBINED_TEXTURE_IMAGE_UNITS",
  "MAX_VIEWPORT_DIMS",
  "ALIASED_LINE_WIDTH_RANGE",
  "ALIASED_POINT_SIZE_RANGE",
  "RED_BITS",
  "GREEN_BITS",
  "BLUE_BITS",
  "ALPHA_BITS",
  "DEPTH_BITS",
  "STENCIL_BITS",
];

/**
 * WebGL2-only `getParameter` limits. Read separately from `WEBGL_PARAMS` because a WebGL1 context
 * has none of them, and recording 25 nulls on a WebGL1 engine would be noise rather than a floor.
 * `011` moves the renderer *string* and leaves every number here at the real card's value, which is
 * the hole T07 closes — so these are the numbers that must move with the name.
 */
const WEBGL2_PARAMS = [
  "MAX_3D_TEXTURE_SIZE",
  "MAX_ARRAY_TEXTURE_LAYERS",
  "MAX_COLOR_ATTACHMENTS",
  "MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS",
  "MAX_COMBINED_UNIFORM_BLOCKS",
  "MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS",
  "MAX_DRAW_BUFFERS",
  "MAX_ELEMENT_INDEX",
  "MAX_ELEMENTS_INDICES",
  "MAX_ELEMENTS_VERTICES",
  "MAX_FRAGMENT_INPUT_COMPONENTS",
  "MAX_FRAGMENT_UNIFORM_BLOCKS",
  "MAX_FRAGMENT_UNIFORM_COMPONENTS",
  "MAX_PROGRAM_TEXEL_OFFSET",
  "MAX_SAMPLES",
  "MAX_SERVER_WAIT_TIMEOUT",
  "MAX_TEXTURE_LOD_BIAS",
  "MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS",
  "MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS",
  "MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS",
  "MAX_UNIFORM_BLOCK_SIZE",
  "MAX_UNIFORM_BUFFER_BINDINGS",
  "MAX_VARYING_COMPONENTS",
  "MAX_VERTEX_OUTPUT_COMPONENTS",
  "MAX_VERTEX_UNIFORM_BLOCKS",
  "MAX_VERTEX_UNIFORM_COMPONENTS",
  "MIN_PROGRAM_TEXEL_OFFSET",
  "UNIFORM_BUFFER_OFFSET_ALIGNMENT",
];

/**
 * Limits that live behind an extension rather than on the context. Anisotropy is the one clearcote's
 * persona table drives, so it belongs with the parameter block T07 has to move; the extension is
 * requested by name and a null means the engine does not expose it.
 */
const WEBGL_EXTENSION_PARAMS = [
  ["EXT_texture_filter_anisotropic", "MAX_TEXTURE_MAX_ANISOTROPY_EXT"],
];

/**
 * Every shader-precision combination the API defines: 2 shader types × 6 precision types. The
 * triple each one returns (`rangeMin`, `rangeMax`, `precision`) is driver-derived, so it is one more
 * number set that must agree with a spoofed renderer name.
 */
const SHADER_TYPES = ["VERTEX_SHADER", "FRAGMENT_SHADER"];
const PRECISION_TYPES = [
  "LOW_FLOAT",
  "MEDIUM_FLOAT",
  "HIGH_FLOAT",
  "LOW_INT",
  "MEDIUM_INT",
  "HIGH_INT",
];

const VERTEX_SHADER = `attribute vec2 aPos;
varying vec2 vPos;
void main() {
  vPos = aPos;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

const FRAGMENT_SHADER = `precision mediump float;
varying vec2 vPos;
void main() {
  gl_FragColor = vec4(abs(vPos.x), abs(vPos.y), 0.5 + 0.5 * sin(vPos.x * 6.0), 1.0);
}`;

async function webglSurface(hasDocument) {
  const canvas = hasDocument
    ? Object.assign(document.createElement("canvas"), {
        width: READ_PIXELS_SIZE,
        height: READ_PIXELS_SIZE,
      })
    : new OffscreenCanvas(READ_PIXELS_SIZE, READ_PIXELS_SIZE);
  const options = { preserveDrawingBuffer: true, antialias: false };
  const gl = canvas.getContext("webgl2", options) || canvas.getContext("webgl", options);
  if (!gl) {
    return unavailable("no WebGL context");
  }
  const parameters = {};
  for (const name of WEBGL_PARAMS) {
    parameters[name] = name in gl ? plain(gl.getParameter(gl[name])) : null;
  }
  const isWebgl2 = gl.getParameter(gl.VERSION).startsWith("WebGL 2");
  // WebGL2-only limits, and only on a WebGL2 context: on WebGL1 the enum names are absent and
  // recording 28 nulls would read as a floor rather than as "this API does not exist here".
  const webgl2Parameters = isWebgl2 ? {} : unavailable("not a WebGL 2 context");
  if (isWebgl2) {
    for (const name of WEBGL2_PARAMS) {
      webgl2Parameters[name] = name in gl ? plain(gl.getParameter(gl[name])) : null;
    }
  }
  const extensionParameters = {};
  for (const [extensionName, constantName] of WEBGL_EXTENSION_PARAMS) {
    const extension = gl.getExtension(extensionName);
    extensionParameters[`${extensionName}.${constantName}`] =
      extension && constantName in extension
        ? plain(gl.getParameter(extension[constantName]))
        : null;
  }
  const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
  return {
    contextType: isWebgl2 ? "webgl2" : "webgl",
    contextAttributes: plain(gl.getContextAttributes()),
    parameters,
    webgl2Parameters,
    extensionParameters,
    shaderPrecisionFormats: shaderPrecisionFormats(gl),
    unmaskedVendor: debugInfo ? gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) : null,
    unmaskedRenderer: debugInfo ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) : null,
    supportedExtensions: (gl.getSupportedExtensions() || []).slice().sort(),
    readPixelsSha256: await readPixelsHash(gl),
  };
}

/**
 * `getShaderPrecisionFormat` returns a `WebGLShaderPrecisionFormat` whose three values are prototype
 * getters, so `plain()` — which walks own enumerable entries — would flatten it to `{}`. Read the
 * three explicitly.
 */
function shaderPrecisionFormats(gl) {
  const out = {};
  for (const shaderType of SHADER_TYPES) {
    for (const precisionType of PRECISION_TYPES) {
      const key = `${shaderType}.${precisionType}`;
      if (!(shaderType in gl) || !(precisionType in gl)) {
        out[key] = null;
        continue;
      }
      const format = gl.getShaderPrecisionFormat(gl[shaderType], gl[precisionType]);
      out[key] = format
        ? { rangeMin: format.rangeMin, rangeMax: format.rangeMax, precision: format.precision }
        : null;
    }
  }
  return out;
}

async function readPixelsHash(gl) {
  const program = gl.createProgram();
  for (const [type, source] of [
    [gl.VERTEX_SHADER, VERTEX_SHADER],
    [gl.FRAGMENT_SHADER, FRAGMENT_SHADER],
  ]) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      return { unavailable: `shader: ${gl.getShaderInfoLog(shader)}` };
    }
    gl.attachShader(program, shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    return { unavailable: `link: ${gl.getProgramInfoLog(program)}` };
  }
  gl.useProgram(program);
  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  const vertices = new Float32Array([-0.9, -0.8, 0.9, -0.7, 0.0, 0.95]);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  const location = gl.getAttribLocation(program, "aPos");
  gl.enableVertexAttribArray(location);
  gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 0, 0);
  gl.viewport(0, 0, READ_PIXELS_SIZE, READ_PIXELS_SIZE);
  gl.clearColor(0.1, 0.2, 0.3, 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  const pixels = new Uint8Array(READ_PIXELS_SIZE * READ_PIXELS_SIZE * 4);
  gl.readPixels(0, 0, READ_PIXELS_SIZE, READ_PIXELS_SIZE, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
  return await sha256(pixels);
}

async function audioSurface() {
  const surface = {
    sampleRate: null,
    maxChannelCount: null,
    // Both latencies are derived from the real audio device, so on a spoofed platform they must move
    // with it: a Windows WASAPI baseLatency on a persona claiming macOS is a one-read contradiction.
    baseLatency: null,
    outputLatency: null,
    state: null,
    render: null,
  };
  const AudioCtor = window.AudioContext || window.webkitAudioContext;
  if (AudioCtor) {
    const live = new AudioCtor();
    surface.sampleRate = live.sampleRate;
    surface.maxChannelCount = live.destination.maxChannelCount;
    surface.baseLatency = valueOrNull(live.baseLatency);
    // `outputLatency` is 0 until the graph has actually rendered on some platforms; recorded as read
    // rather than waited for, because the diffable fact is what a page sees on first access.
    surface.outputLatency = valueOrNull(live.outputLatency);
    surface.state = live.state;
    await live.close();
  }
  const offline = new OfflineAudioContext(1, AUDIO_FRAMES, AUDIO_SAMPLE_RATE);
  const oscillator = offline.createOscillator();
  oscillator.type = "triangle";
  oscillator.frequency.value = 10000;
  const compressor = offline.createDynamicsCompressor();
  compressor.threshold.value = -50;
  compressor.knee.value = 40;
  compressor.ratio.value = 12;
  compressor.attack.value = 0;
  compressor.release.value = 0.25;
  oscillator.connect(compressor);
  compressor.connect(offline.destination);
  oscillator.start(0);
  const buffer = await offline.startRendering();
  const channel = buffer.getChannelData(0);
  let sum = 0;
  for (let i = 4500; i < 5000; i += 1) {
    sum += Math.abs(channel[i]);
  }
  surface.render = {
    sampleRate: buffer.sampleRate,
    length: buffer.length,
    // Both a hash (did anything at all change) and a human-readable magnitude (roughly how much),
    // because a bare hash tells you nothing about the size of a regression.
    sha256: await sha256(new Uint8Array(channel.buffer, channel.byteOffset, channel.byteLength)),
    absSum4500to5000: sum,
  };
  return surface;
}

/**
 * Measure-based font detection: render a fixed string in `"<probe>", <generic>` and compare against
 * the generic alone. A difference against *any* generic counts as present — the sensitive
 * definition detectors use. The method is recorded in the snapshot so the numbers stay
 * interpretable if it ever changes.
 *
 * `metrics` records the size every family measured against the **monospace** reference, present or
 * absent, and it is the measurement this function already had to take. Presence and metrics are two
 * surfaces: `006` reports a claimed-platform family present by resolving it to a face the *host*
 * owns, so `Menlo` can be in `present` while its advance widths are Tahoma's. A family that fell
 * back records the monospace baseline exactly, which is itself the evidence that it fell back.
 * Monospace is the reference because a proportional face almost never matches it by accident, which
 * is why font-enumeration detectors use it too.
 */
function fontSurface() {
  const generics = ["monospace", "sans-serif", "serif"];
  const host = document.createElement("div");
  host.setAttribute("aria-hidden", "true");
  host.style.cssText =
    "position:absolute;left:-9999px;top:-9999px;visibility:hidden;font-size:72px;line-height:normal;";
  document.body.appendChild(host);
  const measure = (family) => {
    const span = document.createElement("span");
    span.style.fontFamily = family;
    span.textContent = "mmmmmmmmmmlli WwMmIi0O";
    host.appendChild(span);
    const size = { width: span.offsetWidth, height: span.offsetHeight };
    host.removeChild(span);
    return size;
  };
  try {
    const baselines = {};
    for (const generic of generics) {
      baselines[generic] = measure(generic);
    }
    const present = [];
    const metrics = {};
    for (const font of FONT_PROBES) {
      let detected = false;
      // `generics[0]` is monospace, so the recorded size is taken before any early exit — the
      // measurement order and count are exactly what version 4 performed.
      for (const generic of generics) {
        const size = measure(`"${font}", ${generic}`);
        if (generic === "monospace") {
          metrics[font] = [size.width, size.height];
        }
        if (size.width !== baselines[generic].width || size.height !== baselines[generic].height) {
          detected = true;
          break;
        }
      }
      if (detected) {
        present.push(font);
      }
    }
    return {
      method: "measure-span-vs-generic-baselines/any",
      probed: FONT_PROBES.length,
      presentCount: present.length,
      present,
      baselines,
      metrics,
      // Deliberately NOT called: `queryLocalFonts` shows a permission prompt, which would hang an
      // unattended run. Availability is the baseline fact worth recording.
      queryLocalFontsAvailable: typeof window.queryLocalFonts === "function",
    };
  } finally {
    document.body.removeChild(host);
  }
}

async function miscSurface(hasDocument) {
  const n = navigator;
  const misc = {
    connection: n.connection
      ? {
          downlink: n.connection.downlink,
          effectiveType: n.connection.effectiveType,
          rtt: n.connection.rtt,
          saveData: n.connection.saveData,
        }
      : null,
    battery: await safe(async () => {
      if (typeof n.getBattery !== "function") {
        return unavailable("navigator.getBattery is absent");
      }
      const battery = await n.getBattery();
      return {
        charging: battery.charging,
        level: battery.level,
        chargingTime: battery.chargingTime,
        dischargingTime: battery.dischargingTime,
      };
    }),
    storageEstimate: await safe(async () => {
      if (!n.storage || typeof n.storage.estimate !== "function") {
        return unavailable("navigator.storage.estimate is absent");
      }
      const estimate = await n.storage.estimate();
      // Quota is derived from free disk space and usage from what this throwaway profile wrote, so
      // both move between runs. Recorded anyway: the *shape* of the quota is a coherence surface.
      return { quota: estimate.quota, usage: estimate.usage };
    }),
    performanceMemory:
      typeof performance !== "undefined" && performance.memory
        ? {
            jsHeapSizeLimit: performance.memory.jsHeapSizeLimit,
            totalJSHeapSize: performance.memory.totalJSHeapSize,
            usedJSHeapSize: performance.memory.usedJSHeapSize,
          }
        : null,
    webgpu: await webgpuInfo(),
    // `MediaCapabilities` is exposed on `WorkerNavigator` too, so this one is read in all three
    // realms rather than gated on a document.
    mediaCapabilities: await mediaCapabilityInfo(),
  };
  if (!hasDocument) {
    return misc;
  }
  misc.mediaDevices = await mediaDeviceInfo();
  misc.speechVoices = await speechVoiceInfo();
  misc.gamepads = gamepadInfo();
  misc.sensors = await sensorInfo();
  misc.geolocation = await geolocationInfo();
  misc.codecSupport = codecSupportInfo();
  // probe v8 adds the map's *content* beside its size: `971` substitutes per-locale key values
  // while the key set stays the host's, so the size alone cannot distinguish a German QWERTZ map
  // from the host's US one -- measured 50/50 on this host (6.22). Entries are sorted by code so
  // the leaf is byte-stable across runs.
  const keyboardLayout = await safe(async () => {
    if (!n.keyboard || typeof n.keyboard.getLayoutMap !== "function") {
      return unavailable("navigator.keyboard is absent");
    }
    const map = await n.keyboard.getLayoutMap();
    return Object.fromEntries([...map.entries()].sort(([a], [b]) => (a < b ? -1 : 1)));
  });
  misc.keyboardLayoutMap = keyboardLayout;
  misc.keyboardLayoutMapSize =
    keyboardLayout && keyboardLayout.unavailable
      ? keyboardLayout
      : Object.keys(keyboardLayout).length;
  misc.mediaQueries = Object.fromEntries(
    MEDIA_QUERIES.map((query) => [query, window.matchMedia(query).matches]),
  );
  misc.mediaQueryAgreement = mediaQueryAgreement();
  misc.chromeObjectKeys = window.chrome ? Object.keys(window.chrome).sort() : null;
  misc.notificationPermission = typeof Notification === "function" ? Notification.permission : null;
  return misc;
}

async function webgpuInfo() {
  if (!navigator.gpu) {
    return unavailable("navigator.gpu is absent");
  }
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    return unavailable("no WebGPU adapter");
  }
  const info = adapter.info || {};
  const features = adapter.features ? [...adapter.features].sort() : null;
  // GPUSupportedLimits exposes WebIDL attributes through prototype getters. Object.entries()
  // sees no values on Chrome, so read the attribute names from that prototype explicitly.
  const limits = adapter.limits
    ? Object.fromEntries(
        Object.getOwnPropertyNames(Object.getPrototypeOf(adapter.limits))
          .filter((name) => name !== "constructor")
          .sort()
          .map((name) => [name, adapter.limits[name]]),
      )
    : null;
  return {
    vendor: valueOrNull(info.vendor),
    architecture: valueOrNull(info.architecture),
    device: valueOrNull(info.device),
    description: valueOrNull(info.description),
    featureCount: features?.length ?? null,
    features,
    limits,
  };
}

/**
 * The Gamepad API's shape, not any pad's state. `getGamepads()` returns a fixed-length array padded
 * with nulls, so the connected count is the fact worth recording — and a headless-looking machine
 * that exposes the API with nothing attached is the ordinary desktop answer.
 */
function gamepadInfo() {
  if (typeof navigator.getGamepads !== "function") {
    return unavailable("navigator.getGamepads is absent");
  }
  const pads = navigator.getGamepads();
  const connected = [...pads].filter(Boolean);
  return {
    available: true,
    slotCount: pads.length,
    connectedCount: connected.length,
    // Ids identify the physical pad, so they are a real surface. Empty on this host, and that is the
    // "before" value: a persona must not start reporting a controller nobody plugged in.
    ids: connected.map((pad) => pad.id).sort(),
    mappings: [...new Set(connected.map((pad) => pad.mapping))].sort(),
  };
}

/**
 * Generic-sensor capability and permission state — never a reading. `Sensor.start()` is what asks
 * for permission and what blocks on hardware that will never answer, so it is not called: the
 * diffable facts are which constructors exist and what the permission state is, and on a desktop
 * persona the correct answer for all six is "absent", which is exactly what T13 has to preserve.
 */
async function sensorInfo() {
  const constructors = {};
  for (const [key, name] of SENSOR_PROBES) {
    constructors[key] = typeof globalThis[name] === "function";
  }
  return {
    constructors,
    presentCount: Object.values(constructors).filter(Boolean).length,
    permissions: await permissionStates(),
    note: "capability and permission state only: Sensor.start() would prompt and can never answer on a machine with no sensor",
  };
}

/**
 * `permissions.query()` for the geolocation and sensor names. Reports state; never prompts. A
 * cross-origin iframe without the matching Permissions-Policy allowlist reports `denied` here by
 * policy rather than by fingerprint, which is why these paths are excluded from assertion 2.
 */
async function permissionStates() {
  if (!navigator.permissions || typeof navigator.permissions.query !== "function") {
    return unavailable("navigator.permissions is absent");
  }
  const states = {};
  for (const name of PERMISSION_PROBES) {
    try {
      states[name] = (await navigator.permissions.query({ name })).state;
    } catch (error) {
      states[name] = { unavailable: error && error.name ? error.name : "Error" };
    }
  }
  return states;
}

/**
 * Geolocation capability, permission state, and coordinates **only if already granted**.
 *
 * `getCurrentPosition()` shows a permission prompt, and a capture runs unattended, so a default run
 * must never reach it — the same rule `queryLocalFonts` follows. The engine is also de-Googled and
 * ships no geolocation API key, so even a granted read is expected to fail with
 * `POSITION_UNAVAILABLE` until `970` fabricates a position from `--fingerprint-location`. That error
 * code *is* the before value, and it is what makes `970` measurable.
 */
async function geolocationInfo() {
  if (!navigator.geolocation) {
    return unavailable("navigator.geolocation is absent");
  }
  const permissions = await permissionStates();
  const state =
    permissions && typeof permissions === "object" && typeof permissions.geolocation === "string"
      ? permissions.geolocation
      : null;
  const surface = { available: true, permissionState: state, position: null };
  if (state !== "granted") {
    // Deliberately state-*independent* text. Interpolating the state here made this string diverge
    // between the main frame (`prompt`) and the cross-site iframe (`denied`) — measured — which is a
    // divergence the probe invented rather than one the engine has. `permissionState` above is the
    // single place the state is reported.
    surface.position = unavailable("not read: permission is not granted and asking would prompt");
    return surface;
  }
  surface.position = await new Promise((resolve) => {
    let settled = false;
    const finish = (value) => {
      if (!settled) {
        settled = true;
        resolve(value);
      }
    };
    setTimeout(
      () => finish(unavailable("no verdict within the bounded wait")),
      GEOLOCATION_TIMEOUT_MS,
    );
    navigator.geolocation.getCurrentPosition(
      (position) => {
        const c = position.coords;
        finish({
          latitude: c.latitude,
          longitude: c.longitude,
          accuracy: c.accuracy,
          altitude: valueOrNull(c.altitude),
          altitudeAccuracy: valueOrNull(c.altitudeAccuracy),
          heading: valueOrNull(c.heading),
          speed: valueOrNull(c.speed),
        });
      },
      (error) => finish({ errorCode: error.code, errorName: geolocationErrorName(error.code) }),
      { enableHighAccuracy: false, timeout: GEOLOCATION_TIMEOUT_MS, maximumAge: 0 },
    );
  });
  return surface;
}

function geolocationErrorName(code) {
  return { 1: "PERMISSION_DENIED", 2: "POSITION_UNAVAILABLE", 3: "TIMEOUT" }[code] ?? "UNKNOWN";
}

/**
 * `MediaCapabilities.decodingInfo()` per codec family. The `{supported, smooth, powerEfficient}`
 * triple is derived from the host's actual decode hardware, so on a spoofed GPU it has to move with
 * the renderer name — the coherence T11 owes against T07 and T08.
 */
async function mediaCapabilityInfo() {
  if (
    !navigator.mediaCapabilities ||
    typeof navigator.mediaCapabilities.decodingInfo !== "function"
  ) {
    return unavailable("navigator.mediaCapabilities is absent");
  }
  const out = {};
  for (const [family, contentType] of MEDIA_DECODING_CONFIGS) {
    try {
      const info = await navigator.mediaCapabilities.decodingInfo({
        type: "file",
        video: { contentType, ...MEDIA_DECODING_VIDEO },
      });
      out[family] = {
        supported: info.supported,
        smooth: info.smooth,
        powerEfficient: info.powerEfficient,
      };
    } catch (error) {
      out[family] = { unavailable: error && error.name ? error.name : "Error" };
    }
  }
  return out;
}

/**
 * The two synchronous codec queries over one fixed MIME list. Hardware support for HEVC and H.264
 * differs by OS and GPU, so this block is platform-bound in the same way the font and voice
 * catalogues are — it just answers faster.
 */
function codecSupportInfo() {
  const video = document.createElement("video");
  const audio = document.createElement("audio");
  const canPlayType = {};
  for (const type of CODEC_PROBES) {
    const element = type.startsWith("audio/") ? audio : video;
    // "", "maybe" or "probably" — the empty string is a real answer and is kept as-is.
    canPlayType[type] = element.canPlayType(type);
  }
  const mediaSource = {};
  const hasMediaSource =
    typeof MediaSource === "function" && typeof MediaSource.isTypeSupported === "function";
  for (const type of CODEC_PROBES) {
    mediaSource[type] = hasMediaSource ? MediaSource.isTypeSupported(type) : null;
  }
  return {
    canPlayType,
    mediaSourceIsTypeSupported: hasMediaSource
      ? mediaSource
      : unavailable("MediaSource.isTypeSupported is absent"),
  };
}

async function mediaDeviceInfo() {
  if (!navigator.mediaDevices || typeof navigator.mediaDevices.enumerateDevices !== "function") {
    return unavailable("navigator.mediaDevices is absent");
  }
  const devices = await navigator.mediaDevices.enumerateDevices();
  const kinds = {};
  for (const device of devices) {
    kinds[device.kind] = (kinds[device.kind] || 0) + 1;
  }
  return {
    count: devices.length,
    kinds,
    // Device ids are salted per profile and labels are empty without a granted permission, so
    // neither is diffable. Whether they are empty is the fact that matters.
    labelsAllEmpty: devices.every((device) => device.label === ""),
    deviceIdsAllEmpty: devices.every((device) => device.deviceId === ""),
    supportedConstraintCount: Object.keys(navigator.mediaDevices.getSupportedConstraints() || {})
      .length,
  };
}

async function speechVoiceInfo() {
  if (typeof speechSynthesis === "undefined") {
    return unavailable("speechSynthesis is absent");
  }
  // The voice list populates asynchronously on first access; poll rather than assume.
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const voices = speechSynthesis.getVoices();
    if (voices.length > 0) {
      return {
        count: voices.length,
        names: voices.map((voice) => `${voice.name} (${voice.lang})`).sort(),
        // Recorded on its own as well as inside the name, because assertion 6 asks whether the
        // catalogue covers the locale the persona claims, and recovering a tag by parsing a display
        // name is a dependency on Windows' voice-naming convention rather than on the API.
        langs: [...new Set(voices.map((voice) => voice.lang))].sort(),
        defaultVoice: (voices.find((voice) => voice.default) || {}).name || null,
      };
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return { count: 0, names: [], langs: [], defaultVoice: null };
}

/**
 * Gather ICE candidates from a STUN-less peer connection: no network is contacted, so this measures
 * exactly which local interfaces the engine is willing to expose.
 *
 * The raw address is **not** recorded. A committed snapshot must not carry the maintainer's LAN or
 * public IP, and the diffable fact is the *class* of address, not its value — a host candidate
 * turning from an mDNS hostname into a private IPv4 is the regression.
 */
async function webrtcSurface() {
  const pc = new RTCPeerConnection({ iceServers: [] });
  try {
    pc.createDataChannel("veil-baseline");
    const candidates = [];
    const gathered = new Promise((resolve) => {
      const done = () => resolve();
      pc.addEventListener("icecandidate", (event) => {
        if (!event.candidate) {
          done();
          return;
        }
        candidates.push(event.candidate);
      });
      pc.addEventListener("icegatheringstatechange", () => {
        if (pc.iceGatheringState === "complete") {
          done();
        }
      });
      setTimeout(done, 5000);
    });
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    await gathered;
    const classified = [];
    for (const candidate of candidates) {
      classified.push({
        type: candidate.type,
        protocol: candidate.protocol,
        addressClass: classifyAddress(candidate.address),
        addressSha256: candidate.address ? await sha256(candidate.address) : null,
        relatedAddressClass: classifyAddress(candidate.relatedAddress),
      });
    }
    return {
      gatheringState: pc.iceGatheringState,
      candidateCount: classified.length,
      // Counted from the local description as well, so "no candidates" cannot be confused with
      // "candidates gathered but the harness missed the events". The SDP text itself is not
      // recorded: it carries the addresses this function deliberately does not commit.
      sdpCandidateLineCount: (pc.localDescription?.sdp ?? "")
        .split("\n")
        .filter((line) => line.startsWith("a=candidate")).length,
      candidates: classified,
      classes: [...new Set(classified.map((entry) => entry.addressClass))].sort(),
      revealsPrivateAddress: classified.some((entry) =>
        [
          "ipv4-private",
          "ipv4-link-local",
          "ipv4-shared",
          "ipv4-benchmark",
          "ipv6-link-local",
          "ipv6-unique-local",
        ].includes(entry.addressClass),
      ),
      revealsGlobalAddress: classified.some((entry) =>
        ["ipv4-public", "ipv6-global"].includes(entry.addressClass),
      ),
    };
  } finally {
    pc.close();
  }
}

/**
 * ICE candidate addresses, by what they would tell a page.
 *
 * **`ipv4-public` is a claim, not a fallback**, and treating it as the fallback was a defect: Stage 6
 * T03 measured a host candidate of `198.18.0.1` on this machine — the gateway of a Clash-family TUN
 * adapter — and this function filed it as `ipv4-public` because 198.18.0.0/15 is not RFC 1918
 * (fixed by T19). It is RFC 2544 benchmark space: not
 * globally routable, and not the user's real address either. The two blocks a proxied machine
 * plausibly binds and RFC 1918 does not cover are named here; the rest of RFC 6890's registry
 * (documentation, multicast, reserved) is deliberately left to the fallback, because none of it can
 * be a bound interface address.
 *
 * Both new classes count as *private*, not as *global*: they are addresses a page must not see, and
 * they are not the public IP — so assertion 4's deliberate `fail` stays a `fail` while the reason it
 * gives becomes true.
 */
function classifyAddress(address) {
  if (!address) {
    return null;
  }
  if (/\.local$/i.test(address)) {
    return "mdns-hostname";
  }
  const ipv4 = address.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (ipv4) {
    const [a, b] = [Number(ipv4[1]), Number(ipv4[2])];
    if (a === 127) return "ipv4-loopback";
    if (a === 169 && b === 254) return "ipv4-link-local";
    if (a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168)) {
      return "ipv4-private";
    }
    // RFC 6598 shared address space — a CGNAT'd machine's own address.
    if (a === 100 && b >= 64 && b <= 127) return "ipv4-shared";
    // RFC 2544 benchmarking — what a TUN-based local proxy hands out on this host.
    if (a === 198 && (b === 18 || b === 19)) return "ipv4-benchmark";
    return "ipv4-public";
  }
  if (address.includes(":")) {
    const lower = address.toLowerCase();
    if (lower === "::1") return "ipv6-loopback";
    if (lower.startsWith("fe8") || lower.startsWith("fe9") || lower.startsWith("fea")) {
      return "ipv6-link-local";
    }
    if (/^f[cd]/.test(lower)) return "ipv6-unique-local";
    return "ipv6-global";
  }
  return "other";
}

/**
 * Control for assertion 1. Nothing is patched yet, so
 * every entry here must read `function x() { [native code] }`. Recording it on the stock engine is
 * what makes the assertion meaningful later: after Stage 5 the same list must still say `native`.
 */
function nativeSourceSurface() {
  const navigatorProto = Object.getPrototypeOf(navigator);
  const targets = [
    ["navigator.userAgent", () => getterOf(navigatorProto, "userAgent")],
    ["navigator.platform", () => getterOf(navigatorProto, "platform")],
    ["navigator.language", () => getterOf(navigatorProto, "language")],
    ["navigator.languages", () => getterOf(navigatorProto, "languages")],
    ["navigator.hardwareConcurrency", () => getterOf(navigatorProto, "hardwareConcurrency")],
    ["navigator.deviceMemory", () => getterOf(navigatorProto, "deviceMemory")],
    ["navigator.webdriver", () => getterOf(navigatorProto, "webdriver")],
    ["navigator.maxTouchPoints", () => getterOf(navigatorProto, "maxTouchPoints")],
    ["navigator.vendor", () => getterOf(navigatorProto, "vendor")],
    ["Screen.width", () => globalGetter("Screen", "width")],
    ["Screen.height", () => globalGetter("Screen", "height")],
    ["Screen.colorDepth", () => globalGetter("Screen", "colorDepth")],
    ["HTMLCanvasElement.toDataURL", () => globalMethod("HTMLCanvasElement", "toDataURL")],
    [
      "CanvasRenderingContext2D.getImageData",
      () => globalMethod("CanvasRenderingContext2D", "getImageData"),
    ],
    [
      "CanvasRenderingContext2D.measureText",
      () => globalMethod("CanvasRenderingContext2D", "measureText"),
    ],
    ["Element.getClientRects", () => globalMethod("Element", "getClientRects")],
    [
      "WebGLRenderingContext.getParameter",
      () => globalMethod("WebGLRenderingContext", "getParameter"),
    ],
    ["WebGLRenderingContext.readPixels", () => globalMethod("WebGLRenderingContext", "readPixels")],
    ["AudioBuffer.getChannelData", () => globalMethod("AudioBuffer", "getChannelData")],
    ["BaseAudioContext.sampleRate", () => globalGetter("BaseAudioContext", "sampleRate")],
    ["Intl.DateTimeFormat.resolvedOptions", () => Intl.DateTimeFormat.prototype.resolvedOptions],
    ["Date.getTimezoneOffset", () => Date.prototype.getTimezoneOffset],
    ["Function.toString", () => Function.prototype.toString],
    [
      "RTCPeerConnection.setLocalDescription",
      () => globalMethod("RTCPeerConnection", "setLocalDescription"),
    ],
  ];
  const out = {};
  for (const [label, resolve] of targets) {
    let fn = null;
    try {
      fn = resolve();
    } catch {
      fn = null;
    }
    if (typeof fn !== "function") {
      out[label] = { absent: true };
      continue;
    }
    const source = Function.prototype.toString.call(fn);
    out[label] = { source, native: /\{\s*\[native code\]\s*\}$/.test(source) };
  }
  return out;
}

function getterOf(object, key) {
  let target = object;
  while (target) {
    const descriptor = Object.getOwnPropertyDescriptor(target, key);
    if (descriptor && descriptor.get) {
      return descriptor.get;
    }
    target = Object.getPrototypeOf(target);
  }
  return null;
}

function globalGetter(constructorName, key) {
  const ctor = globalThis[constructorName];
  return ctor ? getterOf(ctor.prototype, key) : null;
}

function globalMethod(constructorName, key) {
  const ctor = globalThis[constructorName];
  return ctor ? ctor.prototype[key] : null;
}

async function sha256(input) {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function valueOrNull(value) {
  return value === undefined ? null : value;
}

/** Turn typed arrays and dictionary-like host objects into plain JSON values. */
function plain(value) {
  if (value == null) {
    return null;
  }
  if (ArrayBuffer.isView(value)) {
    return Array.from(value);
  }
  if (typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, inner]) => [key, plain(inner)]));
  }
  return value;
}

/**
 * `Infinity` and `NaN` survive structured clone but not JSON, and `getBattery().dischargingTime` is
 * `Infinity` on a desktop. Stringify them so the snapshot keeps the information instead of turning
 * it into `null`.
 */
function sanitize(value) {
  if (typeof value === "number" && !Number.isFinite(value)) {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.map(sanitize);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, inner]) => [key, sanitize(inner)]));
  }
  return value;
}
